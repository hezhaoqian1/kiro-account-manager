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
