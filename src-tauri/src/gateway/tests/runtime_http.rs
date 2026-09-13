use super::*;

#[tokio::test]
async fn runtime_serves_health_over_real_http() {
    let guard = RuntimeHttpTestGuard::new();
    let port = guard.port;

    let config = runtime_test_gateway_config(port);
    let mut runtime = spawn_runtime(config).await.expect("runtime should start");

    let response = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/health"))
        .bearer_auth("sk-test")
        .send()
        .await
        .expect("health request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    stop_runtime(&mut runtime).await;
}

#[tokio::test]
async fn runtime_serves_models_over_real_http() {
    let guard = RuntimeHttpTestGuard::new();
    let port = guard.port;

    let config = runtime_test_gateway_config(port);
    let mut runtime = spawn_runtime(config).await.expect("runtime should start");

    let response = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/v1/models"))
        .bearer_auth("sk-test")
        .send()
        .await
        .expect("models request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = response
        .json()
        .await
        .expect("models response should be json");
    assert_eq!(payload.get("object").and_then(Value::as_str), Some("list"));
    assert!(
        payload
            .get("data")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty()),
        "models response should include at least one model"
    );
    stop_runtime(&mut runtime).await;
}

#[tokio::test]
async fn runtime_serves_count_tokens_over_real_http() {
    let guard = RuntimeHttpTestGuard::new();
    let port = guard.port;

    let config = runtime_test_gateway_config(port);
    let mut runtime = spawn_runtime(config).await.expect("runtime should start");

    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/v1/messages/count_tokens"))
        .bearer_auth("sk-test")
        .header("content-type", "application/json")
        .body(
            json!({
                "model": "claude-sonnet-4-5-20250929",
                "messages": [{ "role": "user", "content": "hello world" }]
            })
            .to_string(),
        )
        .send()
        .await
        .expect("count tokens request should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = response
        .json()
        .await
        .expect("count tokens response should be json");
    // 该端点估算的是整个序列化请求（含 model / messages 结构），实测 27
    assert_eq!(payload.get("input_tokens").and_then(Value::as_u64), Some(27));
    stop_runtime(&mut runtime).await;
}

#[tokio::test]
async fn runtime_rejects_unauthenticated_health_requests_over_real_http() {
    let guard = RuntimeHttpTestGuard::new();
    let port = guard.port;

    let config = runtime_test_gateway_config(port);
    let mut runtime = spawn_runtime(config).await.expect("runtime should start");

    let response = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/health"))
        .send()
        .await
        .expect("health request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    stop_runtime(&mut runtime).await;
}

#[tokio::test]
async fn runtime_rejects_raw_authorization_header_without_bearer_over_real_http() {
    let guard = RuntimeHttpTestGuard::new();
    let port = guard.port;

    let config = runtime_test_gateway_config(port);
    let mut runtime = spawn_runtime(config).await.expect("runtime should start");

    let response = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/health"))
        .header("Authorization", "sk-test")
        .send()
        .await
        .expect("health request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    stop_runtime(&mut runtime).await;
}

#[tokio::test]
async fn runtime_rejects_unauthenticated_models_requests_over_real_http() {
    let guard = RuntimeHttpTestGuard::new();
    let port = guard.port;

    let config = runtime_test_gateway_config(port);
    let mut runtime = spawn_runtime(config).await.expect("runtime should start");

    let response = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/v1/models"))
        .send()
        .await
        .expect("models request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    stop_runtime(&mut runtime).await;
}

#[tokio::test]
async fn runtime_rejects_unauthenticated_count_tokens_requests_over_real_http() {
    let guard = RuntimeHttpTestGuard::new();
    let port = guard.port;

    let config = runtime_test_gateway_config(port);
    let mut runtime = spawn_runtime(config).await.expect("runtime should start");

    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/v1/messages/count_tokens"))
        .header("content-type", "application/json")
        .body(
            json!({
                "model": "claude-sonnet-4-5-20250929",
                "input": [{ "role": "user", "content": "hello" }]
            })
            .to_string(),
        )
        .send()
        .await
        .expect("count tokens request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    stop_runtime(&mut runtime).await;
}

#[tokio::test]
async fn runtime_rejects_unauthenticated_responses_requests_over_real_http() {
    let guard = RuntimeHttpTestGuard::new();
    let port = guard.port;

    let config = runtime_test_gateway_config(port);
    let mut runtime = spawn_runtime(config).await.expect("runtime should start");

    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/v1/responses"))
        .header("content-type", "application/json")
        .body(
            json!({
                "model": "claude-sonnet-4-5-20250929",
                "input": [{ "role": "user", "content": "hello" }]
            })
            .to_string(),
        )
        .send()
        .await
        .expect("responses request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    stop_runtime(&mut runtime).await;
}

#[tokio::test]
async fn runtime_rejects_unauthenticated_messages_requests_over_real_http() {
    let guard = RuntimeHttpTestGuard::new();
    let port = guard.port;

    let config = runtime_test_gateway_config(port);
    let mut runtime = spawn_runtime(config).await.expect("runtime should start");

    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/v1/messages"))
        .header("content-type", "application/json")
        .body(
            json!({
                "model": "claude-sonnet-4-5-20250929",
                "messages": [{ "role": "user", "content": "hello" }]
            })
            .to_string(),
        )
        .send()
        .await
        .expect("messages request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    stop_runtime(&mut runtime).await;
}

#[tokio::test]
async fn runtime_requires_client_api_key_even_when_local_only() {
    let guard = RuntimeHttpTestGuard::new();
    let port = guard.port;

    let config = GatewayConfig {
        port,
        local_only: true,
        account_mode: "single".to_string(),
        account_id: Some("test-account".to_string()),
        access_token: Some("sk-test".to_string()),
        ..GatewayConfig::default()
    };
    let mut runtime = spawn_runtime(config).await.expect("runtime should start");

    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/v1/responses"))
        .header("content-type", "application/json")
        .body(
            json!({
                "model": "claude-sonnet-4-5-20250929",
                "input": [{ "role": "user", "content": "hello" }]
            })
            .to_string(),
        )
        .send()
        .await
        .expect("responses request should succeed");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    stop_runtime(&mut runtime).await;
}
