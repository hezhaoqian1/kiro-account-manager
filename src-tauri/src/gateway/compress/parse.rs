//! 从 EventStream 响应中解析摘要与 token 信息。

use crate::gateway::eventstream::decode_message;
use serde_json::Value;

/// 从 EventStream 响应中解析摘要（尝试提取 token 信息）
pub(super) fn parse_summary_from_eventstream(
    body: &[u8],
) -> Result<(String, Option<i32>, Option<i32>), String> {
    let mut summary = String::new();
    let mut input_tokens: Option<i32> = None;
    let mut output_tokens: Option<i32> = None;
    let mut offset = 0;

    while offset < body.len() {
        match decode_message(&body[offset..]) {
            Ok(Some((event, consumed))) => {
                offset += consumed;

                // 解析 payload 为 JSON
                if let Ok(value) = serde_json::from_slice::<Value>(&event.payload) {
                    // 提取文本内容
                    if let Some(content) = value
                        .get("assistantResponseEvent")
                        .and_then(|e| e.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        summary.push_str(content);
                    }

                    // 尝试提取 token 信息（即使可能不存在）
                    // 方式1：metadataEvent.tokenUsage（新格式）
                    if let Some(metadata_event) = value.get("metadataEvent") {
                        if let Some(token_usage) = metadata_event.get("tokenUsage") {
                            let uncached = token_usage
                                .get("uncachedInputTokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0) as i32;

                            // input_tokens 仅为未缓存的输入（Anthropic 规范，避免双重计费）
                            input_tokens = Some(uncached);
                            output_tokens = token_usage
                                .get("outputTokens")
                                .and_then(|v| v.as_i64())
                                .map(|v| v as i32);

                            log::info!(
                                "[压缩] ✅ 从 metadataEvent 提取到 token: 输入={:?}, 输出={:?}",
                                input_tokens,
                                output_tokens
                            );
                        }
                    }

                    // 方式2：顶层 usage 字段（旧格式）
                    if input_tokens.is_none() {
                        if let Some(usage) = value.get("usage") {
                            input_tokens = usage
                                .get("inputTokens")
                                .or_else(|| usage.get("input_tokens"))
                                .and_then(|v| v.as_i64())
                                .map(|v| v as i32);
                            output_tokens = usage
                                .get("outputTokens")
                                .or_else(|| usage.get("output_tokens"))
                                .and_then(|v| v.as_i64())
                                .map(|v| v as i32);

                            if input_tokens.is_some() || output_tokens.is_some() {
                                log::info!(
                                    "[压缩] ✅ 从 usage 提取到 token: 输入={:?}, 输出={:?}",
                                    input_tokens,
                                    output_tokens
                                );
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                break;
            }
            Err(e) => {
                log::warn!("[压缩] 解析 EventStream 失败: {}", e);
                break;
            }
        }
    }

    if summary.is_empty() {
        Err("未能从响应中提取摘要".to_string())
    } else {
        Ok((summary, input_tokens, output_tokens))
    }
}
