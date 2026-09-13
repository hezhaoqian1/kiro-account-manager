/// Token 估算模块
///
/// 由于 Kiro IDE 的响应中不包含实际的 token 统计信息，
/// 我们使用与 Kiro IDE 相同的估算算法来计算 token 数量。
///
/// 参考：Kiro IDE extension.js 中的 token-estimator 模块

/// 估算文本的 token 数量（Claude 模型）
///
/// 算法：字符数 / 4
/// 这是 Kiro IDE 对 Claude 模型使用的估算方法
pub fn estimate_tokens_claude(text: &str) -> i32 {
    if text.is_empty() {
        return 0;
    }

    // Claude 模型：字符数 / 4
    ((text.len() as f64) / 4.0).ceil() as i32
}

/// 估算文本的 token 数量（通用方法）
///
/// 算法：
/// - 基础 tokens = 字符数 / 4
/// - 换行符加成 = 换行数 * 0.5
/// - 代码块加成 = 代码块数量 * 2
///
/// 这是 Kiro IDE 的 fallback 估算方法
pub fn estimate_tokens_generic(text: &str) -> i32 {
    if text.is_empty() {
        return 0;
    }

    // 基础 tokens
    let base_tokens = ((text.len() as f64) / 4.0).ceil() as i32;

    // 换行符加成
    let newline_count = text.matches('\n').count();
    let newline_bonus = ((newline_count as f64) * 0.5).ceil() as i32;

    // 代码块加成（```）
    let code_block_count = text.matches("```").count();
    let code_block_bonus = (code_block_count * 2) as i32;

    base_tokens + newline_bonus + code_block_bonus
}

/// 估算文本的 token 数量（Llama 模型）
///
/// 算法：字符数 / 3.5
pub fn estimate_tokens_llama(text: &str) -> i32 {
    if text.is_empty() {
        return 0;
    }

    // Llama 模型：字符数 / 3.5
    ((text.len() as f64) / 3.5).ceil() as i32
}

/// 根据模型名称选择合适的估算方法
pub fn estimate_tokens(text: &str, model_name: &str) -> i32 {
    if text.is_empty() {
        return 0;
    }

    let model_lower = model_name.to_lowercase();

    // 根据模型名称选择估算方法
    if model_lower.contains("claude")
        || model_lower.contains("sonnet")
        || model_lower.contains("opus")
        || model_lower.contains("haiku")
    {
        // Claude 系列模型
        estimate_tokens_claude(text)
    } else if model_lower.contains("llama") {
        // Llama 系列模型
        estimate_tokens_llama(text)
    } else if model_lower.contains("deepseek")
        || model_lower.contains("qwen")
        || model_lower.contains("glm")
        || model_lower.contains("minimax")
    {
        // 中文开源模型（DeepSeek、Qwen、GLM、MiniMax）
        // 使用 Llama 的估算方法（字符数 / 3.5）
        estimate_tokens_llama(text)
    } else {
        // 默认使用通用方法
        estimate_tokens_generic(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_claude() {
        // 空字符串
        assert_eq!(estimate_tokens_claude(""), 0);

        // 简单文本：4 个字符 = 1 token
        assert_eq!(estimate_tokens_claude("test"), 1);

        // 8 个字符 = 2 tokens
        assert_eq!(estimate_tokens_claude("test123"), 2);

        // 中文：每个汉字约 3 字节，"你好" = 6 字节 / 4 = 2 tokens
        assert_eq!(estimate_tokens_claude("你好"), 2);
    }

    #[test]
    fn test_estimate_tokens_generic() {
        // 空字符串
        assert_eq!(estimate_tokens_generic(""), 0);

        // 简单文本：4 个字符 = 1 token
        assert_eq!(estimate_tokens_generic("test"), 1);

        // 带换行符：len("test\n") = 5 → ceil(5/4) = 2，换行加成 ceil(1 * 0.5) = 1，合计 3
        // 注意 len() 已包含换行符本身，所以 base 是 2 而非 1
        assert_eq!(estimate_tokens_generic("test\n"), 3);

        // 带代码块：len("```rust\n```") = 11 → ceil(11/4) = 3，换行加成 1，2 个 ``` * 2 = 4，合计 8
        assert_eq!(estimate_tokens_generic("```rust\n```"), 8);
    }

    #[test]
    fn test_estimate_tokens_by_model() {
        let text = "Hello, world!";

        // Claude 模型
        assert_eq!(
            estimate_tokens(text, "claude-sonnet-4.5"),
            estimate_tokens_claude(text)
        );
        assert_eq!(
            estimate_tokens(text, "claude-opus-4"),
            estimate_tokens_claude(text)
        );

        // Llama 模型
        assert_eq!(
            estimate_tokens(text, "llama-3"),
            estimate_tokens_llama(text)
        );

        // 未知模型（使用通用方法）
        assert_eq!(
            estimate_tokens(text, "unknown-model"),
            estimate_tokens_generic(text)
        );
    }
}
