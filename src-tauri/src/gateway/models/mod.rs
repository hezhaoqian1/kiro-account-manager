//! 网关的线上模型定义，按协议拆分。

use serde::{Deserialize, Serialize};

mod common;
mod kiro;
mod anthropic;
mod openai_chat;

pub use common::*;
pub use kiro::*;
pub use anthropic::*;
pub use openai_chat::*;
