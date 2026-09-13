//! LoadBalancer 主体：取号、健康检查、加权轮询与故障转移，以及配额估算。

use super::*;

/// 负载均衡器
#[derive(Debug)]
pub struct LoadBalancer {
    /// 负载均衡策略
    strategy: LoadBalancerStrategy,
    /// 当前轮询索引（用于 RoundRobin）
    current_index: Arc<RwLock<usize>>,
    /// 账号健康状态
    health_map: Arc<RwLock<HashMap<String, AccountHealth>>>,
    /// 健康检查间隔
    #[allow(dead_code)]
    health_check_interval: Duration,
    /// 速率限制的账号（临时屏蔽）
    rate_limited_accounts: Arc<RwLock<HashMap<String, Instant>>>,
}

impl LoadBalancer {
    pub fn new(strategy: LoadBalancerStrategy) -> Self {
        Self {
            strategy,
            current_index: Arc::new(RwLock::new(0)),
            health_map: Arc::new(RwLock::new(HashMap::new())),
            health_check_interval: Duration::from_secs(30),
            rate_limited_accounts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 选择账号
    pub async fn select_account(&self, accounts: &[Account]) -> Option<Account> {
        if accounts.is_empty() {
            return None;
        }

        // 过滤健康的账号
        let healthy_accounts = self.filter_healthy_accounts(accounts).await;

        if healthy_accounts.is_empty() {
            // 如果没有健康的账号，尝试重置健康状态
            self.reset_health_if_all_unhealthy(accounts).await;
            return accounts.first().cloned();
        }

        match self.strategy {
            LoadBalancerStrategy::RoundRobin => self.select_round_robin(&healthy_accounts).await,
            LoadBalancerStrategy::Random => self.select_random(&healthy_accounts),
            LoadBalancerStrategy::Balanced => self.select_balanced(&healthy_accounts),
            LoadBalancerStrategy::MostQuota => self.select_most_quota(&healthy_accounts),
            LoadBalancerStrategy::WeightedRandom => {
                self.select_weighted_random(&healthy_accounts).await
            }
            LoadBalancerStrategy::LeastConnections => {
                self.select_least_connections(&healthy_accounts).await
            }
        }
    }

    /// 过滤健康的账号（排除速率限制和已封禁的账号）
    async fn filter_healthy_accounts(&self, accounts: &[Account]) -> Vec<Account> {
        let health_map = self.health_map.read().await;
        let rate_limited = self.rate_limited_accounts.read().await;

        accounts
            .iter()
            .filter(|acc| {
                // 检查账号是否启用（数据库持久化状态）
                if !acc.enabled {
                    log::debug!("[LoadBalancer] 跳过已禁用的账号: {} (enabled=false)", acc.label);
                    return false;
                }

                // 检查是否被速率限制
                if let Some(blocked_at) = rate_limited.get(&acc.id) {
                    if blocked_at.elapsed().as_secs() < 60 {
                        log::debug!("[LoadBalancer] 跳过被速率限制的账号: {}", acc.label);
                        return false;
                    }
                }

                // 检查健康状态
                health_map
                    .get(&acc.id)
                    .map(|h| h.is_healthy)
                    .unwrap_or(true) // 新账号默认健康
            })
            .cloned()
            .collect()
    }

    /// 如果所有账号都不健康，重置健康状态
    async fn reset_health_if_all_unhealthy(&self, accounts: &[Account]) {
        let mut health_map = self.health_map.write().await;

        let all_unhealthy = accounts.iter().all(|acc| {
            health_map
                .get(&acc.id)
                .map(|h| !h.is_healthy)
                .unwrap_or(false)
        });

        if all_unhealthy {
            log::warn!("[LoadBalancer] 所有账号都不健康，执行自愈机制重置健康状态");
            for acc in accounts {
                if let Some(health) = health_map.get_mut(&acc.id) {
                    health.is_healthy = true;
                    health.recent_failures = 0;
                }
            }
        }
    }

    /// 轮询选择
    async fn select_round_robin(&self, accounts: &[Account]) -> Option<Account> {
        let mut index = self.current_index.write().await;
        let account = accounts.get(*index % accounts.len()).cloned();
        *index = (*index + 1) % accounts.len();
        account
    }

    /// 随机选择
    fn select_random(&self, accounts: &[Account]) -> Option<Account> {
        let mut rng = thread_rng();
        accounts.choose(&mut rng).cloned()
    }

    /// 均衡选择（优先使用成功次数最少的账号）
    fn select_balanced(&self, accounts: &[Account]) -> Option<Account> {
        accounts.iter().min_by_key(|acc| acc.success_count).cloned()
    }

    /// 最多配额选择
    fn select_most_quota(&self, accounts: &[Account]) -> Option<Account> {
        accounts
            .iter()
            .max_by(|a, b| {
                let quota_a = remaining_quota(a);
                let quota_b = remaining_quota(b);
                quota_a
                    .partial_cmp(&quota_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    /// 加权随机选择（根据健康分数和配额加权）
    async fn select_weighted_random(&self, accounts: &[Account]) -> Option<Account> {
        let health_map = self.health_map.read().await;

        // 计算权重：健康分数 × 剩余配额百分比
        let weights: Vec<u32> = accounts
            .iter()
            .map(|acc| {
                let health_score = health_map
                    .get(&acc.id)
                    .map(|h| h.health_score())
                    .unwrap_or(100);

                let quota_percent = (remaining_quota(acc) * 100.0) as u32;

                // 权重 = 健康分数 × (配额百分比 + 1)
                // +1 避免配额为 0 时权重为 0
                health_score * (quota_percent + 1)
            })
            .collect();

        // 如果所有权重都是 0，返回第一个账号
        if weights.iter().all(|&w| w == 0) {
            return accounts.first().cloned();
        }

        // 加权随机选择
        let dist = WeightedIndex::new(&weights).ok()?;
        let mut rng = thread_rng();
        let index = dist.sample(&mut rng);

        accounts.get(index).cloned()
    }

    /// 最少连接选择
    async fn select_least_connections(&self, accounts: &[Account]) -> Option<Account> {
        let health_map = self.health_map.read().await;

        accounts
            .iter()
            .min_by_key(|acc| {
                health_map
                    .get(&acc.id)
                    .map(|h| h.active_connections)
                    .unwrap_or(0)
            })
            .cloned()
    }

    /// 增加账号的活跃连接数
    pub async fn increment_connections(&self, account_id: &str) {
        let mut health_map = self.health_map.write().await;
        health_map
            .entry(account_id.to_string())
            .or_insert_with(|| AccountHealth::new(account_id.to_string()))
            .increment_connections();
    }

    /// 减少账号的活跃连接数
    pub async fn decrement_connections(&self, account_id: &str) {
        let mut health_map = self.health_map.write().await;
        if let Some(health) = health_map.get_mut(account_id) {
            health.decrement_connections();
        }
    }

    /// 记录账号请求成功
    pub async fn record_success(&self, account_id: &str, response_time_ms: u64) {
        let mut health_map = self.health_map.write().await;
        health_map
            .entry(account_id.to_string())
            .or_insert_with(|| AccountHealth::new(account_id.to_string()))
            .record_success(response_time_ms);
    }

    /// 记录账号请求失败
    pub async fn record_failure(&self, account_id: &str) {
        let mut health_map = self.health_map.write().await;
        health_map
            .entry(account_id.to_string())
            .or_insert_with(|| AccountHealth::new(account_id.to_string()))
            .record_failure();
    }

    /// 标记账号为速率限制（临时屏蔽 60 秒）
    pub async fn mark_rate_limited(&self, account_id: &str) {
        let mut rate_limited = self.rate_limited_accounts.write().await;
        rate_limited.insert(account_id.to_string(), Instant::now());
        log::info!(
            "[LoadBalancer] 账号 {} 被标记为速率限制，将屏蔽 60 秒",
            account_id
        );
    }

    /// 标记账号为已封禁（永久屏蔽，直到手动重置）
    ///
    /// 此方法会直接更新账号数据库，设置 status="banned" 和 enabled=false
    /// 不再使用内存标记，直接依赖数据库的 enabled 字段过滤
    pub async fn mark_account_banned(&self, account_id: &str) {
        log::warn!(
            "[LoadBalancer] 账号 {} 被标记为已封禁，正在持久化到数据库",
            account_id
        );

        // 标记为不健康
        let mut health_map = self.health_map.write().await;
        if let Some(health) = health_map.get_mut(account_id) {
            health.is_healthy = false;
            health.recent_failures = 999; // 设置高失败次数
        }
        drop(health_map); // 释放锁

        // 持久化到数据库
        if let Err(e) = Self::persist_banned_status(account_id).await {
            log::error!(
                "[LoadBalancer] 持久化账号 {} 的封禁状态失败: {}",
                account_id,
                e
            );
        }
    }

    /// 持久化封禁状态到数据库
    async fn persist_banned_status(account_id: &str) -> Result<(), String> {
        use crate::core::account::AccountStore;
        use crate::commands::common::update_account_status;

        let mut store = AccountStore::new();

        // 查找并更新账号
        let mut found = false;
        let accounts = &mut store.accounts;

        for account in accounts.iter_mut() {
            if account.id == account_id {
                found = true;

                // 更新状态为封禁，enabled 设为 false
                update_account_status(account, true, false);

                log::info!(
                    "[LoadBalancer] 正在持久化账号 {} 的封禁状态: status={}, enabled={}",
                    account_id,
                    account.status,
                    account.enabled
                );
                break;
            }
        }

        if !found {
            return Err(format!("账号 {} 未找到", account_id));
        }

        // 保存到文件
        store.try_save_to_file()?;

        log::info!("[LoadBalancer] 账号 {} 的封禁状态已持久化到数据库", account_id);
        Ok(())
    }

    /// 获取所有被速率限制的账号
    pub async fn get_rate_limited_accounts(&self) -> Vec<String> {
        let mut rate_limited = self.rate_limited_accounts.write().await;

        // 清理过期的屏蔽
        rate_limited.retain(|account_id, blocked_at| {
            if blocked_at.elapsed().as_secs() >= 60 {
                log::info!("[LoadBalancer] 账号 {} 速率限制已自动解除", account_id);
                false
            } else {
                true
            }
        });

        rate_limited.keys().cloned().collect()
    }

    /// 清除账号的速率限制标记
    pub async fn clear_rate_limit(&self, account_id: &str) {
        let mut rate_limited = self.rate_limited_accounts.write().await;
        if rate_limited.remove(account_id).is_some() {
            log::info!("[LoadBalancer] 手动清除账号 {} 的速率限制", account_id);
        }
    }

    /// 获取所有账号的健康状态
    pub async fn get_all_health(&self) -> HashMap<String, AccountHealth> {
        let health_map = self.health_map.read().await;
        health_map.clone()
    }

    /// 重置账号健康状态
    pub async fn reset_health(&self, account_id: &str) {
        let mut health_map = self.health_map.write().await;
        if let Some(health) = health_map.get_mut(account_id) {
            health.is_healthy = true;
            health.recent_failures = 0;
            health.recent_successes = 0;
        }
    }

    /// 清理过期的健康状态（超过 1 小时未使用）
    pub async fn cleanup_stale_health(&self) {
        let mut health_map = self.health_map.write().await;
        let now = Instant::now();
        let stale_duration = Duration::from_secs(3600); // 1 小时

        health_map.retain(|_, health| now.duration_since(health.last_check) < stale_duration);
    }
}

/// 计算账号剩余配额百分比
fn remaining_quota(account: &Account) -> f64 {
    // 从 usage_data 中提取配额信息
    if let Some(usage_data) = &account.usage_data {
        // 尝试提取 remaining 和 total
        if let (Some(remaining), Some(total)) = (
            usage_data.get("remaining").and_then(|v| v.as_i64()),
            usage_data.get("total").and_then(|v| v.as_i64()),
        ) {
            if total > 0 {
                return (remaining as f64 / total as f64) * 100.0;
            }
        }
    }
    100.0 // 默认 100%
}
