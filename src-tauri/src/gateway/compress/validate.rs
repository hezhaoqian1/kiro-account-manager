//! 消息序列校验：确保发给 Kiro 的消息序列合法。

use crate::gateway::models::NormalizedMessage;
use serde_json::Value;

/// 确保消息序列符合 Kiro API 要求（避免 400 错误）
///
/// 规则：
/// 1. 必须以 user 消息开始
/// 2. 必须以 user 消息结束
/// 3. user 和 assistant 消息必须交替出现
/// 4. 不能有空的 user 消息
pub(super) fn ensure_valid_message_sequence(
    messages: Vec<NormalizedMessage>,
) -> Vec<NormalizedMessage> {
    let mut result = Vec::new();
    let mut last_role: Option<String> = None;

    log::info!("[压缩] 开始验证消息格式，原始消息数: {}", messages.len());

    for msg in messages {
        // 跳过系统消息（系统消息不参与交替规则）
        if msg.role == "system" {
            result.push(msg);
            continue;
        }

        // 检查是否与上一条消息角色相同
        if let Some(ref last) = last_role {
            if last == &msg.role {
                // 连续相同角色，插入占位符消息
                let placeholder = if msg.role == "user" {
                    // 连续 user，插入 assistant 占位符
                    log::warn!("[压缩] 检测到连续 user 消息，插入 assistant 占位符");
                    NormalizedMessage {
                        role: "assistant".to_string(),
                        content: Some(Value::String("收到。".to_string())),
                        tool_calls: None,
                        tool_call_id: None,
                        metadata: None,
                    }
                } else {
                    // 连续 assistant，插入 user 占位符
                    log::warn!("[压缩] 检测到连续 assistant 消息，插入 user 占位符");
                    NormalizedMessage {
                        role: "user".to_string(),
                        content: Some(Value::String("继续。".to_string())),
                        tool_calls: None,
                        tool_call_id: None,
                        metadata: None,
                    }
                };
                result.push(placeholder);
            }
        }

        // 检查 user 消息是否为空
        if msg.role == "user" {
            let is_empty = match &msg.content {
                Some(Value::String(s)) => s.trim().is_empty(),
                Some(Value::Array(arr)) => arr.is_empty(),
                None => true,
                _ => false,
            };

            if is_empty && msg.tool_call_id.is_none() {
                log::warn!("[压缩] 跳过空的 user 消息");
                continue;
            }
        }

        result.push(msg.clone());
        last_role = Some(msg.role);
    }

    // 确保以 user 消息开始
    if let Some(first) = result.first() {
        if first.role != "system" && first.role != "user" {
            log::warn!("[压缩] 消息不以 user 开始，插入占位符");
            result.insert(
                0,
                NormalizedMessage {
                    role: "user".to_string(),
                    content: Some(Value::String("你好。".to_string())),
                    tool_calls: None,
                    tool_call_id: None,
                    metadata: None,
                },
            );
        }
    }

    // 确保以 user 消息结束
    if let Some(last) = result.last() {
        if last.role != "user" {
            log::warn!("[压缩] 消息不以 user 结束，追加占位符");
            result.push(NormalizedMessage {
                role: "user".to_string(),
                content: Some(Value::String("继续。".to_string())),
                tool_calls: None,
                tool_call_id: None,
                metadata: None,
            });
        }
    }

    log::info!("[压缩] 消息格式验证完成，最终消息数: {}", result.len());
    result
}
