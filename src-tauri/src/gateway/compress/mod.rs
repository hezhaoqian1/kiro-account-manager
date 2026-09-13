//! 对话历史压缩：保留最近若干轮对话，中间历史交给 LLM 生成摘要（带三层缓存）。
//!
//! 按职责拆分为 summary（摘要生成、token 估算、缓存键）/ validate（消息序列校验）/ parse（EventStream 摘要解析）。

#![allow(dead_code)]

use crate::gateway::models::NormalizedMessage;
use crate::gateway::response_cache::ResponseCache;
use serde_json::Value;

mod parse;
mod summary;
mod validate;

use summary::{calculate_messages_hash, generate_summary_with_tokens};
use validate::ensure_valid_message_sequence;

/// 压缩对话历史
///
/// 策略：
/// 1. 保留最后 2 轮对话（最近的上下文）
/// 2. 将中间的历史对话发送给 LLM 生成摘要（支持三层缓存）
/// 3. 用摘要替换中间的历史消息
/// 4. 返回：[系统消息] + [摘要] + [最近2轮对话]
pub async fn compress_conversation_history(
    http: &reqwest::Client,
    access_token: &str,
    region: &str,
    messages: &mut Vec<NormalizedMessage>,
    model_id: &str,
    _max_input_tokens: usize,
    mut cache: Option<&mut ResponseCache>,
    session_id: Option<&str>,
) -> Result<bool, String> {
    // 至少需要 5 条消息才值得压缩
    if messages.len() < 5 {
        log::info!("[压缩] 消息数量不足 5 条，跳过压缩");
        return Ok(false);
    }

    log::info!("[压缩] 开始压缩对话历史，当前消息数: {}", messages.len());

    // 分离消息：系统消息 + 需要压缩的消息 + 最近的消息
    let mut system_messages = Vec::new();

    // 找出系统消息
    for msg in messages.iter() {
        if msg.role == "system" {
            system_messages.push(msg.clone());
        }
    }

    // 找出非系统消息
    let non_system: Vec<_> = messages
        .iter()
        .filter(|m| m.role != "system")
        .cloned()
        .collect();

    if non_system.len() < 5 {
        log::info!("[压缩] 非系统消息不足 5 条，跳过压缩");
        return Ok(false);
    }

    // 保留最后 4 条消息（2轮对话）
    let preserve_count = 4.min(non_system.len());
    let compress_count = non_system.len() - preserve_count;

    let to_compress = non_system[..compress_count].to_vec();
    let recent_messages = non_system[compress_count..].to_vec();

    log::info!(
        "[压缩] 系统消息: {}, 待压缩: {}, 保留: {}",
        system_messages.len(),
        to_compress.len(),
        recent_messages.len()
    );

    // 计算待压缩消息的哈希值（用于缓存键）
    let messages_hash = calculate_messages_hash(&to_compress);
    let message_count = to_compress.len();
    let total_chars: usize = to_compress
        .iter()
        .filter_map(|m| m.content.as_ref())
        .map(|c| c.to_string().len())
        .sum();

    // 尝试从缓存获取摘要
    let summary = if let (Some(cache_ref), Some(sid)) = (cache.as_mut(), session_id) {
        if let Some(cached_entry) = cache_ref.get(sid, &messages_hash, message_count, total_chars) {
            log::info!(
                "[压缩] 命中缓存！使用缓存的摘要（节省 {} 输入 tokens，{} 输出 tokens）",
                cached_entry.input_tokens,
                cached_entry.output_tokens
            );
            cached_entry.response
        } else {
            // 缓存未命中，生成新摘要
            log::info!("[压缩] 缓存未命中，调用 LLM 生成摘要...");
            let (summary, input_tokens, output_tokens) =
                generate_summary_with_tokens(http, access_token, region, &to_compress, model_id)
                    .await?;

            // 保存到缓存
            if let Some(cache_ref) = cache.as_mut() {
                cache_ref.put(
                    sid,
                    &messages_hash,
                    summary.clone(),
                    input_tokens,
                    output_tokens,
                    message_count,
                    total_chars,
                );
                log::info!(
                    "[压缩] 摘要已保存到缓存（输入 {} tokens，输出 {} tokens）",
                    input_tokens,
                    output_tokens
                );
            }

            summary
        }
    } else {
        // 没有缓存，直接生成摘要
        log::info!("[压缩] 未启用缓存，调用 LLM 生成摘要...");
        let (summary, _, _) =
            generate_summary_with_tokens(http, access_token, region, &to_compress, model_id)
                .await?;
        summary
    };

    log::info!("[压缩] 摘要生成成功，长度: {} 字符", summary.len());

    // 构建新的消息列表
    let mut new_messages = system_messages;

    // 添加摘要消息
    new_messages.push(NormalizedMessage {
        role: "assistant".to_string(),
        content: Some(Value::String(format!("[对话历史摘要]\n\n{}", summary))),
        tool_calls: None,
        tool_call_id: None,
        metadata: None,
    });

    // 添加最近的消息
    new_messages.extend(recent_messages);

    // ✅ 验证并修复消息格式（避免 400 错误）
    new_messages = ensure_valid_message_sequence(new_messages);

    *messages = new_messages;
    log::info!("[压缩] 压缩完成，新消息数: {}", messages.len());
    Ok(true)
}
