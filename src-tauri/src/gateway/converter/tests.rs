use super::*;
use crate::gateway::models::AnthropicTool;
use base64::engine::general_purpose::STANDARD;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

/// 回归测试：工具描述超长且截断点落在多字节字符（中文）中间时不得 panic。
/// 旧实现 `&desc[..TOOL_DESCRIPTION_MAX_LENGTH]` 按字节切，会在
/// `byte index ... is not a char boundary` 处 panic（叠加 panic=abort 直接崩进程）。
#[test]
fn convert_anthropic_tool_truncates_multibyte_description_without_panic() {
    // '中' = 3 字节。3413 个 '中' = 10239 字节 > 10237(TOOL_DESCRIPTION_MAX_LENGTH)，
    // 且字节边界只落在 3 的倍数上，10237 不是边界 → 旧代码会 panic。
    let multibyte_desc = "中".repeat(3413);
    assert!(multibyte_desc.len() > TOOL_DESCRIPTION_MAX_LENGTH);
    assert!(
        !multibyte_desc.is_char_boundary(TOOL_DESCRIPTION_MAX_LENGTH),
        "测试前提：截断点必须落在多字节字符中间，否则测不到该 bug"
    );

    let tool = AnthropicTool {
        r#type: Some("custom".to_string()),
        name: "长描述工具".to_string(),
        description: Some(multibyte_desc),
        input_schema: json!({ "type": "object" }),
        cache_control: None,
    };

    // 不 panic 即通过；同时校验截断后的描述本身是合法 UTF-8（隐含于 String 类型）
    let (converted, _mapping) = convert_anthropic_tool(&tool);
    let desc = converted.function.description.expect("描述应存在");
    assert!(desc.ends_with("..."), "超长描述应被截断并加省略号");
    // 截断主体（去掉结尾的 "..."）必须落在字符边界，长度不超过字节上限
    let body_len = desc.len() - "...".len();
    assert!(body_len <= TOOL_DESCRIPTION_MAX_LENGTH);
}

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

#[tokio::test]
async fn build_kiro_payload_moves_long_tool_docs_and_tool_results_into_context() {
    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![
            NormalizedMessage {
                role: "system".to_string(),
                content: Some(json!("系统要求")),
                tool_calls: None,
                tool_call_id: None,
                metadata: None,
            },
            NormalizedMessage {
                role: "assistant".to_string(),
                content: Some(json!("我先调用工具")),
                tool_calls: Some(vec![crate::gateway::models::ToolCall {
                    id: "call_1".to_string(),
                    call_type: "function".to_string(),
                    function: crate::gateway::models::ToolCallFunction {
                        name: "search_docs".to_string(),
                        arguments: "{\"q\":\"gateway\"}".to_string(),
                    },
                }]),
                tool_call_id: None,
                metadata: None,
            },
            NormalizedMessage {
                role: "tool".to_string(),
                content: Some(json!("命中结果")),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
                metadata: None,
            },
            NormalizedMessage {
                role: "user".to_string(),
                content: Some(json!("继续总结")),
                tool_calls: None,
                tool_call_id: None,
                metadata: None,
            },
        ],
        stream: true,
        max_tokens: Some(2048),
        temperature: Some(0.1),
        top_p: None,
        stop: Some(vec!["END".to_string()]),
        tools: Some(vec![Tool {
            tool_type: "function".to_string(),
            function: crate::gateway::models::ToolFunction {
                name: "search_docs".to_string(),
                description: Some("A".repeat(TOOL_DESCRIPTION_MAX_LENGTH + 32)),
                parameters: Some(json!({
                    "type": "object",
                    "properties": { "q": { "type": "string" } }
                })),
            },
            cache_control: None,
        }]),
        tool_choice: None,
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(
        &Client::new(),
        &request,
        Some("arn:aws:codewhisperer:::profile/test".to_string()),
        None,
    )
    .await
    .expect("payload should build");
    let current = &payload
        .conversation_state
        .current_message
        .user_input_message;

    assert!(current.content.contains("Tool Documentation"));
    assert_eq!(current.model_id, "claude-sonnet-4.5");
    assert_eq!(
        payload.profile_arn.as_deref(),
        Some("arn:aws:codewhisperer:::profile/test")
    );

    let history = payload
        .conversation_state
        .history
        .expect("history should exist");
    // sanitize_history 会补全结构：
    //   [0] 占位 User("Hello")（保证以 user 开头）
    //   [1] Assistant（携带 toolUses）
    //   [2] User（携带 toolResults）
    //   [3] 占位 Assistant("understood")（修复两个连续 user）
    assert_eq!(history.len(), 4);
    match &history[1] {
        HistoryItem::Assistant {
            assistant_response_message,
        } => {
            assert_eq!(
                assistant_response_message.tool_uses.as_ref().map(Vec::len),
                Some(1)
            );
        }
        other => panic!("unexpected history item: {other:?}"),
    }
    match &history[2] {
        HistoryItem::User { user_input_message } => {
            let context = user_input_message
                .user_input_message_context
                .as_ref()
                .expect("tool result context should exist");
            assert_eq!(context.tool_results.as_ref().map(Vec::len), Some(1));
        }
        other => panic!("unexpected history item: {other:?}"),
    }
}

