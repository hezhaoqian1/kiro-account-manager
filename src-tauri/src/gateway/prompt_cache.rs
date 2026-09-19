//! Prompt Cache 模拟器
//! 在2API侧追踪 cache_control 断点，模拟 Anthropic 的 prompt caching 行为
//! 让 Claude Code 的 cache_control 字段产生实际效果的 usage 统计

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::gateway::models::NormalizedMessage;

// 常量
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(5 * 60); // 5 分钟
const ONE_HOUR_CACHE_TTL: Duration = Duration::from_secs(60 * 60); // 1 小时
const DEFAULT_MIN_CACHEABLE_TOKENS: usize = 1024;
const OPUS_MIN_CACHEABLE_TOKENS: usize = 4096;
const MAX_CACHE_RATIO: f64 = 0.85; // 最新内容不可能 100% 缓存命中
const MAX_ENTRIES_PER_ACCOUNT: usize = 200;

/// 缓存使用统计
#[derive(Debug, Clone, Default)]
pub struct CacheUsage {
    pub cache_creation_input_tokens: usize,
    pub cache_read_input_tokens: usize,
}

/// Merge Kiro's metering with the proxy's account-scoped fallback.
///
/// Kiro can report a positive cache write for every stateless request even
/// when the same stable prefix was already seen by this proxy. In that case a
/// local read is more useful to downstream Anthropic clients. A real upstream
/// read always wins because it is the authoritative billing signal.
pub fn merge_cache_usage(
    upstream_read: Option<i32>,
    upstream_creation: Option<i32>,
    local: &CacheUsage,
) -> (Option<i32>, Option<i32>, &'static str) {
    if upstream_read.unwrap_or(0) > 0 {
        return (upstream_read, upstream_creation, "upstream");
    }

    if local.cache_read_input_tokens > 0 {
        return (
            Some(local.cache_read_input_tokens as i32),
            (local.cache_creation_input_tokens > 0)
                .then_some(local.cache_creation_input_tokens as i32),
            "local_estimate",
        );
    }

    if upstream_creation.unwrap_or(0) > 0 {
        return (upstream_read, upstream_creation, "upstream");
    }

    (
        (local.cache_read_input_tokens > 0).then_some(local.cache_read_input_tokens as i32),
        (local.cache_creation_input_tokens > 0).then_some(local.cache_creation_input_tokens as i32),
        "local_estimate",
    )
}

/// Convert normalized messages to the cache tracker's lossless JSON view.
///
/// Anthropic `cache_control` is normalized into `metadata.cache_point` before
/// the Kiro payload is built. Keep that metadata here so the local estimator
/// can still find the caller's cache breakpoint after normalization.
pub fn normalized_messages_for_cache(messages: &[NormalizedMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            json!({
                "role": message.role,
                "content": message.content,
                "metadata": message.metadata,
            })
        })
        .collect()
}

/// 缓存断点
#[derive(Debug, Clone)]
struct CacheBreakpoint {
    fingerprint: [u8; 32],
    cumulative_tokens: usize,
    ttl: Duration,
}

/// 缓存 Profile（一次请求的缓存结构）
#[derive(Debug, Clone)]
pub struct CacheProfile {
    breakpoints: Vec<CacheBreakpoint>,
    total_input_tokens: usize,
    model: String,
}

/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    expires_at: Instant,
    ttl: Duration,
}

/// 可缓存的内容块
struct CacheableBlock {
    value: String,
    tokens: usize,
    ttl: Duration,
    is_message_end: bool,
}

/// Prompt Cache Tracker（全局单例）
pub struct PromptCacheTracker {
    entries_by_account: Mutex<HashMap<String, HashMap<[u8; 32], CacheEntry>>>,
}

impl PromptCacheTracker {
    pub fn new() -> Self {
        Self {
            entries_by_account: Mutex::new(HashMap::new()),
        }
    }

