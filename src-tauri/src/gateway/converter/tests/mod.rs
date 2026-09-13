use super::*;
use crate::gateway::models::AnthropicTool;
use base64::engine::general_purpose::STANDARD;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

mod tools;
mod normalize_anthropic;
mod normalize_openai_chat;
mod normalize_openai_responses;
mod payload;
mod images;
mod models;
mod content;
