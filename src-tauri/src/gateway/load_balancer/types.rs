//! 策略枚举与账号健康度数据结构（含序列化形态）。

use super::*;

/// 负载均衡策略
#[derive(Debug, Clone, PartialEq)]
pub enum LoadBalancerStrategy {
    /// 轮询（Round Robin）- 按顺序轮流使用账号
    RoundRobin,
    /// 随机（Random）- 随机选择账号
    Random,
    /// 均衡（Balanced）- 优先使用成功次数最少的账号
    Balanced,
    /// 最多配额（Most Quota）- 优先使用剩余配额最多的账号
    MostQuota,
    /// 加权随机（Weighted Random）- 根据配额和成功率加权随机
    WeightedRandom,
    /// 最少连接（Least Connections）- 优先使用活跃连接最少的账号
    LeastConnections,
}

impl LoadBalancerStrategy {
    pub fn from_str(s: &str) -> Self {
        match s {
            "round_robin" => Self::RoundRobin,
            "random" => Self::Random,
            "balanced" => Self::Balanced,
            "most_quota" => Self::MostQuota,
            "weighted_random" => Self::WeightedRandom,
            "least_connections" => Self::LeastConnections,
            _ => Self::RoundRobin, // 默认轮询
        }
    }
}

/// 账号健康状态
#[derive(Debug, Clone)]
pub struct AccountHealth {
    /// 账号 ID
    #[allow(dead_code)]
    pub account_id: String,
    /// 活跃连接数
    pub active_connections: usize,
    /// 最近失败次数（滑动窗口）
    pub recent_failures: usize,
    /// 最近成功次数（滑动窗口）
    pub recent_successes: usize,
    /// 最后一次健康检查时间
    pub last_check: Instant,
    /// 是否健康
    pub is_healthy: bool,
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: u64,
}

/// 可序列化的健康状态（用于API响应）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SerializableAccountHealth {
    pub account_id: String,
    pub active_connections: usize,
    pub recent_failures: usize,
    pub recent_successes: usize,
    pub is_healthy: bool,
    pub avg_response_time_ms: u64,
    pub health_score: u32,
}

impl From<&AccountHealth> for SerializableAccountHealth {
    fn from(health: &AccountHealth) -> Self {
        Self {
            account_id: health.account_id.clone(),
            active_connections: health.active_connections,
            recent_failures: health.recent_failures,
            recent_successes: health.recent_successes,
            is_healthy: health.is_healthy,
            avg_response_time_ms: health.avg_response_time_ms,
            health_score: health.health_score(),
        }
    }
}

impl AccountHealth {
    pub fn new(account_id: String) -> Self {
        Self {
            account_id,
            active_connections: 0,
            recent_failures: 0,
            recent_successes: 0,
            last_check: Instant::now(),
            is_healthy: true,
            avg_response_time_ms: 0,
        }
    }

    /// 计算健康分数（0-100）
    pub fn health_score(&self) -> u32 {
        if !self.is_healthy {
            return 0;
        }

        let total_requests = self.recent_successes + self.recent_failures;
        if total_requests == 0 {
            return 100; // 新账号，默认满分
        }

        // 成功率（0-100）
        let success_rate = (self.recent_successes as f64 / total_requests as f64 * 100.0) as u32;

        // 连接负载惩罚（每 10 个连接减 5 分）
        let connection_penalty = (self.active_connections / 10) as u32 * 5;

        // 响应时间惩罚（每 1000ms 减 10 分）
        let response_penalty = (self.avg_response_time_ms / 1000) as u32 * 10;

        success_rate
            .saturating_sub(connection_penalty)
            .saturating_sub(response_penalty)
    }

    /// 记录成功
    pub fn record_success(&mut self, response_time_ms: u64) {
        self.recent_successes += 1;
        self.is_healthy = true;
        self.last_check = Instant::now();

        // 更新平均响应时间（简单移动平均）
        if self.avg_response_time_ms == 0 {
            self.avg_response_time_ms = response_time_ms;
        } else {
            self.avg_response_time_ms = (self.avg_response_time_ms * 9 + response_time_ms) / 10;
        }

        // 滑动窗口：保持最近 100 次请求的统计
        if self.recent_successes + self.recent_failures > 100 {
            self.recent_successes = (self.recent_successes * 9) / 10;
            self.recent_failures = (self.recent_failures * 9) / 10;
        }
    }

    /// 记录失败
    pub fn record_failure(&mut self) {
        self.recent_failures += 1;
        self.last_check = Instant::now();

        // 连续失败 3 次标记为不健康
        if self.recent_failures >= 3 && self.recent_successes == 0 {
            self.is_healthy = false;
        }

        // 滑动窗口
        if self.recent_successes + self.recent_failures > 100 {
            self.recent_successes = (self.recent_successes * 9) / 10;
            self.recent_failures = (self.recent_failures * 9) / 10;
        }
    }

    /// 增加活跃连接
    pub fn increment_connections(&mut self) {
        self.active_connections += 1;
    }

    /// 减少活跃连接
    pub fn decrement_connections(&mut self) {
        self.active_connections = self.active_connections.saturating_sub(1);
    }
}