    /// 从 Anthropic 格式请求构建缓存 profile
    pub fn build_profile(
        &self,
        system: Option<&serde_json::Value>,
        messages: &[serde_json::Value],
        tools: Option<&[serde_json::Value]>,
        total_input_tokens: usize,
        model: &str,
    ) -> Option<CacheProfile> {
        let blocks = self.flatten_cache_blocks(system, messages, tools);
        if blocks.is_empty() {
            return None;
        }

        let mut hasher = Sha256::new();
        let mut breakpoints = Vec::new();
        let mut cumulative_tokens = 0usize;
        let mut active_ttl = Duration::ZERO;

        for block in &blocks {
            self.hash_chunk(&mut hasher, &block.value);
            cumulative_tokens += block.tokens;

            let breakpoint_ttl = if block.ttl > Duration::ZERO {
                active_ttl = block.ttl;
                block.ttl
            } else if block.is_message_end && active_ttl > Duration::ZERO {
                active_ttl
            } else {
                Duration::ZERO
            };

            if breakpoint_ttl == Duration::ZERO {
                continue;
            }

            let fingerprint: [u8; 32] = hasher.clone().finalize().into();
            breakpoints.push(CacheBreakpoint {
                fingerprint,
                cumulative_tokens,
                ttl: breakpoint_ttl,
            });
        }

        if breakpoints.is_empty() {
            return None;
        }

        Some(CacheProfile {
            breakpoints,
            total_input_tokens: total_input_tokens.max(cumulative_tokens),
            model: model.to_string(),
        })
    }

    /// 计算缓存命中情况
    pub fn compute(&self, account_id: &str, profile: &CacheProfile) -> CacheUsage {
        if profile.breakpoints.is_empty() || account_id.is_empty() {
            return CacheUsage::default();
        }

        let min_tokens = self.min_cacheable_tokens(&profile.model);
        let last = &profile.breakpoints[profile.breakpoints.len() - 1];
        let mut last_tokens = last.cumulative_tokens.min(profile.total_input_tokens);
        let now = Instant::now();

        let mut entries_map = self
            .entries_by_account
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.prune_expired(&mut entries_map, now);

        let entries = entries_map.get_mut(account_id);
        if entries.is_none() || entries.as_ref().unwrap().is_empty() {
            // 首次请求：全部是 creation
            let effective_creation = if last_tokens >= min_tokens {
                last_tokens
            } else {
                0
            };
            return CacheUsage {
                cache_creation_input_tokens: effective_creation,
                cache_read_input_tokens: 0,
            };
        }

        let entries = entries.unwrap();

        // 上限 85%
        let max_cacheable = (profile.total_input_tokens as f64 * MAX_CACHE_RATIO) as usize;
        if last_tokens > max_cacheable {
            last_tokens = max_cacheable;
        }

        // 从后往前匹配最长前缀
        let mut matched_tokens = 0usize;
        for bp in profile.breakpoints.iter().rev() {
            if bp.cumulative_tokens < min_tokens {
                continue;
            }
            if let Some(entry) = entries.get_mut(&bp.fingerprint) {
                if entry.expires_at > now {
                    // 命中：刷新过期时间
                    entry.expires_at = now + entry.ttl;
                    matched_tokens = bp.cumulative_tokens.min(profile.total_input_tokens);
                    if matched_tokens > last_tokens {
                        matched_tokens = last_tokens;
                    }
                    break;
                }
            }
        }

        let creation = last_tokens.saturating_sub(matched_tokens);
        CacheUsage {
            cache_creation_input_tokens: creation,
            cache_read_input_tokens: matched_tokens,
        }
    }

