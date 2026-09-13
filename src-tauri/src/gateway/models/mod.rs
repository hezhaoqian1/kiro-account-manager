//! 网关的线上模型定义，按协议拆分。

use serde::{Deserialize, Serialize};

mod common;
mod kiro;
mod responses;
mod anthropic;
mod openai_chat;

pub use common::*;
pub use kiro::*;
// `responses` 里的 5 个类型各自带 `#[allow(dead_code)]`（Responses 协议模型，
// 目前全仓库没有调用点），因此这个重导出不会被任何地方用到。
// 保留它是有意的：拆分前这些类型就定义在 `models.rs` 里，
// `models::OpenAIResponsesRequest` 这类路径必须继续可解析。
#[allow(unused_imports)]
pub use responses::*;
pub use anthropic::*;
pub use openai_chat::*;
