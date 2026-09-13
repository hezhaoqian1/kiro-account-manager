//! Anthropic Messages 协议请求归一化的回归测试。

use super::*;

#[test]
fn normalize_anthropic_request_keeps_system_tools_and_tool_result() {
    let request = AnthropicMessagesRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![crate::gateway::models::AnthropicMessage {
            role: "user".to_string(),
            content: json!([
                {
                    "type": "tool_result",
                    "tool_use_id": "tool_1",
                    "content": "42",
                    "is_error": false
                },
                {
                    "type": "text",
                    "text": "继续"
                }
            ]),
        }],
        max_tokens: 4096,
        system: Some(json!([{ "type": "text", "text": "你是测试助手" }])),
        stream: false,
        temperature: Some(0.2),
        top_p: Some(0.8),
        stop_sequences: Some(vec!["STOP".to_string()]),
        tools: Some(vec![AnthropicTool {
            r#type: Some("custom".to_string()),
            name: "math".to_string(),
            description: Some("计算器".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": { "expr": { "type": "string" } },
                "required": ["expr"]
            }),
            cache_control: None,
        }]),
        tool_choice: Some(json!({"type":"auto"})),
        thinking: None,
        metadata: None,
        top_k: None,
    };

    let converted = normalize_anthropic_request(&request);
    assert_eq!(converted.messages.len(), 2);
    assert_eq!(converted.messages[0].role, "system");
    assert_eq!(converted.tools.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        converted.messages[1].tool_call_id.as_deref(),
        Some("tool_1")
    );
}

#[test]
fn normalize_anthropic_request_preserves_image_content() {
    // 测试：包含图片的 content 应该保留为数组，不应该被转换为字符串
    let request = AnthropicMessagesRequest {
        model: "claude-sonnet-4-5".to_string(),
        messages: vec![crate::gateway::models::AnthropicMessage {
            role: "user".to_string(),
            content: json!([
                {
                    "type": "text",
                    "text": "这是什么图片？"
                },
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
                    }
                }
            ]),
        }],
        max_tokens: 1024,
        system: None,
        stream: false,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: None,
        tools: None,
        tool_choice: None,
        thinking: None,
        metadata: None,
    };

    let converted = normalize_anthropic_request(&request);

    // 验证 content 仍然是数组（而不是被转换成字符串）
    assert_eq!(converted.messages.len(), 1);
    let content = converted.messages[0]
        .content
        .as_ref()
        .expect("content should exist");

    // 关键断言：content 应该是 Array，不是 String
    assert!(
        content.is_array(),
        "content should be an array to preserve image data"
    );

    let content_array = content.as_array().expect("content should be array");
    assert_eq!(
        content_array.len(),
        2,
        "should have 2 items: text and image"
    );

    // 验证图片 block 仍然存在
    let image_block = &content_array[1];
    assert_eq!(
        image_block.get("type").and_then(Value::as_str),
        Some("image"),
        "image block should be preserved"
    );
    assert!(
        image_block.get("source").is_some(),
        "image source should be preserved"
    );
}