    /// 更新缓存条目（请求成功后调用）
    pub fn update(&self, account_id: &str, profile: &CacheProfile) {
        if profile.breakpoints.is_empty() || account_id.is_empty() {
            return;
        }

        let min_tokens = self.min_cacheable_tokens(&profile.model);
        let now = Instant::now();

        let mut entries_map = self
            .entries_by_account
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let entries = entries_map.entry(account_id.to_string()).or_default();

        for bp in &profile.breakpoints {
            if bp.cumulative_tokens < min_tokens {
                continue;
            }
            entries.insert(
                bp.fingerprint,
                CacheEntry {
                    expires_at: now + bp.ttl,
                    ttl: bp.ttl,
                },
            );
        }

        // 限制条目数
        if entries.len() > MAX_ENTRIES_PER_ACCOUNT {
            let mut sorted: Vec<_> = entries.iter().map(|(k, v)| (*k, v.expires_at)).collect();
            sorted.sort_by_key(|(_, exp)| *exp);
            let to_remove = entries.len() - MAX_ENTRIES_PER_ACCOUNT;
            for (key, _) in sorted.iter().take(to_remove) {
                entries.remove(key);
            }
        }
    }

    // ============ 内部方法 ============

    fn flatten_cache_blocks(
        &self,
        system: Option<&serde_json::Value>,
        messages: &[serde_json::Value],
        tools: Option<&[serde_json::Value]>,
    ) -> Vec<CacheableBlock> {
        let mut blocks = Vec::new();
        let default_ttl = Duration::from_secs(5 * 60); // 5 分钟默认 TTL

        // 工具定义（自动可缓存）
        if let Some(tools) = tools {
            for tool in tools {
                let value = self.canonicalize(tool);
                let tokens = estimate_tokens(&value);
                let ttl = self.extract_ttl(tool);
                blocks.push(CacheableBlock {
                    value,
                    tokens,
                    ttl: if ttl > Duration::ZERO {
                        ttl
                    } else {
                        default_ttl
                    },
                    is_message_end: false,
                });
            }
        }

        // System prompt（自动可缓存）
        if let Some(system) = system {
            match system {
                serde_json::Value::String(s) => {
                    let tokens = estimate_tokens(s);
                    blocks.push(CacheableBlock {
                        value: self.canonicalize(system),
                        tokens,
                        ttl: default_ttl,
                        is_message_end: false,
                    });
                }
                serde_json::Value::Array(arr) => {
                    for block in arr {
                        let value = self.canonicalize(block);
                        let tokens = estimate_tokens(&value);
                        let ttl = self.extract_ttl(block);
                        blocks.push(CacheableBlock {
                            value,
                            tokens,
                            ttl: if ttl > Duration::ZERO {
                                ttl
                            } else {
                                default_ttl
                            },
                            is_message_end: false,
                        });
                    }
                }
                _ => {}
            }
        }

        // Messages：system 自动缓存；普通消息只有显式 cache_control 才可缓存。
        for (i, msg) in messages.iter().enumerate() {
            let content = msg.get("content");
            // Anthropic 的 system 在归一化阶段会进入 messages，而不是传给
            // build_profile 的独立 system 参数。system prompt 是代理自己的
            // 稳定缓存前缀，即使客户端（例如 BirdSub2Api）没有携带
            // cache_control，也必须形成默认的 5 分钟缓存断点。
            let is_system = msg.get("role").and_then(Value::as_str) == Some("system");
            let _is_last_msg = i == messages.len() - 1;

            match content {
                Some(serde_json::Value::String(s)) => {
                    let value = self.canonicalize(msg);
                    let tokens = estimate_tokens(s);
                    let ttl = self.extract_ttl(msg);
                    blocks.push(CacheableBlock {
                        value,
                        tokens,
                        ttl: if ttl > Duration::ZERO || is_system {
                            if ttl > Duration::ZERO {
                                ttl
                            } else {
                                default_ttl
                            }
                        } else {
                            Duration::ZERO
                        },
                        is_message_end: true,
                    });
                }
                Some(serde_json::Value::Array(arr)) => {
                    let last_idx = arr.len().saturating_sub(1);
                    for (j, block) in arr.iter().enumerate() {
                        let value = self.canonicalize(block);
                        let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                        let tokens = estimate_tokens(if text.is_empty() { &value } else { text });
                        let ttl = self.extract_ttl(block);
                        blocks.push(CacheableBlock {
                            value,
                            tokens,
                            ttl: if ttl > Duration::ZERO || is_system {
                                if ttl > Duration::ZERO {
                                    ttl
                                } else {
                                    default_ttl
                                }
                            } else {
                                Duration::ZERO
                            },
                            is_message_end: j == last_idx,
                        });
                    }
                }
                _ => {}
            }
        }

        blocks
    }