#[tokio::test]
async fn build_kiro_payload_uses_cached_style_model_ids_for_claude_45() {
    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!("hello")),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        stream: false,
        max_tokens: Some(1024),
        temperature: None,
        top_p: None,
        stop: None,
        tools: None,
        tool_choice: None,
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect("payload should build");

    assert_eq!(
        payload
            .conversation_state
            .current_message
            .user_input_message
            .model_id,
        "claude-sonnet-4.5"
    );
}

#[tokio::test]
async fn build_kiro_payload_uses_cached_style_model_ids_for_claude_46() {
    let request = NormalizedRequest {
        model: "claude-sonnet-4-6".to_string(),
        messages: vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!("hello")),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        stream: false,
        max_tokens: Some(1024),
        temperature: None,
        top_p: None,
        stop: None,
        tools: None,
        tool_choice: None,
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect("payload should build");

    assert_eq!(
        payload
            .conversation_state
            .current_message
            .user_input_message
            .model_id,
        "claude-sonnet-4.6"
    );
}

#[test]
fn normalize_openai_responses_request_preserves_message_content_items() {
    let payload = json!({
        "model": "claude-sonnet-4",
        "stream": true,
        "input": [
            {
                "role": "user",
                "content": [
                    { "type": "input_text", "text": "第一段" },
                    { "type": "input_text", "text": "第二段" },
                    { "type": "input_image", "image_url": "data:image/png;base64,aGVsbG8=" }
                ]
            }
        ]
    });

    let converted =
        normalize_openai_responses_request(&payload).expect("responses payload should convert");
    assert!(converted.stream);
    assert_eq!(converted.messages.len(), 1);
    assert_eq!(converted.messages[0].role, "user");
    assert_eq!(
        converted.messages[0].content,
        Some(json!([
            { "type": "input_text", "text": "第一段" },
            { "type": "input_text", "text": "第二段" },
            { "type": "input_image", "image_url": "data:image/png;base64,aGVsbG8=" }
        ]))
    );
}

#[test]
fn extract_text_content_reads_text_from_content_array_without_unwrap() {
    let content = json!([
        { "type": "input_text", "text": "第一段" },
        { "type": "output_text", "text": "第二段" },
        { "type": "input_image", "image_url": "data:image/png;base64,aGVsbG8=" }
    ]);

    assert_eq!(extract_text_content(Some(&content)), "第一段\n第二段");
}

