//! Shared application library.
//!
//! The desktop binary still owns the Tauri window/tray lifecycle, while the
//! Railway binary reuses these same account, authentication, client and
//! gateway modules without starting a WebView.

pub mod auth;
pub mod auto_switch;
pub mod clients;
pub mod commands;
pub mod core;
pub mod gateway;
pub mod kiro;
pub mod model_lock;
pub mod models;
pub mod services;
pub mod server_db;
pub mod state;
pub mod tasks;
pub mod utils;