    fn extract_ttl(&self, value: &serde_json::Value) -> Duration {
        let cache_control = value.get("cache_control").or_else(|| {
            value
                .get("metadata")
                .and_then(|metadata| metadata.get("cache_point"))
        });
        let Some(cc) = cache_control else {
            return Duration::ZERO;
        };
        let Some(cc_type) = cc.get("type").and_then(|t| t.as_str()) else {
            return Duration::ZERO;
        };
        if !cc_type.eq_ignore_ascii_case("ephemeral") && !cc_type.eq_ignore_ascii_case("default") {
            return Duration::ZERO;
        }
        // 检查 ttl 字段
        if let Some(ttl_val) = cc.get("ttl") {
            if let Some(s) = ttl_val.as_str() {
                if s == "1h" || s == "1H" {
                    return ONE_HOUR_CACHE_TTL;
                }
            }
            if let Some(n) = ttl_val.as_u64() {
                if n > 0 {
                    return Duration::from_secs(n);
                }
            }
        }
        DEFAULT_CACHE_TTL
    }

    fn canonicalize(&self, value: &serde_json::Value) -> String {
        // Cache markers are routing metadata, not prompt content. Remove them
        // recursively so a client adding/removing cache_control (or our
        // normalized metadata.cache_point) cannot change the prefix fingerprint.
        fn normalize(value: &serde_json::Value) -> serde_json::Value {
            match value {
                serde_json::Value::Object(map) => {
                    let mut keys: Vec<_> = map.keys().map(String::as_str).collect();
                    keys.sort_unstable();
                    let mut normalized = serde_json::Map::new();
                    for key in keys {
                        if key == "cache_control" {
                            continue;
                        }
                        if key == "cache_point" {
                            continue;
                        }
                        let normalized_value = normalize(&map[key]);
                        if key == "metadata"
                            && (normalized_value.is_null()
                                || normalized_value
                                    .as_object()
                                    .is_some_and(|object| object.is_empty()))
                        {
                            continue;
                        }
                        normalized.insert(key.to_string(), normalized_value);
                    }
                    serde_json::Value::Object(normalized)
                }
                serde_json::Value::Array(items) => {
                    serde_json::Value::Array(items.iter().map(normalize).collect())
                }
                other => other.clone(),
            }
        }

        serde_json::to_string(&normalize(value)).unwrap_or_default()
    }

    fn hash_chunk(&self, hasher: &mut Sha256, chunk: &str) {
        hasher.update(chunk.len().to_string().as_bytes());
        hasher.update(b"\0");
        hasher.update(chunk.as_bytes());
        hasher.update(b"\0");
    }

    fn min_cacheable_tokens(&self, model: &str) -> usize {
        if model.to_lowercase().contains("opus") {
            OPUS_MIN_CACHEABLE_TOKENS
        } else {
            DEFAULT_MIN_CACHEABLE_TOKENS
        }
    }

    fn prune_expired(
        &self,
        entries_map: &mut HashMap<String, HashMap<[u8; 32], CacheEntry>>,
        now: Instant,
    ) {
        entries_map.retain(|_, entries| {
            entries.retain(|_, entry| entry.expires_at > now);
            !entries.is_empty()
        });
    }
}

/// 估算 token 数（字符数 / 4）
fn estimate_tokens(text: &str) -> usize {
    (text.len() + 3) / 4
}