#[test]
fn normalize_openai_responses_request_defaults_to_claude_sonnet_45() {
    let payload = json!({
        "input": [
            {
                "role": "user",
                "content": "hello"
            }
        ]
    });

    let converted =
        normalize_openai_responses_request(&payload).expect("responses payload should convert");
    assert_eq!(converted.model, "claude-sonnet-4-5-20250929");
}

#[test]
fn normalize_openai_responses_request_keeps_tools_tool_choice_and_function_call_items() {
    let payload = json!({
        "model": "claude-sonnet-4",
        "tool_choice": { "type": "function", "name": "search_docs" },
        "tools": [
            {
                "type": "function",
                "name": "search_docs",
                "description": "搜索文档",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "q": { "type": "string" }
                    },
                    "required": ["q"]
                }
            }
        ],
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [
                    { "type": "input_text", "text": "先检索 gateway" }
                ]
            },
            {
                "type": "function_call",
                "call_id": "call_1",
                "name": "search_docs",
                "arguments": "{\"q\":\"gateway\"}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "命中结果"
            }
        ]
    });

    let converted =
        normalize_openai_responses_request(&payload).expect("responses payload should convert");

    assert_eq!(
        converted.tool_choice,
        Some(json!({ "type": "function", "name": "search_docs" }))
    );
    assert_eq!(converted.tools.as_ref().map(Vec::len), Some(1));
    // 工具名会被规范化为 camelCase（Kiro API 要求）
    assert_eq!(
        converted
            .tools
            .as_ref()
            .and_then(|items| items.first())
            .map(|tool| tool.function.name.as_str()),
        Some("searchDocs")
    );
    assert_eq!(converted.messages.len(), 3);
    assert_eq!(converted.messages[0].role, "user");
    assert_eq!(
        converted.messages[0].content,
        Some(json!([
            { "type": "input_text", "text": "先检索 gateway" }
        ]))
    );
    // 工具调用名在归一化阶段保持原样（构造上游 payload 时才做 camelCase 转换）
    assert_eq!(
        converted.messages[1]
            .tool_calls
            .as_ref()
            .and_then(|items| items.first())
            .map(|call| call.function.name.as_str()),
        Some("search_docs")
    );
    assert_eq!(converted.messages[2].role, "tool");
    assert_eq!(
        converted.messages[2].tool_call_id.as_deref(),
        Some("call_1")
    );
    assert_eq!(converted.messages[2].content, Some(json!("命中结果")));
}

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

#[test]
fn normalize_openai_responses_maps_developer_and_reasoning_and_skips_unknown_type() {
    let payload = json!({
        "model": "claude-sonnet-4",
        "input": [
            { "role": "developer", "content": "系统约束" },
            {
                "type": "reasoning",
                "summary": [{ "type": "summary_text", "text": "思考过程" }],
                "content": [{ "type": "reasoning_text", "text": "思考过程" }]
            },
            {
                "type": "function_call",
                "call_id": "call_2",
                "name": "lookup",
                "arguments": "{}"
            },
            {
                "type": "function_call_output",
                "call_id": "call_2",
                "output": "ok"
            },
            { "type": "totally_unknown_item", "foo": 1 }
        ]
    });

    let converted =
        normalize_openai_responses_request(&payload).expect("responses should convert");
    assert_eq!(converted.messages[0].role, "system");
    assert_eq!(converted.messages[0].content, Some(json!("系统约束")));
    assert_eq!(converted.messages[1].role, "assistant");
    assert!(converted.messages[1].metadata.is_some());
    assert_eq!(
        converted.messages[2]
            .tool_calls
            .as_ref()
            .and_then(|c| c.first())
            .map(|c| c.function.name.as_str()),
        Some("lookup")
    );
    assert_eq!(converted.messages[3].role, "tool");
    assert_eq!(converted.messages[3].tool_call_id.as_deref(), Some("call_2"));
    // unknown type 被跳过，不伪造 user 文本
    assert_eq!(converted.messages.len(), 4);
}

