//! 请求归一化：把 Anthropic Messages / OpenAI Chat / OpenAI Responses
//! 三种下游协议统一转换为内部 `NormalizedRequest`。

use super::*;

pub fn normalize_anthropic_request(request: &AnthropicMessagesRequest) -> NormalizedRequest {
    let mut messages = Vec::new();

    // 处理 system prompt，提取 cache_control
    if let Some(system) = &request.system {
        let (system_text, system_cache_point) = extract_text_and_cache_control(system);
        if !system_text.is_empty() {
            let mut metadata = None;
            if let Some(cache_point) = system_cache_point {
                metadata = Some(json!({"cache_point": cache_point}));
            }
            messages.push(NormalizedMessage {
                role: "system".to_string(),
                content: Some(Value::String(system_text)),
                tool_calls: None,
                tool_call_id: None,
                metadata,
            });
        }
    }

    // 处理消息，提取每条消息中的 cache_control
    for message in &request.messages {
        let cache_point = extract_cache_control_from_content(&message.content);
        let mut metadata = extract_anthropic_message_metadata(message);

        // 如果消息内容中有 cache_control，添加到 metadata
        if let Some(cp) = cache_point {
            let mut meta_obj = metadata.unwrap_or_else(|| json!({}));
            if let Some(obj) = meta_obj.as_object_mut() {
                obj.insert("cache_point".to_string(), cp);
            }
            metadata = Some(meta_obj);
        }

        messages.push(NormalizedMessage {
            role: message.role.clone(),
            content: Some(convert_anthropic_content(&message.content)),
            tool_calls: extract_anthropic_tool_calls(&message.content),
            tool_call_id: extract_anthropic_tool_result_id(&message.content),
            metadata,
        });
    }

    let mut tool_name_map = std::collections::HashMap::new();
    let tools = request.tools.as_ref().map(|tools| {
        tools
            .iter()
            .map(|tool| {
                let (converted_tool, mapping) = convert_anthropic_tool(tool);
                if let Some((sanitized, original)) = mapping {
                    tool_name_map.insert(sanitized, original);
                }
                converted_tool
            })
            .collect()
    });

    let mut normalized = NormalizedRequest {
        model: request.model.clone(),
        messages,
        stream: request.stream,
        max_tokens: Some(request.max_tokens),
        temperature: request.temperature,
        top_p: request.top_p,
        stop: request.stop_sequences.clone(),
        tools,
        tool_choice: request.tool_choice.clone(),
        previous_response_id: None,
        thinking: request.thinking.clone(),
        include_usage: false,
        tool_name_map,
    };

    // 检测模型名是否包含 "thinking" 后缀，若包含则自动启用 thinking
    override_thinking_from_model_name(&mut normalized);

    normalized
}

pub fn normalize_openai_responses_request(payload: &Value) -> Result<NormalizedRequest, String> {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("claude-sonnet-4-5-20250929")
        .to_string();

    let mut messages = Vec::new();

    if let Some(instructions) = payload.get("instructions") {
        let text = extract_text_blocks(instructions, &["text", "input_text", "output_text"]);
        if !text.is_empty() {
            messages.push(NormalizedMessage {
                role: "system".to_string(),
                content: Some(Value::String(text)),
                tool_calls: None,
                tool_call_id: None,
                metadata: None,
            });
        }
    }

    if let Some(input) = payload.get("input") {
        messages.extend(convert_responses_input(input));
    }

    if messages.is_empty() {
        return Err("Responses 请求缺少可转换的 input".to_string());
    }

    let (tools, tool_name_map) = convert_responses_tools(payload.get("tools"));
    Ok(build_normalized_request_from_payload(
        payload,
        model,
        messages,
        tools,
        tool_name_map,
    ))
}

pub fn normalize_openai_chat_payload(payload: &Value) -> Result<NormalizedRequest, String> {
    let model = payload
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("claude-sonnet-4-5-20250929")
        .to_string();

    let messages = convert_openai_chat_messages(payload.get("messages"));
    if messages.is_empty() {
        return Err("chat.completions 请求缺少可转换的 messages".to_string());
    }

    let (tools, tool_name_map) = convert_openai_chat_tools(payload.get("tools"));
    Ok(build_normalized_request_from_payload(
        payload,
        model,
        messages,
        tools,
        tool_name_map,
    ))
}

