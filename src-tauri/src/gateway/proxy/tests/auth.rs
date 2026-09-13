//! 客户端鉴权的回归测试。

use super::*;

#[test]
fn verify_client_auth_accepts_any_configured_client_api_key() {
    let config = GatewayConfig {
        access_token: Some("sk-primary".to_string()),
        client_api_keys: vec!["sk-primary".to_string(), "sk-secondary".to_string()],
        ..GatewayConfig::default()
    };

    let mut bearer_headers = HeaderMap::new();
    bearer_headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer sk-secondary"),
    );
    assert!(verify_client_auth(&bearer_headers, &config).is_ok());

    let mut x_api_key_headers = HeaderMap::new();
    x_api_key_headers.insert("x-api-key", HeaderValue::from_static("sk-primary"));
    assert!(verify_client_auth(&x_api_key_headers, &config).is_ok());

    let mut invalid_headers = HeaderMap::new();
    invalid_headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer sk-unknown"),
    );
    assert!(verify_client_auth(&invalid_headers, &config).is_err());
}