#[test]
fn normalize_tool_choice_maps_anthropic_any_and_tool() {
    let tools = Some(vec![Tool {
        tool_type: "function".to_string(),
        function: crate::gateway::models::ToolFunction {
            name: "search_docs".to_string(),
            description: Some("搜索".to_string()),
            parameters: Some(json!({"type": "object"})),
        },
        cache_control: None,
    }]);

    assert_eq!(
        normalize_tool_choice(&Some(json!({ "type": "any" })), &tools).unwrap(),
        Some(json!({ "type": "required" }))
    );
    assert_eq!(
        normalize_tool_choice(
            &Some(json!({ "type": "tool", "name": "search_docs" })),
            &tools
        )
        .unwrap(),
        Some(json!({ "type": "function", "name": "search_docs" }))
    );
    assert_eq!(
        normalize_tool_choice(&Some(json!({ "type": "required" })), &tools).unwrap(),
        Some(json!({ "type": "required" }))
    );
    let err = normalize_tool_choice(&Some(json!({ "type": "mystery" })), &tools)
        .expect_err("unknown type should fail");
    assert!(err.contains("不支持的 tool_choice.type"));
}

#[test]
fn normalize_openai_responses_request_preserves_assistant_message_metadata() {
    let payload = json!({
        "model": "claude-sonnet-4",
        "input": [
            {
                "type": "message",
                "role": "assistant",
                "id": "msg_history_1",
                "cachePoint": { "type": "default" },
                "content": [
                    { "type": "output_text", "text": "历史回答" },
                    { "type": "reasoning", "summary": "内部推理" }
                ]
            },
            {
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": "继续" }]
            }
        ]
    });

    let converted =
        normalize_openai_responses_request(&payload).expect("responses payload should convert");

    assert_eq!(converted.messages.len(), 2);
    assert_eq!(converted.messages[0].role, "assistant");
    assert_eq!(
        converted.messages[0].metadata,
        Some(json!({
            "messageId": "msg_history_1",
            "cachePoint": { "type": "default" },
            "reasoningContent": {
                "reasoningText": {
                    "text": "内部推理"
                }
            }
        }))
    );
}

#[tokio::test]
async fn build_kiro_payload_preserves_responses_tool_choice() {
    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!("hello")),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        stream: false,
        max_tokens: Some(1024),
        temperature: None,
        top_p: None,
        stop: None,
        tools: Some(vec![Tool {
            tool_type: "function".to_string(),
            function: crate::gateway::models::ToolFunction {
                name: "search_docs".to_string(),
                description: Some("搜索文档".to_string()),
                parameters: Some(json!({
                    "type": "object",
                    "properties": { "q": { "type": "string" } }
                })),
            },
            cache_control: None,
        }]),
        tool_choice: Some(json!({ "type": "function", "name": "search_docs" })),
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect("payload should build");

    // Kiro API 实际请求中不包含 tool_choice 字段，
    // tool_choice 由网关消费但不传递给上游 —— 仅验证 tools 正常转发即可
    let context = payload
        .conversation_state
        .current_message
        .user_input_message
        .user_input_message_context
        .as_ref()
        .expect("tools context should exist");

    let kiro_tools = context.tools.as_ref().expect("tools should be present");
    assert_eq!(kiro_tools.len(), 1);
}

