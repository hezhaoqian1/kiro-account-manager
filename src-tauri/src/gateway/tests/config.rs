use super::*;

#[test]
fn rejects_unsupported_region() {
    let config = GatewayConfig {
        region: "moon-east-1".to_string(),
        ..GatewayConfig::default()
    };

    let err = ensure_config_valid(&config).expect_err("unsupported region should fail");
    assert!(err.contains("region 不受支持"));
}

#[test]
fn rejects_local_account_mode_for_gateway() {
    let config = GatewayConfig {
        account_mode: "local".to_string(),
        access_token: Some("sk-test".to_string()),
        ..GatewayConfig::default()
    };

    let err = ensure_config_valid(&config).expect_err("local mode should fail");
    assert!(
        err.contains("不再支持 local 模式"),
        "unexpected error: {err}"
    );
}

#[test]
fn accepts_known_regions() {
    let mut config = GatewayConfig {
        account_mode: "single".to_string(),
        account_id: Some("test-account".to_string()),
        access_token: Some("sk-test".to_string()),
        ..GatewayConfig::default()
    };
    for region in [
        "us-east-1",
        "eu-central-1",
        "us-west-2",
        "ap-northeast-1",
        "ap-southeast-1",
        "us-gov-west-1",
    ] {
        config.region = region.to_string();
        ensure_config_valid(&config).expect("known region should pass validation");
    }
}

#[test]
fn rejects_remote_access_without_api_key() {
    let config = GatewayConfig {
        local_only: false,
        account_mode: "single".to_string(),
        account_id: Some("test-account".to_string()),
        access_token: None,
        ..GatewayConfig::default()
    };

    let err =
        ensure_config_valid(&config).expect_err("remote access without api key should fail");
    assert!(err.contains("API Key"), "unexpected error: {err}");
}

#[test]
fn rejects_remote_access_without_allowlist() {
    let config = GatewayConfig {
        local_only: false,
        account_mode: "single".to_string(),
        account_id: Some("test-account".to_string()),
        access_token: Some("sk-test".to_string()),
        allowed_ips: Vec::new(),
        ..GatewayConfig::default()
    };

    let err =
        ensure_config_valid(&config).expect_err("remote access without allowlist should fail");
    assert!(err.contains("白名单"), "unexpected error: {err}");
}

#[test]
fn normalize_config_promotes_legacy_access_token_to_client_api_keys() {
    let config = GatewayConfig {
        access_token: Some(" sk-primary ".to_string()),
        client_api_keys: Vec::new(),
        ..GatewayConfig::default()
    };

    let normalized = normalize_config(&config);

    assert_eq!(normalized.client_api_keys, vec!["sk-primary".to_string()]);
    assert_eq!(normalized.access_token.as_deref(), Some("sk-primary"));
}

#[test]
fn normalize_config_deduplicates_client_api_keys() {
    let config = GatewayConfig {
        access_token: Some("sk-primary".to_string()),
        client_api_keys: vec![
            " sk-primary ".to_string(),
            "sk-secondary".to_string(),
            "".to_string(),
            "sk-secondary".to_string(),
        ],
        ..GatewayConfig::default()
    };

    let normalized = normalize_config(&config);

    assert_eq!(
        normalized.client_api_keys,
        vec!["sk-primary".to_string(), "sk-secondary".to_string()]
    );
}

#[test]
fn rejects_config_without_any_client_api_keys() {
    let config = GatewayConfig {
        access_token: Some("   ".to_string()),
        client_api_keys: vec!["".to_string(), "   ".to_string()],
        account_mode: "single".to_string(),
        account_id: Some("test-account".to_string()),
        ..GatewayConfig::default()
    };

    let err = ensure_config_valid(&normalize_config(&config))
        .expect_err("missing client api keys should fail");
    assert!(err.contains("客户端 API Key"), "unexpected error: {err}");
}
