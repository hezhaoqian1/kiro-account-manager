//! 内容块解析的回归测试。

use super::*;

#[test]
fn extract_text_content_reads_text_from_content_array_without_unwrap() {
    let content = json!([
        { "type": "input_text", "text": "第一段" },
        { "type": "output_text", "text": "第二段" },
        { "type": "input_image", "image_url": "data:image/png;base64,aGVsbG8=" }
    ]);

    assert_eq!(extract_text_content(Some(&content)), "第一段\n第二段");
}

#[test]
fn proxy_thinking_signature_is_not_forwarded_to_kiro() {
    let content = json!([{
        "type": "thinking",
        "thinking": "internal plan",
        "signature": "kiro-proxy-v1-test"
    }]);

    let reasoning = extract_reasoning_content(Some(&content)).expect("thinking content");
    assert_eq!(reasoning["reasoningText"]["text"], "internal plan");
    assert!(reasoning["reasoningText"]["signature"].is_null());
    assert!(has_empty_thinking_signature(&Some(reasoning)));
}