#[tokio::test]
async fn build_kiro_payload_includes_tools_when_current_message_has_tool_results() {
    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!([{
                "type": "tool_result",
                "tool_use_id": "toolu_123",
                "content": "result text"
            }])),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        stream: false,
        max_tokens: Some(1024),
        temperature: None,
        top_p: None,
        stop: None,
        tools: Some(vec![Tool {
            tool_type: "function".to_string(),
            function: crate::gateway::models::ToolFunction {
                name: "search_docs".to_string(),
                description: Some("搜索文档".to_string()),
                parameters: Some(json!({
                    "type": "object",
                    "properties": { "q": { "type": "string" } }
                })),
            },
            cache_control: None,
        }]),
        tool_choice: None,
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect("payload should build");

    let context = payload
        .conversation_state
        .current_message
        .user_input_message
        .user_input_message_context
        .as_ref()
        .expect("tool_results context should exist");

    assert!(
        payload
            .conversation_state
            .current_message
            .user_input_message
            .content
            .is_empty(),
        "tool result continuation content must be empty"
    );
    assert_eq!(
        context.tools.as_ref().map(Vec::len),
        Some(1),
        "Kiro IDE includes tools with current toolResults"
    );
    assert_eq!(context.tool_results.as_ref().map(Vec::len), Some(1));
}

#[tokio::test]
async fn build_kiro_payload_orders_current_tool_results_like_previous_tool_uses() {
    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![
            NormalizedMessage {
                role: "user".to_string(),
                content: Some(json!("search twice")),
                tool_calls: None,
                tool_call_id: None,
                metadata: None,
            },
            NormalizedMessage {
                role: "assistant".to_string(),
                content: None,
                tool_calls: Some(vec![
                    ToolCall {
                        id: "tooluse_a".to_string(),
                        call_type: "function".to_string(),
                        function: ToolCallFunction {
                            name: "searchPathnamesOnly".to_string(),
                            arguments: "{}".to_string(),
                        },
                    },
                    ToolCall {
                        id: "tooluse_b".to_string(),
                        call_type: "function".to_string(),
                        function: ToolCallFunction {
                            name: "runTerminalCmd".to_string(),
                            arguments: "{}".to_string(),
                        },
                    },
                ]),
                tool_call_id: None,
                metadata: None,
            },
            NormalizedMessage {
                role: "user".to_string(),
                content: Some(json!([
                    {
                        "type": "tool_result",
                        "tool_use_id": "tooluse_b",
                        "content": "second result"
                    },
                    {
                        "type": "tool_result",
                        "tool_use_id": "tooluse_a",
                        "content": "first result"
                    }
                ])),
                tool_calls: None,
                tool_call_id: None,
                metadata: None,
            },
        ],
        stream: false,
        max_tokens: Some(1024),
        temperature: None,
        top_p: None,
        stop: None,
        tools: None,
        tool_choice: None,
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect("payload should build");

    let context = payload
        .conversation_state
        .current_message
        .user_input_message
        .user_input_message_context
        .as_ref()
        .expect("tool_results context should exist");

    let ids: Vec<_> = context
        .tool_results
        .as_ref()
        .expect("tool_results should exist")
        .iter()
        .map(|result| result.tool_use_id.as_str())
        .collect();

    assert_eq!(ids, vec!["tooluse_a", "tooluse_b"]);
    assert!(
        payload
            .conversation_state
            .current_message
            .user_input_message
            .content
            .is_empty(),
        "tool result continuation content must be empty"
    );
}

#[tokio::test]
async fn build_kiro_payload_reuses_previous_response_id_as_conversation_id() {
    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!("继续")),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        stream: false,
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop: None,
        tools: None,
        tool_choice: None,
        previous_response_id: Some("resp_prev_123".to_string()),
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect("payload should build");

    assert_eq!(payload.conversation_state.conversation_id, "resp_prev_123");
}

#[tokio::test]
async fn build_kiro_payload_rejects_unknown_tool_choice_function() {
    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!("hello")),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        stream: false,
        max_tokens: Some(1024),
        temperature: None,
        top_p: None,
        stop: None,
        tools: Some(vec![Tool {
            tool_type: "function".to_string(),
            function: crate::gateway::models::ToolFunction {
                name: "search_docs".to_string(),
                description: Some("搜索文档".to_string()),
                parameters: Some(json!({
                    "type": "object",
                    "properties": { "q": { "type": "string" } }
                })),
            },
            cache_control: None,
        }]),
        tool_choice: Some(json!({ "type": "function", "name": "missing_tool" })),
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let error = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect_err("unknown tool choice should fail");

    assert!(error.contains("tool_choice 指定的工具不存在"));
}

