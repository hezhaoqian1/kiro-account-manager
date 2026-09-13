//! OpenAI Responses 协议的请求、响应与流式事件模型。

use super::*;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct OpenAIResponsesRequest {
    pub model: String,
    pub input: Vec<NormalizedMessage>,
    #[serde(default)]
    pub stream: bool,
    pub max_output_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default)]
    pub previous_response_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct OpenAIResponsesResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub output: Vec<ResponseOutputItem>,
    pub usage: OpenAIChatUsage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
pub enum ResponseOutputItem {
    #[serde(rename = "message")]
    Message {
        role: String,
        content: Vec<ResponseContent>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ResponseContent {
    #[serde(rename = "text")]
    Text { text: String },
}

// Responses API 流式事件

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event")]
pub enum ResponsesStreamEvent {
    #[serde(rename = "response.created")]
    ResponseCreated {
        id: String,
        object: String,
        created: i64,
    },
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded {
        item_index: i32,
        item: ResponseOutputItem,
    },
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta {
        item_index: i32,
        content_index: i32,
        delta: String,
    },
    #[serde(rename = "response.output_item.done")]
    OutputItemDone { item_index: i32 },
    #[serde(rename = "response.completed")]
    ResponseCompleted { usage: OpenAIChatUsage },
}
