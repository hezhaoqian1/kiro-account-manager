//! 下游响应体与流式事件构造的回归测试。

use super::*;

#[test]
fn build_responses_response_emits_kiro_citation_annotations() {
    let aggregated = stream::AggregatedKiroResponse {
        text: "Hello Rust".to_string(),
        thinking: String::new(),
        thinking_signature: None,
        tool_calls: Vec::new(),
        input_tokens: 3,
        output_tokens: 5,
        context_usage_percentage: None,
        citations: vec![stream::AggregatedCitation {
            text: Some("Rust".to_string()),
            link: "https://example.com/rust".to_string(),
            target: json!({ "range": { "start": 6, "end": 10 } }),
        }],
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        metering_usage: None,
    };

    let response = build_responses_response_with_ids(
        "gpt-5.4",
        &aggregated,
        "resp_test",
        "msg_test",
        123,
        None,
    );

    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["type"],
        "url_citation"
    );
    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["start_index"],
        6
    );
    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["end_index"],
        10
    );
    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["url"],
        "https://example.com/rust"
    );
    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["citationText"],
        "Rust"
    );
    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["citationLink"],
        "https://example.com/rust"
    );
    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["target"]["range"]["start"],
        6
    );
    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["target"]["range"]["end"],
        10
    );
    assert!(response["output"][0]["content"][0]["annotations"][0]["title"].is_null());
}

#[test]
fn build_responses_response_omits_guessed_range_for_location_citations() {
    let aggregated = stream::AggregatedKiroResponse {
        text: "Hello Rust".to_string(),
        thinking: String::new(),
        thinking_signature: None,
        tool_calls: Vec::new(),
        input_tokens: 3,
        output_tokens: 5,
        context_usage_percentage: None,
        citations: vec![stream::AggregatedCitation {
            text: Some("Rust".to_string()),
            link: "https://example.com/rust".to_string(),
            target: json!({ "location": 6 }),
        }],
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        metering_usage: None,
    };

    let response = build_responses_response_with_ids(
        "gpt-4.1",
        &aggregated,
        "resp_test",
        "msg_test",
        123,
        None,
    );

    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["type"],
        "url_citation"
    );
    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["citationText"],
        "Rust"
    );
    assert_eq!(
        response["output"][0]["content"][0]["annotations"][0]["target"]["location"],
        6
    );
    assert!(response["output"][0]["content"][0]["annotations"][0]["start_index"].is_null());
    assert!(response["output"][0]["content"][0]["annotations"][0]["end_index"].is_null());
    assert!(response["output"][0]["content"][0]["annotations"][0]["title"].is_null());
}

#[test]
fn build_anthropic_response_maps_kiro_citations_into_sdk_shape() {
    let aggregated = stream::AggregatedKiroResponse {
        text: "Hello Rust".to_string(),
        thinking: String::new(),
        thinking_signature: None,
        tool_calls: Vec::new(),
        input_tokens: 3,
        output_tokens: 5,
        context_usage_percentage: None,
        citations: vec![stream::AggregatedCitation {
            text: Some("Rust".to_string()),
            link: "https://example.com/rust".to_string(),
            target: json!({ "range": { "start": 6, "end": 10 } }),
        }],
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        metering_usage: None,
    };

    let response = build_anthropic_response("claude-sonnet-4-5", &aggregated);

    assert_eq!(response["content"][0]["type"], "text");
    assert_eq!(
        response["content"][0]["citations"][0]["type"],
        "char_location"
    );
    assert_eq!(
        response["content"][0]["citations"][0]["start_char_index"],
        6
    );
    assert_eq!(response["content"][0]["citations"][0]["end_char_index"], 10);
    assert_eq!(response["content"][0]["citations"][0]["cited_text"], "Rust");
    assert_eq!(
        response["content"][0]["citations"][0]["document_title"],
        "https://example.com/rust"
    );
    assert!(response["content"][0]["citations"][0]["file_id"].is_null());
}

#[test]
fn build_stream_responses_completed_event_keeps_citations_and_tool_calls() {
    let aggregated = stream::AggregatedKiroResponse {
        text: "Hello Rust".to_string(),
        thinking: String::new(),
        thinking_signature: None,
        tool_calls: vec![(
            "call_1".to_string(),
            "search_docs".to_string(),
            "{\"q\":\"rust\"}".to_string(),
        )],
        input_tokens: 3,
        output_tokens: 5,
        context_usage_percentage: None,
        citations: vec![stream::AggregatedCitation {
            text: Some("Rust".to_string()),
            link: "https://example.com/rust".to_string(),
            target: json!({ "range": { "start": 6, "end": 10 } }),
        }],
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        metering_usage: None,
    };

    let event = build_stream_responses_completed_event(
        "gpt-4.1",
        &aggregated,
        "resp_test",
        "msg_test",
        123,
        None,
    );

    assert_eq!(event["type"], "response.completed");
    assert_eq!(event["response"]["output_text"], "Hello Rust");
    assert_eq!(
        event["response"]["output"][0]["content"][0]["annotations"][0]["citationText"],
        "Rust"
    );
    assert!(event["response"]["output"][0]["content"][0]["annotations"][0]["title"].is_null());
    assert_eq!(
        event["response"]["output"][0]["content"][1]["type"],
        "function_call"
    );
    assert_eq!(
        event["response"]["output"][0]["content"][1]["call_id"],
        "call_1"
    );
}

#[test]
fn build_stream_responses_done_events_use_expected_shape() {
    let function_done = build_stream_responses_function_call_arguments_done_event(
        "resp_test",
        "call_1",
        "{\"q\":\"rust\"}",
    );
    let text_done = build_stream_responses_output_text_done_event("resp_test", "Hello Rust");
    let reasoning_done = build_stream_responses_reasoning_done_event("resp_test", "Think");

    assert_eq!(
        function_done["type"],
        "response.function_call_arguments.done"
    );
    assert_eq!(function_done["response_id"], "resp_test");
    assert_eq!(function_done["call_id"], "call_1");
    assert_eq!(function_done["arguments"], "{\"q\":\"rust\"}");

    assert_eq!(text_done["type"], "response.output_text.done");
    assert_eq!(text_done["response_id"], "resp_test");
    assert_eq!(text_done["text"], "Hello Rust");

    assert_eq!(reasoning_done["type"], "response.reasoning.done");
    assert_eq!(reasoning_done["response_id"], "resp_test");
    assert_eq!(reasoning_done["text"], "Think");
}