#[tokio::test]
async fn build_kiro_payload_extracts_base64_images() {
    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!([
                {
                    "type": "text",
                    "text": "看图回答"
                },
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "aGVsbG8="
                    }
                }
            ])),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        stream: false,
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop: None,
        tools: None,
        tool_choice: None,
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect("payload should build");
    let current = &payload
        .conversation_state
        .current_message
        .user_input_message;

    assert_eq!(current.images.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        current
            .images
            .as_ref()
            .and_then(|images| images.first())
            .map(|image| image.format.as_str()),
        Some("png")
    );
}

#[tokio::test]
async fn build_kiro_payload_rejects_private_remote_images() {
    let expected_bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    listener
        .set_nonblocking(true)
        .expect("listener should set nonblocking");
    let address = format!(
        "http://{}",
        listener.local_addr().expect("local addr should resolve")
    );

    let handle = thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0u8; 1024];
                    let _ = stream.read(&mut buffer);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        expected_bytes.len()
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("headers should write");
                    stream
                        .write_all(&expected_bytes)
                        .expect("body should write");
                    return true;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        return false;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(_) => return false,
            }
        }
    });

    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!([
                {
                    "type": "text",
                    "text": "看图回答"
                },
                {
                    "type": "input_image",
                    "image_url": format!("{address}/sample.png")
                }
            ])),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        stream: false,
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop: None,
        tools: None,
        tool_choice: None,
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect("payload should build");
    assert!(
        !handle.join().expect("server thread should finish"),
        "client should not fetch private image"
    );
    let current = &payload
        .conversation_state
        .current_message
        .user_input_message;

    assert!(current.images.as_ref().map(Vec::is_empty).unwrap_or(true));
}

#[tokio::test]
async fn build_kiro_payload_rejects_oversized_data_url_images() {
    let oversized = STANDARD.encode(vec![0u8; 6 * 1024 * 1024]);
    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!([
                {
                    "type": "text",
                    "text": "看图回答"
                },
                {
                    "type": "input_image",
                    "image_url": format!("data:image/png;base64,{oversized}")
                }
            ])),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        stream: false,
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop: None,
        tools: None,
        tool_choice: None,
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect("payload should build");
    let current = &payload
        .conversation_state
        .current_message
        .user_input_message;

    assert!(current.images.as_ref().map(Vec::is_empty).unwrap_or(true));
}

