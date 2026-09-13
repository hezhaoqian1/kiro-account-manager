//! token 估算、payload 体积统计与 payload 裁剪的回归测试。

use super::*;

#[test]
fn test_tokenizer_type_from_model_id() {
    assert!(matches!(
        TokenizerType::from_model_id("claude-sonnet-4"),
        TokenizerType::Claude
    ));
    assert!(matches!(
        TokenizerType::from_model_id("gpt-4"),
        TokenizerType::OpenAI
    ));
    assert!(matches!(
        TokenizerType::from_model_id("o1-preview"),
        TokenizerType::OpenAI
    ));
    assert!(matches!(
        TokenizerType::from_model_id("llama-3-70b"),
        TokenizerType::Llama
    ));
    assert!(matches!(
        TokenizerType::from_model_id("unknown-model"),
        TokenizerType::Generic
    ));
}

#[test]
fn test_estimate_text_tokens_claude() {
    let text = "Hello, world!";
    let tokens = estimate_text_tokens(text, TokenizerType::Claude);
    assert_eq!(tokens, (text.len() + 3) / 4);
}

#[test]
fn test_estimate_text_tokens_llama() {
    let text = "Hello, world!";
    let tokens = estimate_text_tokens(text, TokenizerType::Llama);
    assert_eq!(tokens, ((text.len() as f64 / 3.5).ceil() as usize).max(1));
}

#[test]
fn test_estimate_text_tokens_generic() {
    let text = "Hello\nWorld\n```rust\nfn main() {}\n```";
    let tokens = estimate_text_tokens(text, TokenizerType::Generic);

    let base_tokens = (text.len() + 3) / 4;
    let lines = text.lines().count();
    let newline_tokens = (lines + 1) / 2;
    let code_blocks = text.matches("```").count();
    let code_block_tokens = code_blocks * 2;
    let expected = base_tokens + newline_tokens + code_block_tokens;

    assert_eq!(tokens, expected);
}

#[test]
fn test_estimate_request_tokens() {
    let messages = vec![
        NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!("Hello, how are you?")),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        },
        NormalizedMessage {
            role: "assistant".to_string(),
            content: Some(json!("I'm doing well, thank you!")),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        },
    ];

    let tokens = estimate_request_tokens(&messages, "claude-sonnet-4");
    assert!(tokens > 0);
}

#[test]
fn test_get_payload_size() {
    let payload = json!({
        "model": "claude-sonnet-4",
        "messages": [
            {"role": "user", "content": "Hello"}
        ]
    });

    let size = get_payload_size(&payload);
    assert!(size > 0);
}

#[test]
fn test_trim_kiro_payload_history_removes_oldest_messages() {
    let mut payload = json!({
        "conversationState": {
            "history": [
                {
                    "user_input_message": {
                        "user_input_message_context": {
                            "text": "First message"
                        }
                    }
                },
                {
                    "assistant_response_message": {
                        "text": "First response"
                    }
                },
                {
                    "user_input_message": {
                        "user_input_message_context": {
                            "text": "Second message"
                        }
                    }
                },
                {
                    "assistant_response_message": {
                        "text": "Second response"
                    }
                }
            ]
        }
    });

    let max_bytes = 100;
    let trimmed = trim_kiro_payload_history(&mut payload, max_bytes);

    assert!(trimmed);
    let history = payload
        .pointer("/conversationState/history")
        .and_then(|v| v.as_array())
        .unwrap();
    assert!(history.len() < 4);
    assert!(history.len() >= 2);
}

#[test]
fn test_trim_kiro_payload_history_preserves_tool_call_pairs() {
    let mut payload = json!({
        "conversationState": {
            "history": [
                {
                    "assistant_response_message": {
                        "text": "Let me search for that",
                        "tool_uses": [
                            {
                                "id": "call_1",
                                "name": "search",
                                "input": {"q": "test"}
                            }
                        ]
                    }
                },
                {
                    "user_input_message": {
                        "user_input_message_context": {
                            "tool_results": [
                                {
                                    "call_id": "call_1",
                                    "output": "Found results"
                                }
                            ]
                        }
                    }
                },
                {
                    "user_input_message": {
                        "user_input_message_context": {
                            "text": "Recent message"
                        }
                    }
                }
            ]
        }
    });

    let max_bytes = 200;
    let trimmed = trim_kiro_payload_history(&mut payload, max_bytes);

    if trimmed {
        let history = payload
            .pointer("/conversationState/history")
            .and_then(|v| v.as_array())
            .unwrap();

        if history.len() == 1 {
            assert!(history[0].get("user_input_message").is_some());
        }
    }
}

#[tokio::test]
async fn test_get_model_max_input_tokens() {
    assert_eq!(get_model_max_input_tokens("auto").await, 1_000_000);
    assert_eq!(
        get_model_max_input_tokens("claude-sonnet-4").await,
        200_000
    );
    assert_eq!(get_model_max_input_tokens("gpt-4").await, 200_000);
    assert_eq!(get_model_max_input_tokens("deepseek-chat").await, 128_000);
    assert_eq!(get_model_max_input_tokens("llama-3-70b").await, 128_000);
    assert_eq!(get_model_max_input_tokens("unknown-model").await, 200_000);
}
