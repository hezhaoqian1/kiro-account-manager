use super::*;

#[tokio::test]
async fn health_route_is_reachable() {
    let app = router(test_router_state());
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/health")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn responses_route_is_reachable() {
    let app = router(test_router_state());
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/v1/responses")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "claude-sonnet-4-5-20250929",
                        "input": [{ "role": "user", "content": "hello" }]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn openai_chat_completions_endpoint_accepts_requests() {
    let app = router(test_router_state());
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "claude-sonnet-4-5-20250929",
                        "messages": [{ "role": "user", "content": "hello" }]
                    })
                    .to_string(),
                ))
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn lightweight_routes_increment_request_count_and_write_logs() {
    let fixture = RequestLogTestFixture::new();
    let state = gateway_runtime_test_state();
    let client_addr: SocketAddr = "127.0.0.1:4317".parse().expect("socket addr should parse");

    let health = proxy::health_handler(state.clone(), client_addr, auth_headers()).await;
    assert_eq!(health.status(), StatusCode::OK);

    let models = proxy::models_handler(state.clone(), client_addr, auth_headers()).await;
    assert_eq!(models.status(), StatusCode::OK);
    let count_tokens = proxy::anthropic_count_tokens_handler(
        state.clone(),
        client_addr,
        auth_headers(),
        json!({
            "model": "claude-sonnet-4-5-20250929",
            "messages": [{ "role": "user", "content": "hello world" }]
        }),
    )
    .await;
    assert_eq!(count_tokens.status(), StatusCode::OK);

    assert_eq!(state.request_count.load(Ordering::Relaxed), 3);

    let logs = get_gateway_request_logs_from_path(fixture.path.as_path(), Some(10))
        .expect("request logs should read");
    assert_eq!(logs.len(), 3);
    assert_eq!(logs[0].endpoint, "count_tokens");
    assert_eq!(logs[1].endpoint, "models");
    assert_eq!(logs[2].endpoint, "health");
    assert_eq!(logs[0].status_code, 200);
    assert_eq!(logs[1].status_code, 200);
    assert_eq!(logs[2].status_code, 200);
    assert_eq!(logs[0].outcome, "success");
    assert_eq!(logs[1].outcome, "success");
    assert_eq!(logs[2].outcome, "success");
    assert_eq!(logs[0].client_ip, "127.0.0.1");
    // 注意：当前实现默认会记录请求体/响应体（前端 RequestLogsDialog 有「请求体 / 响应体」展示位）。
    // 历史上 7a8aba8「收紧网关日志鉴权」曾把 prepare_logged_body 掏空为恒返回 None，
    // 但 9ff5ccc「日志优化」又改回记录原文，且未同步本断言 —— 见 PR 讨论。
    assert!(
        logs[0]
            .request_body
            .as_deref()
            .is_some_and(|body| body.contains("hello world")),
        "count_tokens 的请求体应被记录"
    );
    assert!(
        logs[0].response_body.is_some(),
        "count_tokens 的响应体应被记录"
    );
    // health / models 属于无请求体的轻量路由
    assert!(logs[1].request_body.is_none());
    assert!(logs[2].request_body.is_none());
}
