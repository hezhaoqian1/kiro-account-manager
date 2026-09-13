//! 上游错误映射：状态码与错误类型归一化、错误信息提取与脱敏。

use super::*;

pub fn gateway_error_response(
    format: ResponseFormat,
    status: StatusCode,
    error_type: &str,
    message: &str,
    response_body: Option<&str>,
) -> Response {
    // 如果有原始响应体且是有效的 JSON，直接透传
    if let Some(body_str) = response_body {
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(body_str) {
            return (status, Json(json_value)).into_response();
        }
    }

    // 否则使用构造的错误响应
    let body = build_gateway_error_body(format, status, error_type, message);
    (status, Json(body)).into_response()
}

pub fn map_upstream_error(status: StatusCode, body: &str) -> (StatusCode, &'static str, String) {
    let sanitized = sanitize_error(&extract_error_message(body));
    let explicit_error_type = extract_error_type(body);
    let text = body.to_lowercase();

    // 检测封禁错误（403 + TEMPORARILY_SUSPENDED 或 AccessDeniedException + TemporarilySuspended）
    let is_banned = status == StatusCode::FORBIDDEN
        && (body.contains("TEMPORARILY_SUSPENDED")
            || (body.contains("AccessDeniedException") && body.contains("TemporarilySuspended")));

    // 检测token失效错误（403 + bearer token invalid/expired）
    let is_token_invalid = status == StatusCode::FORBIDDEN
        && (text.contains("bearer token") || text.contains("bearer_token"))
        && (text.contains("invalid") || text.contains("expired"));

    let mapped_status = if status == StatusCode::BAD_GATEWAY || status == StatusCode::OK {
        if explicit_error_type == Some("authentication_error") {
            StatusCode::UNAUTHORIZED
        } else if explicit_error_type == Some("permission_error") {
            StatusCode::FORBIDDEN
        } else if explicit_error_type == Some("rate_limit_error") {
            StatusCode::TOO_MANY_REQUESTS
        } else if explicit_error_type == Some("invalid_request_error") {
            StatusCode::BAD_REQUEST
        } else if text.contains("throttlingexception")
            || text.contains("servicequotaexceededexception")
        {
            StatusCode::TOO_MANY_REQUESTS
        } else if text.contains("accessdeniedexception") {
            StatusCode::FORBIDDEN
        } else if text.contains("validationexception") {
            StatusCode::BAD_REQUEST
        } else if text.contains("serviceunavailableexception") {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::BAD_GATEWAY
        }
    } else {
        status
    };
    // 根据检测结果返回特殊的error_type和message
    let (error_type, message) = if is_banned {
        // 对于封禁错误，返回以 BANNED: 开头的消息，以便前端可以识别
        ("account_banned_error", format!("BANNED: {}", sanitized))
    } else if is_token_invalid {
        ("token_expired_error", sanitized)
    } else {
        let error_type = explicit_error_type.unwrap_or(match mapped_status {
            StatusCode::UNAUTHORIZED => "authentication_error",
            StatusCode::FORBIDDEN => "permission_error",
            StatusCode::PAYMENT_REQUIRED => "insufficient_quota",
            StatusCode::TOO_MANY_REQUESTS => "rate_limit_error",
            StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND | StatusCode::CONFLICT => {
                "invalid_request_error"
            }
            _ => "api_error",
        });
        (error_type, sanitized)
    };

    (mapped_status, error_type, message)
}

pub fn extract_error_type(body: &str) -> Option<&'static str> {
    let value = serde_json::from_str::<Value>(body).ok()?;
    let raw = value
        .pointer("/error/type")
        .and_then(Value::as_str)
        .or_else(|| value.pointer("/type").and_then(Value::as_str))?;

    match raw {
        "authentication_error" => Some("authentication_error"),
        "permission_error" => Some("permission_error"),
        "insufficient_quota" => Some("insufficient_quota"),
        "rate_limit_error" => Some("rate_limit_error"),
        "invalid_request_error" => Some("invalid_request_error"),
        "api_error" => Some("api_error"),
        _ => None,
    }
}

pub fn detect_upstream_error_body(body: &str) -> Option<(StatusCode, &'static str, String)> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }

    let value = serde_json::from_str::<Value>(trimmed).ok()?;
    let object = value.as_object()?;
    let has_error_container = object.get("error").is_some();
    let has_error_metadata = object.get("__type").and_then(Value::as_str).is_some()
        || object.get("errorCode").and_then(Value::as_str).is_some()
        || object.get("Message").and_then(Value::as_str).is_some();
    let has_message_only_error = object.get("message").and_then(Value::as_str).is_some()
        && object.get("content").is_none()
        && object.get("output").is_none()
        && object.get("choices").is_none()
        && object.get("results").is_none();

    if has_error_container || has_error_metadata || has_message_only_error {
        Some(map_upstream_error(StatusCode::OK, trimmed))
    } else {
        None
    }
}

pub fn extract_error_message(body: &str) -> String {
    if body.trim().is_empty() {
        return "上游返回空错误响应".to_string();
    }
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        for pointer in [
            "/message",
            "/Message",
            "/error/message",
            "/reason",
            "/__type",
            "/errorCode",
        ] {
            if let Some(text) = value.pointer(pointer).and_then(Value::as_str) {
                return text.to_string();
            }
        }
    }
    body.to_string()
}

/// 安全截断字符串到指定字节数，确保不会切到 UTF-8 多字节字符中间
pub fn safe_truncate(s: &str, max_bytes: usize) -> usize {
    if s.len() <= max_bytes {
        return s.len();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

pub fn sanitize_error(message: &str) -> String {
    let mut sanitized = message.to_string();
    for pattern in [
        r"Bearer\s+[A-Za-z0-9._\-]+",
        r#""accessToken"\s*:\s*"[^"]+""#,
        r#""refreshToken"\s*:\s*"[^"]+""#,
        r#""clientSecret"\s*:\s*"[^"]+""#,
        r#"sk-[A-Za-z0-9]+"#,
    ] {
        if let Ok(regex) = Regex::new(pattern) {
            sanitized = regex.replace_all(&sanitized, "[REDACTED]").to_string();
        }
    }
    sanitized
}

pub fn short_uuid() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")
}
