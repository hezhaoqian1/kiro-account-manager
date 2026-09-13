//! 请求归一化的回归测试。

use super::*;

#[test]
fn normalize_request_accepts_openai_chat_payloads() {
    let responses_payload = json!({
        "model": "claude-sonnet-4",
        "stream": true,
        "previous_response_id": "resp_prev_123",
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

    let chat_payload = json!({
        "model": "claude-sonnet-4",
        "stream": true,
        "tool_choice": { "type": "function", "name": "search_docs" },
        "tools": [
            {
                "type": "function",
                "function": {
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
            }
        ],
        "messages": [
            {
                "role": "user",
                "content": [
                    { "type": "input_text", "text": "先检索 gateway" }
                ]
            },
            {
                "role": "assistant",
                "tool_calls": [
                    {
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "search_docs",
                            "arguments": "{\"q\":\"gateway\"}"
                        }
                    }
                ]
            },
            {
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "命中结果"
            }
        ]
    });

    let responses_request = normalize_request(ResponseFormat::Responses, &responses_payload)
        .expect("responses payload should normalize");
    let chat_request = normalize_request(ResponseFormat::OpenAI, &chat_payload)
        .expect("chat payload should normalize through the OpenAI Chat protocol");

    assert_eq!(responses_request.model, "claude-sonnet-4");
    assert!(responses_request.stream);
    assert_eq!(
        responses_request.previous_response_id.as_deref(),
        Some("resp_prev_123")
    );
    assert_eq!(
        responses_request.tool_choice,
        Some(json!({ "type": "function", "name": "search_docs" }))
    );
    assert_eq!(responses_request.tools.as_ref().map(Vec::len), Some(1));
    assert_eq!(responses_request.tools.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        responses_request
            .tools
            .as_ref()
            .and_then(|items| items.first())
            .map(|tool| tool.function.name.as_str()),
        Some("searchDocs")
    );
    assert_eq!(responses_request.messages.len(), 3);
    assert_eq!(
        responses_request.messages[1]
            .tool_calls
            .as_ref()
            .and_then(|items| items.first())
            .map(|call| &call.function.arguments),
        Some(&"{\"q\":\"gateway\"}".to_string())
    );
    assert_eq!(
        responses_request.messages[2].content,
        Some(json!("命中结果"))
    );
    assert_eq!(chat_request.model, responses_request.model);
    assert_eq!(chat_request.stream, responses_request.stream);
    assert_eq!(chat_request.tool_choice, responses_request.tool_choice);
    assert_eq!(chat_request.tools.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        chat_request.messages.len(),
        responses_request.messages.len()
    );
    assert_eq!(
        chat_request.messages[1]
            .tool_calls
            .as_ref()
            .and_then(|items| items.first())
            .map(|call| &call.function.arguments),
        Some(&"{\"q\":\"gateway\"}".to_string())
    );
    // 工具结果已结构化为 tool_result 内容块，不再被双重序列化成字符串
    assert_eq!(
        chat_request.messages[2].content,
        Some(json!([{
            "type": "tool_result",
            "tool_use_id": "call_1",
            "content": "命中结果"
        }]))
    );
}
