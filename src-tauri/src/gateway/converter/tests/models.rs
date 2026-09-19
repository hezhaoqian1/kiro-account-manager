//! 模型 ID 归一化与可用模型列表的回归测试。

use super::*;

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
    // 对外列表使用标准 Claude 名称，thinking 由网关默认启用。
    assert!(model_ids
        .iter()
        .any(|id| id == "claude-opus-4.6"));
    assert!(model_ids
        .iter()
        .any(|id| id == "claude-sonnet-4.6"));
    // 4.6 之上的新版本
    assert!(model_ids.iter().any(|id| id == "claude-opus-4.7"));
    assert!(model_ids.iter().any(|id| id == "claude-opus-4.8"));
    assert!(model_ids.iter().any(|id| id == "claude-sonnet-5"));
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
fn get_internal_model_id_with_fallback_accepts_kiro_thinking_suffix() {
    let available_models = vec!["claude-sonnet-4.5-thinking".to_string()];

    assert_eq!(
        get_internal_model_id_with_fallback("claude-sonnet-4.5", &available_models)
            .expect("thinking variant should satisfy the model lookup"),
        "claude-sonnet-4.5"
    );
}

#[test]
fn standard_claude_model_defaults_to_thinking_mode() {
    let request = build_normalized_request_from_payload(
        &json!({}),
        "claude-sonnet-4.5".to_string(),
        vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(json!("hello")),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        None,
        std::collections::HashMap::new(),
    );

    assert_eq!(request.thinking.as_ref().map(|value| value.thinking_type.as_str()), Some("enabled"));
    assert_eq!(request.thinking.as_ref().map(|value| value.budget_tokens), Some(20_000));
}
