//! OpenAI Chat Completions 协议请求归一化的回归测试。

use super::*;

#[test]
fn convert_openai_chat_messages_maps_official_developer_and_legacy_function() {
    let messages = convert_openai_chat_messages(Some(&json!([
        { "role": "developer", "content": "你是网关助手" },
        {
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": { "name": "search", "arguments": "{\"q\":\"x\"}" }
            }]
        },
        { "role": "function", "name": "search", "content": "结果" },
        { "role": "user", "content": "继续" },
        { "role": "invalid_role", "content": "应被跳过" }
    ])));

    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0].role, "system");
    assert_eq!(messages[0].content, Some(json!("你是网关助手")));
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(
        messages[1]
            .tool_calls
            .as_ref()
            .and_then(|c| c.first())
            .map(|c| c.function.name.as_str()),
        Some("search")
    );
    assert_eq!(messages[2].role, "tool");
    assert_eq!(messages[2].tool_call_id.as_deref(), Some("search"));
    assert_eq!(messages[2].content, Some(json!("结果")));
    assert_eq!(messages[3].role, "user");
}

#[test]
fn normalize_openai_chat_request_rejects_unknown_role() {
    let request: OpenAIChatRequest = serde_json::from_value(json!({
        "model": "claude-sonnet-4",
        "messages": [
            { "role": "not_a_role", "content": "x" }
        ]
    }))
    .expect("request should parse");

    let err = normalize_openai_chat_request(&request).expect_err("should reject unknown role");
    assert!(err.contains("不支持的 chat message.role"));
}
