//! 上游错误映射与安全截断的回归测试。

use super::*;

#[test]
fn safe_truncate_never_splits_multibyte_chars() {
    // '中' 是 3 字节。构造一个字节长度超过上限、且上限不落在字符边界上的串。
    // 上限 100：'中'.repeat(40) = 120 字节，字符边界在 0,3,6,...,99,102；100 不是边界。
    // 旧代码 &s[..100] 会 panic（byte index 100 is not a char boundary）。
    let s = "中".repeat(40);
    let end = safe_truncate(&s, 100);
    // 回退到最近的合法边界 99（= 33 个 '中'）
    assert!(s.is_char_boundary(end), "回退点必须是合法字符边界");
    assert!(end <= 100, "不得超过字节上限");
    // 真正切一刀，确认不 panic
    let _ = &s[..end];
    assert_eq!(end, 99);

    // ASCII：上限正好落在边界，原样返回
    let ascii = "A".repeat(120);
    assert_eq!(safe_truncate(&ascii, 100), 100);

    // 串本身比上限短：返回全长
    assert_eq!(safe_truncate("abc", 100), 3);

    // 上限 0：返回 0，不 panic
    assert_eq!(safe_truncate(&s, 0), 0);

    // emoji（4 字节）边界回退
    let emoji = "😀".repeat(10); // 40 字节，边界 0,4,8,...,40
    let e = safe_truncate(&emoji, 10); // 10 不是 4 的倍数 → 回退到 8
    assert_eq!(e, 8);
    let _ = &emoji[..e];
}

#[test]
fn map_upstream_error_detects_invalid_bearer_token() {
    let body =
        r#"{"message":"The bearer token included in the request is invalid.","reason":null}"#;

    let (status, error_type, message) = map_upstream_error(StatusCode::FORBIDDEN, body);

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error_type, "token_expired_error");
    assert_eq!(
        message,
        "The bearer token included in the request is invalid."
    );
}

#[test]
fn detect_upstream_error_body_maps_success_status_error_payloads() {
    let error = detect_upstream_error_body(
        r#"{"error":{"message":"Invalid model. Please select a different model to continue.","type":"invalid_request_error"}}"#,
    )
    .expect("error payload should be detected");

    assert_eq!(error.0, StatusCode::BAD_REQUEST);
    assert_eq!(error.1, "invalid_request_error");
    assert!(error.2.contains("Invalid model"));
}