#[tokio::test]
async fn build_kiro_payload_preserves_assistant_message_metadata() {
    let request = NormalizedRequest {
        model: "claude-sonnet-4-5-20250929".to_string(),
        messages: vec![
            NormalizedMessage {
                role: "assistant".to_string(),
                content: Some(json!([
                    { "type": "output_text", "text": "历史回答" },
                    { "type": "reasoning", "summary": "内部推理" }
                ])),
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    call_type: "function".to_string(),
                    function: ToolCallFunction {
                        name: "search_docs".to_string(),
                        arguments: "{\"q\":\"gateway\"}".to_string(),
                    },
                }]),
                tool_call_id: None,
                metadata: Some(json!({
                    "reasoningContent": {
                        "reasoningText": {
                            "text": "内部推理",
                            "signature": "sig_1"
                        }
                    },
                    "references": [
                        {
                            "licenseName": "MIT",
                            "repository": "repo",
                            "url": "https://example.com/ref"
                        }
                    ],
                    "supplementaryWebLinks": [
                        {
                            "url": "https://example.com",
                            "title": "example",
                            "snippet": "snippet"
                        }
                    ],
                    "followupPrompt": {
                        "content": "继续",
                        "userIntent": "SHOW_EXAMPLES"
                    },
                    "messageId": "msg_123",
                    "cachePoint": {
                        "type": "default"
                    }
                })),
            },
            NormalizedMessage {
                role: "user".to_string(),
                content: Some(Value::String("继续".to_string())),
                tool_calls: None,
                tool_call_id: None,
                metadata: None,
            },
        ],
        stream: false,
        max_tokens: None,
        temperature: None,
        top_p: None,
        stop: None,
        tools: None,
        tool_choice: None,
        previous_response_id: None,
        thinking: None,
        include_usage: false,
        tool_name_map: Default::default(),
    };

    let payload = build_kiro_payload(&Client::new(), &request, None, None)
        .await
        .expect("payload should build");
    let history = payload
        .conversation_state
        .history
        .expect("history should exist");

    // history[0] 是 sanitize_history 补的占位 User("Hello")，assistant 消息在 [1]
    match &history[1] {
        HistoryItem::Assistant {
            assistant_response_message,
        } => {
            assert_eq!(assistant_response_message.content, "历史回答");
            assert_eq!(
                assistant_response_message.reasoning_content,
                Some(json!({
                    "reasoningText": {
                        "text": "内部推理",
                        "signature": "sig_1"
                    }
                }))
            );
            assert_eq!(
                assistant_response_message.references,
                Some(json!([
                    {
                        "licenseName": "MIT",
                        "repository": "repo",
                        "url": "https://example.com/ref"
                    }
                ]))
            );
            assert_eq!(
                assistant_response_message.supplementary_web_links,
                Some(json!([
                    {
                        "url": "https://example.com",
                        "title": "example",
                        "snippet": "snippet"
                    }
                ]))
            );
            assert_eq!(
                assistant_response_message.followup_prompt,
                Some(json!({
                    "content": "继续",
                    "userIntent": "SHOW_EXAMPLES"
                }))
            );
            assert_eq!(
                assistant_response_message.message_id.as_deref(),
                Some("msg_123")
            );
            assert_eq!(
                assistant_response_message.cache_point,
                Some(json!({ "type": "default" }))
            );
        }
        other => panic!("unexpected history item: {other:?}"),
    }
}

#[test]
fn get_internal_model_id_normalizes_versioned_public_model_names() {
    assert_eq!(
        get_internal_model_id("claude-sonnet-4-5-20250929")
            .expect("versioned sonnet 4.5 should map"),
        "claude-sonnet-4.5"
    );
    assert_eq!(
        get_internal_model_id("claude-sonnet-4-6").expect("sonnet 4.6 alias should map"),
        "claude-sonnet-4.6"
    );
    assert_eq!(
        get_internal_model_id("claude-sonnet-4-6-20260217")
            .expect("versioned sonnet 4.6 should map"),
        "claude-sonnet-4.6"
    );
    assert_eq!(
        get_internal_model_id("claude-opus-4-6").expect("opus 4.6 alias should map"),
        "claude-opus-4.6"
    );
    assert_eq!(
        get_internal_model_id("claude-opus-4-6-20260205")
            .expect("versioned opus 4.6 should map"),
        "claude-opus-4.6"
    );
    assert_eq!(
        get_internal_model_id("claude-haiku-4-5-20251001")
            .expect("versioned haiku 4.5 should map"),
        "claude-haiku-4.5"
    );
    assert_eq!(
        get_internal_model_id("claude-sonnet-latest")
            .expect("latest sonnet alias should resolve to Sonnet 5"),
        "claude-sonnet-5"
    );
    // "sonnet" 默认指向当前最新的 Sonnet（Sonnet 4.6）
    assert_eq!(
        get_internal_model_id("sonnet").expect("plain sonnet alias should resolve"),
        "claude-sonnet-4.6"
    );
}

