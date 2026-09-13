use super::*;

#[test]
fn aggregate_kiro_response_from_payloads_keeps_name_when_input_arrives_first() {
    let payloads = vec![
        r#"{"toolUseEvent":{"toolUseId":"tool_1","name":"server_health","input":"{}"}}"#
            .to_string(),
        r#"{"toolUseEvent":{"toolUseId":"tool_1","stop":true}}"#.to_string(),
    ];

    let aggregated = aggregate_kiro_response_from_payloads(&payloads);

    assert_eq!(
        aggregated.tool_calls,
        vec![(
            "tool_1".to_string(),
            "server_health".to_string(),
            "{}".to_string()
        )]
    );
}

#[test]
fn aggregate_kiro_response_from_payloads_concatenates_string_input_deltas() {
    let payloads = vec![
        r#"{"toolUseEvent":{"toolUseId":"tool_1","name":"todo_write","input":"{\"todos\":[{\"content\":\""}}"#.to_string(),
        r#"{"toolUseEvent":{"toolUseId":"tool_1","input":"plan"}}"#.to_string(),
        r#"{"toolUseEvent":{"toolUseId":"tool_1","input":"a\",\"status\":\"pending\"}]}"}}"#.to_string(),
        r#"{"toolUseEvent":{"toolUseId":"tool_1","stop":true}}"#.to_string(),
    ];

    let aggregated = aggregate_kiro_response_from_payloads(&payloads);

    assert_eq!(aggregated.tool_calls.len(), 1);
    assert_eq!(aggregated.tool_calls[0].0, "tool_1");
    assert_eq!(aggregated.tool_calls[0].1, "todo_write");
    assert_eq!(
        aggregated.tool_calls[0].2,
        r#"{"todos":[{"content":"plana","status":"pending"}]}"#
    );
}

#[test]
fn aggregate_kiro_response_from_payloads_collects_citations() {
    let payloads = vec![
        r#"{"assistantResponseEvent":{"content":"Hello Rust"}}"#.to_string(),
        r#"{"target":{"range":{"start":6,"end":10}},"citationText":"Rust","citationLink":"https://example.com/rust"}"#.to_string(),
    ];

    let aggregated = aggregate_kiro_response_from_payloads(&payloads);

    assert_eq!(aggregated.text, "Hello Rust");
    assert_eq!(
        aggregated.citations,
        vec![AggregatedCitation {
            text: Some("Rust".to_string()),
            link: "https://example.com/rust".to_string(),
            target: serde_json::json!({ "range": { "start": 6, "end": 10 } }),
        }]
    );
}

#[test]
fn deduplicate_tool_calls_keeps_latest_args_per_id() {
    let input = vec![
        ("tool_1".to_string(), "search".to_string(), "{}".to_string()),
        (
            "tool_1".to_string(),
            "search".to_string(),
            "{\"q\":\"gateway\"}".to_string(),
        ),
        (
            "tool_2".to_string(),
            "open".to_string(),
            "{\"path\":\"README.md\"}".to_string(),
        ),
    ];

    let deduped = deduplicate_tool_calls(input);
    assert_eq!(deduped.len(), 2);
    assert_eq!(deduped[0].0, "tool_1");
    assert_eq!(deduped[0].2, "{\"q\":\"gateway\"}");
}