// 全局单例
static GLOBAL_TRACKER: std::sync::OnceLock<PromptCacheTracker> = std::sync::OnceLock::new();

pub fn global_prompt_cache_tracker() -> &'static PromptCacheTracker {
    GLOBAL_TRACKER.get_or_init(PromptCacheTracker::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_message() -> NormalizedMessage {
        NormalizedMessage {
            role: "user".to_string(),
            content: Some(Value::String("cacheable context ".repeat(300))),
            tool_calls: None,
            tool_call_id: None,
            metadata: Some(json!({
                "cache_point": { "type": "default" }
            })),
        }
    }

    #[test]
    fn normalized_cache_point_creates_and_reads_cache_per_account() {
        let tracker = PromptCacheTracker::new();
        let messages = normalized_messages_for_cache(&[cache_message()]);
        let profile = tracker
            .build_profile(None, &messages, None, 1500, "claude-sonnet-4.5")
            .expect("cache point should create a profile");

        let created = tracker.compute("account-a", &profile);
        assert!(created.cache_creation_input_tokens >= 1024);
        assert_eq!(created.cache_read_input_tokens, 0);
        tracker.update("account-a", &profile);

        let hit = tracker.compute("account-a", &profile);
        assert!(hit.cache_read_input_tokens >= 1024);
        assert_eq!(hit.cache_creation_input_tokens, 0);

        let other_account = tracker.compute("account-b", &profile);
        assert_eq!(other_account.cache_read_input_tokens, 0);
        assert!(other_account.cache_creation_input_tokens >= 1024);
    }

    #[test]
    fn normalized_system_message_is_cached_without_client_breakpoint() {
        let tracker = PromptCacheTracker::new();
        let messages = vec![
            json!({
                "role": "system",
                "content": "system prompt ".repeat(400),
            }),
            json!({
                "role": "user",
                "content": "first request",
            }),
        ];
        let profile = tracker
            .build_profile(None, &messages, None, 1600, "claude-sonnet-5")
            .expect("system prompt should create a profile without cache_control");

        let created = tracker.compute("account-a", &profile);
        assert!(created.cache_creation_input_tokens >= 1024);
        assert_eq!(created.cache_read_input_tokens, 0);
        tracker.update("account-a", &profile);

        let hit = tracker.compute("account-a", &profile);
        assert_eq!(hit.cache_creation_input_tokens, 0);
        assert!(hit.cache_read_input_tokens >= 1024);
    }

    #[test]
    fn cache_markers_do_not_change_system_prefix_fingerprint() {
        let tracker = PromptCacheTracker::new();
        let prompt = "stable system prompt ".repeat(400);
        let first = vec![json!({"role": "system", "content": prompt})];
        let second = vec![json!({
            "role": "system",
            "content": "stable system prompt ".repeat(400),
            "metadata": {"cache_point": {"type": "default"}},
        })];

        let first_profile = tracker
            .build_profile(None, &first, None, 1600, "claude-sonnet-5")
            .expect("first profile");
        tracker.compute("account-a", &first_profile);
        tracker.update("account-a", &first_profile);

        let second_profile = tracker
            .build_profile(None, &second, None, 1600, "claude-sonnet-5")
            .expect("second profile");
        let hit = tracker.compute("account-a", &second_profile);
        assert!(hit.cache_read_input_tokens >= 1024);
        assert_eq!(hit.cache_creation_input_tokens, 0);
    }

    #[test]
    fn local_read_replaces_repeated_upstream_cache_write() {
        let local = CacheUsage {
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 2048,
        };
        let merged = merge_cache_usage(Some(0), Some(2048), &local);
        assert_eq!(merged, (Some(2048), None, "local_estimate"));
    }

    #[test]
    fn real_upstream_read_remains_authoritative() {
        let local = CacheUsage {
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 1024,
        };
        let merged = merge_cache_usage(Some(4096), Some(0), &local);
        assert_eq!(merged, (Some(4096), Some(0), "upstream"));
    }
}
