use super::*;

#[test]
fn parse_kiro_event_full_reads_text_tool_and_usage_events() {
    assert_eq!(
        parse_kiro_event_full(r#"{"assistantResponseEvent":{"content":"hello"}}"#),
        Some(KiroEvent::Text("hello".to_string()))
    );
    assert_eq!(
        parse_kiro_event_full(r#"{"toolUseId":"tool_1","name":"search_docs"}"#),
        Some(KiroEvent::ToolUseStart {
            id: "tool_1".to_string(),
            name: "search_docs".to_string(),
        })
    );
    assert_eq!(
        parse_kiro_event_full(r#"{"toolUseId":"tool_1","input":{"q":"gateway"}}"#),
        Some(KiroEvent::ToolUseInputDelta {
            id: "tool_1".to_string(),
            name: None,
            input_delta: "{\"q\":\"gateway\"}".to_string(),
        })
    );
    assert_eq!(
        parse_kiro_event_full(r#"{"toolUseId":"tool_1","stop":true}"#),
        Some(KiroEvent::ToolUseStop {
            id: "tool_1".to_string(),
        })
    );
    assert_eq!(
        parse_kiro_event_full(r#"{"usage":{"inputTokens":12,"outputTokens":34}}"#),
        Some(KiroEvent::Usage {
            input_tokens: 12,
            output_tokens: 34,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
        })
    );
}

#[test]
fn parse_kiro_event_full_reads_reasoning_content() {
    assert_eq!(
        parse_kiro_event_full(r#"{"reasoningContentEvent":{"text":"分析中"}}"#),
        Some(KiroEvent::Thinking("分析中".to_string()))
    );
}

#[test]
fn parse_kiro_event_full_reads_metering_event() {
    assert_eq!(
        parse_kiro_event_full(
            r#"{"meteringEvent":{"unit":"credit","unitPlural":"credits","usage":0.3876425741791045}}"#
        ),
        Some(KiroEvent::Metering {
            unit: "credit".to_string(),
            unit_plural: "credits".to_string(),
            usage: 0.3876425741791045,
        })
    );
}

#[test]
fn parse_kiro_event_full_keeps_tool_name_with_wrapped_input() {
    assert_eq!(
        parse_kiro_event_full(
            r#"{"toolUseEvent":{"toolUseId":"tool_1","name":"server_health","input":"{}"}}"#
        ),
        Some(KiroEvent::ToolUseInputDelta {
            id: "tool_1".to_string(),
            name: Some("server_health".to_string()),
            input_delta: "{}".to_string(),
        })
    );
}

#[test]
fn parse_kiro_event_full_reads_citation_events() {
    assert_eq!(
        parse_kiro_event_full(
            r#"{"target":{"range":{"start":2,"end":5}},"citationText":"Rust","citationLink":"https://example.com/rust"}"#
        ),
        Some(KiroEvent::Citation {
            text: Some("Rust".to_string()),
            link: "https://example.com/rust".to_string(),
            target: serde_json::json!({ "range": { "start": 2, "end": 5 } }),
        })
    );
    assert_eq!(
        parse_kiro_event_full(
            r#"{"target":{"location":6},"citationText":"Rust","citationLink":"https://example.com/location"}"#
        ),
        Some(KiroEvent::Citation {
            text: Some("Rust".to_string()),
            link: "https://example.com/location".to_string(),
            target: serde_json::json!({ "location": 6 }),
        })
    );
}