pub fn normalize_openai_chat_request(request: &OpenAIChatRequest) -> Result<NormalizedRequest, String> {
    let mut messages = Vec::new();
    let mut pending_tool_results = Vec::new();

    for msg in &request.messages {
        match msg.role.as_str() {
            "system" => {
                let text = extract_text_content(msg.content.as_ref());
                if !text.is_empty() {
                    messages.push(NormalizedMessage {
                        role: "system".to_string(),
                        content: Some(Value::String(text)),
                        tool_calls: None,
                        tool_call_id: None,
                        metadata: None,
                    });
                }
            }
            "tool" => {
                let content = extract_text_content(msg.content.as_ref());
                let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();
                pending_tool_results.push((tool_call_id, content));
            }
            "user" | "assistant" => {
                if !pending_tool_results.is_empty() {
                    messages.push(create_tool_results_message(&pending_tool_results));
                    pending_tool_results.clear();
                }

                let tool_calls = if msg.role == "assistant" {
                    msg.tool_calls.as_ref().map(|tcs| {
                        tcs.iter()
                            .map(|tc| ToolCall {
                                id: tc.id.clone(),
                                call_type: tc.call_type.clone(),
                                function: ToolCallFunction {
                                    name: tc.function.name.clone(),
                                    arguments: tc.function.arguments.to_string(),
                                },
                            })
                            .collect()
                    })
                } else {
                    None
                };

                messages.push(NormalizedMessage {
                    role: msg.role.clone(),
                    content: msg.content.clone(),
                    tool_calls,
                    tool_call_id: None,
                    metadata: None,
                });
            }
            // OpenAI 文档：developer 指令级消息 → 映射为 system
            "developer" => {
                let text = extract_text_content(msg.content.as_ref());
                if !text.is_empty() {
                    messages.push(NormalizedMessage {
                        role: "system".to_string(),
                        content: Some(Value::String(text)),
                        tool_calls: None,
                        tool_call_id: None,
                        metadata: None,
                    });
                }
            }
            // 旧版 OpenAI function 角色：按 tool 结果处理
            "function" => {
                let content = extract_text_content(msg.content.as_ref());
                let tool_call_id = msg
                    .tool_call_id
                    .clone()
                    .or_else(|| msg.name.clone())
                    .unwrap_or_default();
                pending_tool_results.push((tool_call_id, content));
            }
            other => {
                // 官方 Chat Completions 角色仅限 system/user/assistant/tool/developer（+遗留 function）
                return Err(format!(
                    "不支持的 chat message.role: \"{other}\"，官方取值: system|user|assistant|tool|developer"
                ));
            }
        }
    }

    if !pending_tool_results.is_empty() {
        messages.push(create_tool_results_message(&pending_tool_results));
    }

    let mut tool_name_map = std::collections::HashMap::new();
    let tools = request.tools.as_ref().map(|tools| {
        tools
            .iter()
            .map(|t| {
                let original_name = t.function.name.clone();
                let sanitized_name = shorten_tool_name(&sanitize_tool_name(&original_name));

                if sanitized_name != original_name {
                    tool_name_map.insert(sanitized_name.clone(), original_name);
                }

                Tool {
                    tool_type: t.tool_type.clone(),
                    function: ToolFunction {
                        name: sanitized_name,
                        description: t.function.description.clone(),
                        parameters: t.function.parameters.clone(),
                    },
                    cache_control: None,
                }
            })
            .collect()
    });

    let include_usage = request
        .stream_options
        .as_ref()
        .map(|opts| opts.include_usage)
        .unwrap_or(false);

    // 处理 response_format：注入 system prompt 让模型按格式返回
    if let Some(response_format) = &request.response_format {
        let format_type = response_format.get("type").and_then(Value::as_str);
        match format_type {
            Some("json_object") => {
                let instr = "You must respond with a valid JSON object. Do not include any explanatory text outside the JSON.";
                if let Some(first) = messages.first_mut() {
                    if first.role == "system" {
                        if let Some(Value::String(text)) = &mut first.content {
                            text.push_str("\n\n");
                            text.push_str(instr);
                        }
                    } else {
                        messages.insert(0, NormalizedMessage {
                            role: "system".to_string(),
                            content: Some(Value::String(instr.to_string())),
                            tool_calls: None,
                            tool_call_id: None,
                            metadata: None,
                        });
                    }
                }
            }
            Some("json_schema") => {
                let name = response_format
                    .get("json_schema")
                    .and_then(|s| s.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("output");
                let instr = format!("You must respond with a valid JSON object conforming to the \"{}\" schema. Do not include any explanatory text outside the JSON.", name);
                if let Some(first) = messages.first_mut() {
                    if first.role == "system" {
                        if let Some(Value::String(text)) = &mut first.content {
                            text.push_str("\n\n");
                            text.push_str(&instr);
                        }
                    } else {
                        messages.insert(0, NormalizedMessage {
                            role: "system".to_string(),
                            content: Some(Value::String(instr)),
                            tool_calls: None,
                            tool_call_id: None,
                            metadata: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // 处理 parallel_tool_calls=false
    if request.parallel_tool_calls == Some(false) {
        let instr = "Call only one tool at a time. Wait for the result before calling the next tool.";
        if let Some(first) = messages.first_mut() {
            if first.role == "system" {
                if let Some(Value::String(text)) = &mut first.content {
                    text.push_str("\n\n");
                    text.push_str(instr);
                }
            } else {
                messages.insert(0, NormalizedMessage {
                    role: "system".to_string(),
                    content: Some(Value::String(instr.to_string())),
                    tool_calls: None,
                    tool_call_id: None,
                    metadata: None,
                });
            }
        }
    }

    // 处理 n > 1：Kiro 只支持返回 1 个，打日志提示
    if let Some(n) = request.n {
        if n > 1 {
            log::warn!("[OpenAI Chat] 请求 n={}，但 Kiro 上游仅支持返回 1 个 choice", n);
        }
    }

    Ok(NormalizedRequest {
        model: request.model.clone(),
        messages,
        stream: request.stream,
        max_tokens: request.max_tokens.or(request.max_completion_tokens),
        temperature: request.temperature,
        top_p: request.top_p,
        stop: request.stop.clone(),
        tools,
        tool_choice: request.tool_choice.clone(),
        previous_response_id: None,
        thinking: None,
        include_usage,
        tool_name_map,
    })
}

pub fn create_tool_results_message(tool_results: &[(String, String)]) -> NormalizedMessage {
    let mut content_array = Vec::new();
    for (tool_call_id, content) in tool_results {
        content_array.push(json!({
            "type": "tool_result",
            "tool_use_id": tool_call_id,
            "content": content
        }));
    }

    NormalizedMessage {
        role: "user".to_string(),
        content: Some(Value::Array(content_array)),
        tool_calls: None,
        tool_call_id: None,
        metadata: None,
    }
}

pub fn build_normalized_request_from_payload(
    payload: &Value,
    model: String,
    mut messages: Vec<NormalizedMessage>,
    tools: Option<Vec<Tool>>,
    tool_name_map: std::collections::HashMap<String, String>,
) -> NormalizedRequest {
    // 处理 response_format：注入 system prompt 让模型按格式返回
    if let Some(response_format) = payload.get("response_format").and_then(|v| v.as_object()) {
        let format_type = response_format.get("type").and_then(Value::as_str);
        let instruction = match format_type {
            Some("json_object") => {
                Some("You must respond with a valid JSON object. Do not include any explanatory text outside the JSON.".to_string())
            }
            Some("json_schema") => {
                let name = response_format
                    .get("json_schema")
                    .and_then(|s| s.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("output");
                let description = response_format
                    .get("json_schema")
                    .and_then(|s| s.get("description"))
                    .and_then(Value::as_str)
                    .filter(|d| !d.is_empty());
                match description {
                    Some(desc) => Some(format!(
                        "You must respond with a valid JSON object conforming to the \"{}\" schema. Description: {}. Do not include any explanatory text outside the JSON.",
                        name, desc
                    )),
                    None => Some(format!(
                        "You must respond with a valid JSON object conforming to the \"{}\" schema. Do not include any explanatory text outside the JSON.",
                        name
                    )),
                }
            }
            _ => None,
        };
        if let Some(instr) = instruction {
            if let Some(first) = messages.first_mut() {
                if first.role == "system" {
                    if let Some(Value::String(text)) = &mut first.content {
                        text.push_str("\n\n");
                        text.push_str(&instr);
                    } else {
                        first.content = Some(Value::String(instr));
                    }
                } else {
                    messages.insert(0, NormalizedMessage {
                        role: "system".to_string(),
                        content: Some(Value::String(instr)),
                        tool_calls: None,
                        tool_call_id: None,
                        metadata: None,
                    });
                }
            } else {
                messages.push(NormalizedMessage {
                    role: "system".to_string(),
                    content: Some(Value::String(instr)),
                    tool_calls: None,
                    tool_call_id: None,
                    metadata: None,
                });
            }
        }
    }

    // 处理 parallel_tool_calls=false：注入 system prompt 让模型一次只调用一个工具
    if payload.get("parallel_tool_calls").and_then(Value::as_bool) == Some(false) {
        let instr = "Call only one tool at a time. Wait for the result before calling the next tool.";
        if let Some(first) = messages.first_mut() {
            if first.role == "system" {
                if let Some(Value::String(text)) = &mut first.content {
                    text.push_str("\n\n");
                    text.push_str(instr);
                }
            } else {
                messages.insert(0, NormalizedMessage {
                    role: "system".to_string(),
                    content: Some(Value::String(instr.to_string())),
                    tool_calls: None,
                    tool_call_id: None,
                    metadata: None,
                });
            }
        }
    }

    // 处理 reasoning.effort → Kiro thinking 映射（OpenAI Responses API）
    let thinking = payload.get("reasoning").and_then(|r| r.get("effort")).and_then(Value::as_str).map(|effort| {
        let budget_tokens = match effort {
            "low" => 1024,
            "medium" => 4096,
            "high" => 16384,
            _ => 4096,
        };
        log::info!("[模型映射] reasoning.effort=\"{}\" → thinking enabled, budget_tokens={}", effort, budget_tokens);
        Thinking {
            thinking_type: "enabled".to_string(),
            budget_tokens,
        }
    });

    NormalizedRequest {
        model,
        messages,
        stream: payload
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(true), // 默认使用流式响应
        max_tokens: payload
            .get("max_output_tokens")
            .or_else(|| payload.get("max_completion_tokens"))
            .or_else(|| payload.get("max_tokens"))
            .and_then(Value::as_i64)
            .map(|value| value as i32),
        temperature: payload
            .get("temperature")
            .and_then(Value::as_f64)
            .map(|value| value as f32),
        top_p: payload
            .get("top_p")
            .and_then(Value::as_f64)
            .map(|value| value as f32),
        stop: payload.get("stop").and_then(|value| match value {
            Value::String(item) => Some(vec![item.to_string()]),
            Value::Array(items) => Some(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
            ),
            _ => None,
        }),
        tools,
        tool_choice: payload.get("tool_choice").cloned(),
        previous_response_id: payload
            .get("previous_response_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        thinking,
        include_usage: payload
            .get("stream_options")
            .and_then(|opts| opts.get("include_usage"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        tool_name_map,
    }
}

pub fn convert_openai_chat_messages(messages: Option<&Value>) -> Vec<NormalizedMessage> {
    let Some(Value::Array(items)) = messages else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| {
            // 官方 Chat Completions roles: system | user | assistant | tool | developer
            // 另兼容遗留 function（按 tool 结果处理）
            let raw_role = item.get("role").and_then(Value::as_str)?;
            let (role, tool_call_id) = match raw_role {
                "system" | "user" | "assistant" | "tool" => {
                    let tool_call_id = item
                        .get("tool_call_id")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    (raw_role.to_string(), tool_call_id)
                }
                // OpenAI developer 指令级消息 → 内部 system
                "developer" => ("system".to_string(), None),
                // 旧版 function 角色 → tool，name 作 tool_call_id 回退
                "function" => {
                    let tool_call_id = item
                        .get("tool_call_id")
                        .and_then(Value::as_str)
                        .or_else(|| item.get("name").and_then(Value::as_str))
                        .map(str::to_string);
                    ("tool".to_string(), tool_call_id)
                }
                other => {
                    log::warn!(
                        "[协议映射] 不支持的 chat message.role=\"{other}\"，已跳过。官方取值: system|user|assistant|tool|developer"
                    );
                    return None;
                }
            };

            let tool_calls = if role == "assistant" {
                item.get("tool_calls")
                    .and_then(Value::as_array)
                    .map(|calls| {
                        calls
                            .iter()
                            .filter_map(|call| {
                                Some(ToolCall {
                                    id: call.get("id").and_then(Value::as_str)?.to_string(),
                                    call_type: call
                                        .get("type")
                                        .and_then(Value::as_str)
                                        .unwrap_or("function")
                                        .to_string(),
                                    function: ToolCallFunction {
                                        name: call
                                            .get("function")?
                                            .get("name")
                                            .and_then(Value::as_str)?
                                            .to_string(),
                                        arguments: call
                                            .get("function")?
                                            .get("arguments")
                                            .and_then(Value::as_str)
                                            .unwrap_or("{}")
                                            .to_string(),
                                    },
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .filter(|calls| !calls.is_empty())
            } else {
                None
            };

            let content = item.get("content").map(convert_openai_chat_content);
            Some(NormalizedMessage {
                role,
                content,
                tool_calls,
                tool_call_id,
                metadata: None,
            })
        })
        .collect()
}

pub fn convert_openai_chat_content(content: &Value) -> Value {
    match content {
        Value::String(text) => Value::String(text.clone()),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| {
                    if item.get("type").and_then(Value::as_str) == Some("text") {
                        json!({
                            "type": "input_text",
                            "text": item.get("text").and_then(Value::as_str).unwrap_or_default()
                        })
                    } else {
                        item.clone()
                    }
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

pub fn convert_openai_chat_tools(
    tools: Option<&Value>,
) -> (Option<Vec<Tool>>, std::collections::HashMap<String, String>) {
    convert_responses_tools(tools)
}

/// 检测模型名是否包含 "thinking" 后缀，若包含则覆写 thinking 配置
///
/// 根据 Anthropic 官方文档 (https://platform.claude.com/docs/en/docs/about-claude/models):
///
/// **Adaptive Thinking** (type: "adaptive"):
/// - Claude Opus 4.7
/// - Claude Sonnet 4.6
///
/// **Extended Thinking** (type: "enabled"):
/// - Claude Haiku 4.5
/// - Claude Sonnet 4.5
/// - Claude Opus 4.5
///
/// budget_tokens 固定为 20000
pub fn override_thinking_from_model_name(request: &mut NormalizedRequest) {
    let model_lower = request.model.to_lowercase();
    if !model_lower.contains("thinking") {
        return;
    }

    // 判断是否支持 Adaptive Thinking
    let supports_adaptive =
        // Claude Opus 4.7
        (model_lower.contains("opus") && (model_lower.contains("4-7") || model_lower.contains("4.7")))
        ||
        // Claude Sonnet 4.6
        (model_lower.contains("sonnet") && (model_lower.contains("4-6") || model_lower.contains("4.6")));

    let thinking_type = if supports_adaptive {
        "adaptive"
    } else {
        "enabled"
    };

    log::info!(
        "[Gateway] 模型名 {} 包含 thinking 后缀，覆写 thinking 配置为 {}",
        request.model,
        thinking_type
    );

    use crate::gateway::models::Thinking;
    request.thinking = Some(Thinking {
        thinking_type: thinking_type.to_string(),
        budget_tokens: 20000,
    });
}

pub fn convert_anthropic_content(content: &Value) -> Value {
    match content {
        Value::String(text) => Value::String(text.clone()),
        Value::Array(items) => {
            // 检查是否包含 tool_result
            let has_tool_result = items
                .iter()
                .any(|item| item.get("type").and_then(Value::as_str) == Some("tool_result"));
            if has_tool_result {
                return content.clone();
            }

            // 检查是否包含图片（必须保留原始数组，extract_images 需要从中提取）
            let has_image = items.iter().any(|item| {
                let t = item.get("type").and_then(Value::as_str).unwrap_or_default();
                t == "image" || t == "image_url" || t == "input_image"
            });
            if has_image {
                return content.clone();
            }

            // 只有纯文本内容才转换为字符串
            let text = extract_text_blocks(content, &["text"]);
            if text.is_empty() {
                content.clone()
            } else {
                Value::String(text)
            }
        }
        other => other.clone(),
    }
}

pub fn convert_responses_input(input: &Value) -> Vec<NormalizedMessage> {
    match input {
        Value::String(text) => vec![NormalizedMessage {
            role: "user".to_string(),
            content: Some(Value::String(text.clone())),
            tool_calls: None,
            tool_call_id: None,
            metadata: None,
        }],
        Value::Array(items) => convert_responses_input_items(items),
        _ => Vec::new(),
    }
}

pub fn convert_responses_input_items(items: &[Value]) -> Vec<NormalizedMessage> {
    let mut messages = Vec::new();
    let mut pending_user_items = Vec::new();

    for item in items {
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();

        // EasyInputMessage / message 项：官方 role = system|developer|user|assistant
        if let Some(raw_role) = item.get("role").and_then(Value::as_str) {
            flush_pending_responses_user_items(&mut messages, &mut pending_user_items);
            let role = match raw_role {
                "system" | "user" | "assistant" => raw_role.to_string(),
                // OpenAI Responses: developer 指令级 → 内部 system
                "developer" => "system".to_string(),
                other => {
                    log::warn!(
                        "[协议映射] Responses 不支持的 message.role=\"{other}\"，已跳过。官方取值: system|developer|user|assistant"
                    );
                    continue;
                }
            };
            messages.push(NormalizedMessage {
                role: role.clone(),
                content: responses_message_content(item),
                tool_calls: None,
                tool_call_id: None,
                metadata: extract_responses_message_metadata(item, &role),
            });
            continue;
        }

        match item_type {
            "message" => {
                flush_pending_responses_user_items(&mut messages, &mut pending_user_items);
                // type=message 但缺 role 时官方默认按 user 处理
                let role = "user".to_string();
                messages.push(NormalizedMessage {
                    role: role.clone(),
                    content: responses_message_content(item),
                    tool_calls: None,
                    tool_call_id: None,
                    metadata: extract_responses_message_metadata(item, &role),
                });
            }
            "function_call" => {
                flush_pending_responses_user_items(&mut messages, &mut pending_user_items);
                messages.push(NormalizedMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![ToolCall {
                        id: item
                            .get("call_id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        call_type: "function".to_string(),
                        function: ToolCallFunction {
                            name: item
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            arguments: item
                                .get("arguments")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                                .unwrap_or_else(|| {
                                    serde_json::to_string(
                                        &item
                                            .get("arguments")
                                            .cloned()
                                            .unwrap_or_else(|| json!({})),
                                    )
                                    .unwrap_or_else(|_| "{}".to_string())
                                }),
                        },
                    }]),
                    tool_call_id: None,
                    metadata: None,
                });
            }
            "function_call_output" => {
                flush_pending_responses_user_items(&mut messages, &mut pending_user_items);
                messages.push(NormalizedMessage {
                    role: "tool".to_string(),
                    content: responses_tool_output_content(item.get("output")),
                    tool_calls: None,
                    tool_call_id: item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    metadata: None,
                });
            }
            "input_text" | "output_text" | "input_image" | "image_url" | "image" => {
                pending_user_items.push(item.clone());
            }
            // OpenAI Responses 文档 reasoning item → Kiro reasoningContent
            "reasoning" => {
                flush_pending_responses_user_items(&mut messages, &mut pending_user_items);
                let mut metadata = Map::new();
                if let Some(reasoning) = extract_responses_reasoning_item(item)
                    .or_else(|| extract_reasoning_content(Some(item)))
                    .or_else(|| extract_reasoning_content(item.get("content")))
                {
                    metadata.insert("reasoningContent".to_string(), reasoning);
                }
                messages.push(NormalizedMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: None,
                    tool_call_id: None,
                    metadata: if metadata.is_empty() {
                        None
                    } else {
                        Some(Value::Object(metadata))
                    },
                });
            }
            // 官方工具类 item：按 function_call / function_call_output 语义映射到 Kiro tools
            "custom_tool_call" | "web_search_call" | "file_search_call" | "code_interpreter_call"
            | "computer_call" | "local_shell_call" | "image_generation_call" | "mcp_call" => {
                flush_pending_responses_user_items(&mut messages, &mut pending_user_items);
                push_responses_tool_call_message(&mut messages, item, item_type);
            }
            "custom_tool_call_output" | "computer_call_output" | "local_shell_call_output"
            | "mcp_approval_response" => {
                flush_pending_responses_user_items(&mut messages, &mut pending_user_items);
                push_responses_tool_output_message(&mut messages, item);
            }
            // 会话引用：依赖 previous_response_id 恢复，input 内 item_reference 本身无正文可转
            "item_reference" => {
                log::info!(
                    "[协议映射] Responses item_reference 跳过（依赖 previous_response_id 会话恢复）"
                );
            }
            // 服务端审批/列表类：无对等 Kiro 语义，记录后跳过（不伪造 user 文本）
            "mcp_list_tools" | "mcp_approval_request" => {
                log::warn!(
                    "[协议映射] Responses 官方类型 \"{item_type}\" 暂无 Kiro 对等语义，已跳过"
                );
            }
            // compaction 非 Responses 核心 input 类型；若客户端带入则保留标记（非 /compact 实现）
            "compaction" => {
                flush_pending_responses_user_items(&mut messages, &mut pending_user_items);
                messages.push(NormalizedMessage {
                    role: "system".to_string(),
                    content: Some(item.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    metadata: Some(json!({ "is_compaction": true })),
                });
            }
            other => {
                // 不在官方已知表内的 type：明确告警并跳过，禁止塞成假 user 文本
                log::warn!(
                    "[协议映射] 非官方/未映射的 Responses input type=\"{other}\"，已跳过。\
                     已映射: message|function_call|function_call_output|reasoning|\
                     custom_tool_call(|_output)|web_search_call|file_search_call|\
                     code_interpreter_call|computer_call(|_output)|local_shell_call(|_output)|\
                     image_generation_call|mcp_call|item_reference|input_text|output_text|image*"
                );
            }
        }
    }

    flush_pending_responses_user_items(&mut messages, &mut pending_user_items);
    messages
}

pub fn flush_pending_responses_user_items(
    messages: &mut Vec<NormalizedMessage>,
    pending_user_items: &mut Vec<Value>,
) {
    if pending_user_items.is_empty() {
        return;
    }

    messages.push(NormalizedMessage {
        role: "user".to_string(),
        content: Some(Value::Array(std::mem::take(pending_user_items))),
        tool_calls: None,
        tool_call_id: None,
        metadata: None,
    });
}

/// OpenAI Responses 官方 reasoning item → Kiro reasoningContent 结构
/// 文档字段：summary[].summary_text / content[].reasoning_text / encrypted_content
pub fn extract_responses_reasoning_item(item: &Value) -> Option<Value> {
    let mut texts = Vec::new();

    if let Some(summary) = item.get("summary") {
        let text = extract_text_content(Some(summary));
        if !text.is_empty() {
            texts.push(text);
        }
    }

    if let Some(content) = item.get("content") {
        match content {
            Value::Array(parts) => {
                for part in parts {
                    let part_type = part.get("type").and_then(Value::as_str).unwrap_or_default();
                    if matches!(part_type, "reasoning_text" | "summary_text" | "text") {
                        if let Some(text) = part.get("text").and_then(Value::as_str) {
                            if !text.is_empty() {
                                texts.push(text.to_string());
                            }
                        }
                    }
                }
            }
            Value::String(text) if !text.is_empty() => texts.push(text.clone()),
            _ => {}
        }
    }

    // summary / content 都没有时，兜底 text 字段
    if texts.is_empty() {
        if let Some(text) = item.get("text").and_then(Value::as_str) {
            if !text.is_empty() {
                texts.push(text.to_string());
            }
        }
    }

    let encrypted = item
        .get("encrypted_content")
        .or_else(|| item.get("encryptedContent"))
        .cloned();

    if texts.is_empty() && encrypted.is_none() {
        return None;
    }

    let mut reasoning_text = Map::new();
    let merged = texts.join("\n");
    if !merged.is_empty() {
        reasoning_text.insert("text".to_string(), Value::String(merged));
    }

    let mut reasoning = Map::new();
    if !reasoning_text.is_empty() {
        reasoning.insert("reasoningText".to_string(), Value::Object(reasoning_text));
    }
    if let Some(enc) = encrypted {
        reasoning.insert("encryptedContent".to_string(), enc);
    }

    meaningful_optional_value(Some(Value::Object(reasoning)))
}

/// Responses 官方工具调用 item → assistant.tool_calls（function 语义）
pub fn push_responses_tool_call_message(
    messages: &mut Vec<NormalizedMessage>,
    item: &Value,
    item_type: &str,
) {
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(item_type)
        .to_string();
    let arguments = item
        .get("arguments")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            serde_json::to_string(
                item.get("arguments")
                    .or_else(|| item.get("action"))
                    .or_else(|| item.get("input"))
                    .unwrap_or(&json!({})),
            )
            .unwrap_or_else(|_| "{}".to_string())
        });
    messages.push(NormalizedMessage {
        role: "assistant".to_string(),
        content: None,
        tool_calls: Some(vec![ToolCall {
            id: item
                .get("call_id")
                .or_else(|| item.get("id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            call_type: "function".to_string(),
            function: ToolCallFunction { name, arguments },
        }]),
        tool_call_id: None,
        metadata: None,
    });
}

/// Responses 官方工具输出 item → role=tool
pub fn push_responses_tool_output_message(messages: &mut Vec<NormalizedMessage>, item: &Value) {
    messages.push(NormalizedMessage {
        role: "tool".to_string(),
        content: responses_tool_output_content(
            item.get("output")
                .or_else(|| item.get("result"))
                .or_else(|| item.get("content")),
        ),
        tool_calls: None,
        tool_call_id: item
            .get("call_id")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        metadata: None,
    });
}

pub fn responses_message_content(item: &Value) -> Option<Value> {
    item.get("content")
        .cloned()
        .or_else(|| item.get("text").cloned())
}

pub fn responses_tool_output_content(output: Option<&Value>) -> Option<Value> {
    match output {
        None => None,
        Some(Value::String(text)) => Some(Value::String(text.clone())),
        Some(other) => Some(Value::String(other.to_string())),
    }
}

pub fn extract_responses_message_metadata(item: &Value, role: &str) -> Option<Value> {
    if role != "assistant" {
        return None;
    }

    let mut metadata = Map::new();
    for key in [
        "reasoningContent",
        "references",
        "supplementaryWebLinks",
        "followupPrompt",
        "cachePoint",
    ] {
        if let Some(value) = meaningful_optional_value(item.get(key).cloned()) {
            metadata.insert(key.to_string(), value);
        }
    }

    if let Some(message_id) = item
        .get("messageId")
        .or_else(|| item.get("id"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        metadata.insert(
            "messageId".to_string(),
            Value::String(message_id.to_string()),
        );
    }

    if !metadata.contains_key("reasoningContent") {
        if let Some(reasoning) = extract_reasoning_content(item.get("content")) {
            metadata.insert("reasoningContent".to_string(), reasoning);
        }
    }

    if metadata.is_empty() {
        None
    } else {
        Some(Value::Object(metadata))
    }
}

pub fn extract_anthropic_message_metadata(
    message: &crate::gateway::models::AnthropicMessage,
) -> Option<Value> {
    if message.role != "assistant" {
        return None;
    }

    let mut metadata = Map::new();
    if let Some(reasoning) = extract_reasoning_content(Some(&message.content)) {
        metadata.insert("reasoningContent".to_string(), reasoning);
    }

    if metadata.is_empty() {
        None
    } else {
        Some(Value::Object(metadata))
    }
}

pub fn convert_responses_tools(
    tools: Option<&Value>,
) -> (Option<Vec<Tool>>, std::collections::HashMap<String, String>) {
    let mut tool_name_map = std::collections::HashMap::new();

    let Some(items) = tools.and_then(Value::as_array) else {
        return (None, tool_name_map);
    };

    let converted: Vec<Tool> = items
        .iter()
        .filter_map(|item| {
            let (tool, mapping) = convert_responses_tool(item)?;
            if let Some((sanitized, original)) = mapping {
                tool_name_map.insert(sanitized, original);
            }
            Some(tool)
        })
        .collect();

    if converted.is_empty() {
        (None, tool_name_map)
    } else {
        (Some(converted), tool_name_map)
    }
}

pub fn convert_responses_tool(item: &Value) -> Option<(Tool, Option<(String, String)>)> {
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();

    if item.get("function").is_some() {
        let mut tool: Tool = serde_json::from_value(item.clone()).ok()?;
        let original_name = tool.function.name.clone();
        let sanitized_name = shorten_tool_name(&sanitize_tool_name(&original_name));
        tool.function.name = sanitized_name.clone();

        let mapping = if sanitized_name != original_name {
            Some((sanitized_name, original_name))
        } else {
            None
        };

        return Some((tool, mapping));
    }

    // 修复：MCP 工具缺少 type 字段导致之前被跳过
    // MCP 格式：{ "name": "...", "description": "...", "inputSchema": {...} }
    // 转换为 OpenAI 格式：{ "type": "function", "function": { "name": "...", "parameters": {...} } }
    if item_type.is_empty() && item.get("name").is_some() {
        let original_name = item.get("name").and_then(Value::as_str)?.to_string();
        let sanitized_name = shorten_tool_name(&sanitize_tool_name(&original_name));
        let description = item
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string);

        // 从 inputSchema 或 parameters 中提取参数定义
        // MCP 工具的 inputSchema 本身就是 JSON Schema，不需要访问 .json 字段
        let parameters = item
            .get("inputSchema")
            .cloned()
            .or_else(|| item.get("parameters").cloned());

        let mapping = if sanitized_name != original_name {
            Some((sanitized_name.clone(), original_name))
        } else {
            None
        };

        return Some((
            Tool {
                tool_type: "function".to_string(),
                function: ToolFunction {
                    name: sanitized_name,
                    description,
                    parameters,
                },
                cache_control: None,
            },
            mapping,
        ));
    }

    if item_type != "function" {
        return None;
    }

    let original_name = item.get("name").and_then(Value::as_str)?.to_string();
    let sanitized_name = shorten_tool_name(&sanitize_tool_name(&original_name));

    let mapping = if sanitized_name != original_name {
        Some((sanitized_name.clone(), original_name))
    } else {
        None
    };

    Some((
        Tool {
            tool_type: "function".to_string(),
            function: ToolFunction {
                name: sanitized_name,
                description: item
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                parameters: item.get("parameters").cloned(),
            },
            cache_control: None,
        },
        mapping,
    ))
}

pub fn extract_anthropic_tool_calls(content: &Value) -> Option<Vec<ToolCall>> {
    let Value::Array(items) = content else {
        return None;
    };

    let tool_calls: Vec<ToolCall> = items
        .iter()
        .filter_map(|item| {
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
            if item_type != "tool_use" {
                return None;
            }

            Some(ToolCall {
                id: item
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                call_type: "function".to_string(),
                function: ToolCallFunction {
                    name: item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    arguments: serde_json::to_string(
                        &item.get("input").cloned().unwrap_or_else(|| json!({})),
                    )
                    .unwrap_or_else(|_| "{}".to_string()),
                },
            })
        })
        .collect();

    if tool_calls.is_empty() {
        None
    } else {
        Some(tool_calls)
    }
}

pub fn extract_anthropic_tool_result_id(content: &Value) -> Option<String> {
    let Value::Array(items) = content else {
        return None;
    };

    items.iter().find_map(|item| {
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
        if item_type == "tool_result" {
            item.get("tool_use_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        } else {
            None
        }
    })
}