#[test]
fn get_available_models_includes_claude_46_official_ids() {
    let model_ids: Vec<_> = get_available_models()
        .into_iter()
        .map(|model| model.id)
        .collect();

    // Kiro ListAvailableModels API 实际返回的是带点号的 ID。
    // 注意：Claude 模型只保留 -thinking 变体，不带后缀的已下线。
    assert!(model_ids
        .iter()
        .any(|id| id == "claude-opus-4.6-thinking"));
    assert!(model_ids
        .iter()
        .any(|id| id == "claude-sonnet-4.6-thinking"));
    // 4.6 之上的新版本
    assert!(model_ids.iter().any(|id| id == "claude-opus-4.7-thinking"));
    assert!(model_ids.iter().any(|id| id == "claude-opus-4.8-thinking"));
    assert!(model_ids
        .iter()
        .any(|id| id == "claude-sonnet-5-thinking"));
}

#[test]
fn get_internal_model_id_with_fallback_caps_to_sonnet_45_when_it_is_highest_available() {
    let available_models = vec![
        "auto".to_string(),
        "claude-sonnet-4.5".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-haiku-4.5".to_string(),
        "deepseek-3.2".to_string(),
    ];

    for requested in [
        "claude-sonnet-5",
        "claude-opus-4.8",
        "claude-opus-4.7",
        "claude-opus-4.6",
        "claude-opus-4.5",
        "claude-sonnet-4.6",
    ] {
        assert_eq!(
            get_internal_model_id_with_fallback(requested, &available_models)
                .expect("model should fallback"),
            "claude-sonnet-4.5",
            "{requested} should be capped to highest available Claude model"
        );
    }
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

#[tokio::test]
async fn extract_images_works_with_preserved_image_array() {
    // 测试：extract_images 能从保留的数组中提取图片
    let content = json!([
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
    ]);

    let client = Client::new();
    let images = extract_images(&client, Some(&content)).await;

    // 验证成功提取了图片
    assert_eq!(images.len(), 1, "should extract 1 image");
    assert_eq!(images[0].format, "png", "image format should be png");

    // 验证图片数据
    match &images[0].source {
        ImageSource::Bytes { bytes } => {
            assert!(!bytes.is_empty(), "image bytes should not be empty");
            assert_eq!(
                bytes,
                "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
                "image bytes should match"
            );
        }
        ImageSource::Other { .. } => {
            panic!("expected ImageSource::Bytes, got ImageSource::Other");
        }
    }
}

#[test]
fn normalize_openai_responses_request_preserves_compaction_items() {
    // 测试：Responses API 的 compaction item 应该被保留
    let payload = json!({
        "model": "gpt-5",
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": "Hello"
            },
            {
                "type": "message",
                "role": "assistant",
                "content": "Hi there!"
            },
            {
                "type": "compaction",
                "data": "encrypted_compaction_data_here"
            },
            {
                "type": "message",
                "role": "user",
                "content": "Continue"
            }
        ]
    });

    let normalized =
        normalize_openai_responses_request(&payload).expect("should normalize successfully");

    // 验证消息数量：user + assistant + compaction + user = 4
    assert_eq!(normalized.messages.len(), 4, "should have 4 messages");

    // 验证 compaction item 被保留为 system 消息
    assert_eq!(
        normalized.messages[2].role, "system",
        "compaction should be system role"
    );
    assert!(
        normalized.messages[2]
            .metadata
            .as_ref()
            .and_then(|m| m.get("is_compaction"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        "compaction should have is_compaction metadata"
    );

    // 验证 compaction 内容被原样保留
    let compaction_content = normalized.messages[2].content.as_ref().unwrap();
    assert_eq!(
        compaction_content.get("type").and_then(|v| v.as_str()),
        Some("compaction"),
        "compaction type should be preserved"
    );
    assert_eq!(
        compaction_content.get("data").and_then(|v| v.as_str()),
        Some("encrypted_compaction_data_here"),
        "compaction data should be preserved"
    );
}
