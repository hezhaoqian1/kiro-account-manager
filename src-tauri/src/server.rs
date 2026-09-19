//! Railway headless server.
//!
//! This binary serves the React management console, admin APIs and the
//! existing `/v1/*` gateway from one Axum listener. It never starts Tauri,
//! WebView, tray, desktop OAuth listeners or Kiro IDE processes.

use axum::{
    extract::{Extension, Path, Query},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use kiro_account_manager::{
    auth::{self, providers::get_provider_config},
    clients::kiro_auth_client::KiroAuthServiceClient,
    commands::{
        account_cmd::{self, UpdateAccountParams},
        app_settings_cmd::{self, AppSettings},
        auth_cmd, cache_cmd,
        common::{
            find_existing_account_idx, generate_account_machine_id,
            get_usage_by_provider_with_machine_id, lock_store, save_store, update_account_status,
        },
        custom_agents_cmd, gateway_cmd, hooks_cmd, kiro_settings_cmd, machine_guid, mcp_cmd,
        permissions_cmd, powers_cmd, proxy_cmd, skills_cmd, specs_cmd, steering_cmd, workflows_cmd,
    },
    core::{
        self,
        account::{Account, AccountStore},
    },
    gateway::{self, GatewayConfig},
    server_db::DatabaseStore,
    state::{AppState, PendingLogin},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, sync::Arc};
use tauri::{Manager, State as TauriState};
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
struct ServerContext {
    handle: tauri::AppHandle<tauri::test::MockRuntime>,
    db: Arc<DatabaseStore>,
    admin_token: Arc<String>,
    redirect_uri: Arc<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| headers.get("x-admin-token").and_then(|v| v.to_str().ok()))
}

fn token_matches(provided: &str, expected: &str) -> bool {
    use sha2::{Digest, Sha256};
    let left = Sha256::digest(provided.as_bytes());
    let right = Sha256::digest(expected.as_bytes());
    left == right
}

fn require_admin(headers: &HeaderMap, ctx: &ServerContext) -> Result<(), Response> {
    match bearer(headers) {
        Some(token) if token_matches(token, ctx.admin_token.as_str()) => Ok(()),
        _ => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": { "type": "authentication_error", "message": "管理员令牌无效" }
            })),
        )
            .into_response()),
    }
}

fn tauri_state(ctx: &ServerContext) -> TauriState<'_, AppState> {
    ctx.handle.state::<AppState>()
}

async fn healthz(Extension(ctx): Extension<ServerContext>) -> Response {
    if let Err(error) = ctx.db.ping().await {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "ok": false, "database": false, "error": error.to_string() })),
        )
            .into_response();
    }
    let accounts = AccountStore::new().get_all().len();
    Json(json!({
        "ok": true,
        "service": "kiro-account-manager",
        "accounts": accounts,
        "database": true,
    }))
    .into_response()
}

fn json_result<T: serde::Serialize>(value: T) -> Result<Response, Response> {
    serde_json::to_value(value)
        .map(|value| Json(value).into_response())
        .map_err(|error| internal_error(error.to_string()))
}

fn internal_error(message: String) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": message })),
    )
        .into_response()
}

fn bad_request(message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": message.into() })),
    )
        .into_response()
}

async fn invoke(
    Path(command): Path<String>,
    Extension(ctx): Extension<ServerContext>,
    headers: HeaderMap,
    Json(args): Json<Value>,
) -> Response {
    if let Err(response) = require_admin(&headers, &ctx) {
        return response;
    }

    let result = dispatch_command(&command, &ctx, args).await;
    if result.is_ok() {
        refresh_headless_pool_config();
        if let Err(error) = ctx.db.sync_now().await {
            log::error!("database state sync after {command} failed: {error:#}");
        }
    }
    match result {
        Ok(value) => json_result(value).unwrap_or_else(|response| response),
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response(),
    }
}

fn refresh_headless_pool_config() {
    let Ok(mut config) = gateway::load_gateway_config() else {
        return;
    };
    if config.account_mode != "pool" || !config.pool_account_ids.is_empty() {
        return;
    }
    config.pool_account_ids = AccountStore::new()
        .get_all()
        .into_iter()
        .filter(|account| account.enabled)
        .map(|account| account.id)
        .collect();
    if let Err(error) = gateway::save_gateway_config(&config) {
        log::warn!("刷新 headless 账号池失败: {error}");
    }
}

async fn dispatch_command(
    command: &str,
    ctx: &ServerContext,
    args: Value,
) -> Result<Value, String> {
    let state = tauri_state(ctx);
    let result: Result<Value, String> = match command {
        "get_current_user" => Ok(json!(auth::User {
            id: "railway-admin".to_string(),
            email: None,
            name: "Railway Admin".to_string(),
            avatar: None,
            provider: "admin".to_string(),
        })),
        "logout" => {
            auth_cmd::logout(state);
            Ok(Value::Null)
        }
        "get_supported_providers" => Ok(json!(auth::providers::get_supported_providers())),
        "kiro_login" => headless_start_login(&ctx, &args).await,
        "cancel_kiro_login" => Ok(json!(auth_cmd::cancel_kiro_login(state))),

        "get_accounts" => Ok(json!(account_cmd::get_accounts(state))),
        "get_available_accounts" => Ok(json!(account_cmd::get_available_accounts(state))),
        "get_accounts_by_group" => Ok(json!(account_cmd::get_accounts_by_group(
            state,
            arg_string(&args, "groupId")?,
        ))),
        "get_accounts_by_tag" => Ok(json!(account_cmd::get_accounts_by_tag(
            state,
            arg_string(&args, "tagId")?,
        ))),
        "add_account_by_social" => account_cmd::add_account_by_social(
            state,
            required_string(&args, "refreshToken", "refresh_token")?,
            optional_string(&args, "provider"),
            optional_string(&args, "machineId").or_else(|| optional_string(&args, "machine_id")),
            optional_string(&args, "accessToken")
                .or_else(|| optional_string(&args, "access_token")),
        )
        .await
        .map(|result| json!(result)),
        "add_account_by_idc" => account_cmd::add_account_by_idc(
            state,
            optional_string(&args, "provider"),
            required_string(&args, "refreshToken", "refresh_token")?,
            required_string(&args, "clientId", "client_id")?,
            required_string(&args, "clientSecret", "client_secret")?,
            optional_string(&args, "region"),
            optional_string(&args, "machineId").or_else(|| optional_string(&args, "machine_id")),
            optional_string(&args, "accessToken")
                .or_else(|| optional_string(&args, "access_token")),
            optional_string(&args, "password"),
            optional_string(&args, "startUrl").or_else(|| optional_string(&args, "start_url")),
            optional_string(&args, "clientIdHash")
                .or_else(|| optional_string(&args, "client_id_hash")),
            optional_string(&args, "profileArn").or_else(|| optional_string(&args, "profile_arn")),
            args.get("usageData")
                .or_else(|| args.get("usage_data"))
                .cloned(),
        )
        .await
        .map(|result| json!(result)),
        "add_account_by_external_idp" => account_cmd::add_account_by_external_idp(
            state,
            required_string(&args, "refreshToken", "refresh_token")?,
            required_string(&args, "clientId", "client_id")?,
            required_string(&args, "profileArn", "profile_arn")?,
            optional_string(&args, "clientSecret")
                .or_else(|| optional_string(&args, "client_secret")),
            optional_string(&args, "accessToken")
                .or_else(|| optional_string(&args, "access_token")),
            optional_string(&args, "tokenEndpoint")
                .or_else(|| optional_string(&args, "token_endpoint")),
            optional_string(&args, "issuerUrl").or_else(|| optional_string(&args, "issuer_url")),
            optional_string(&args, "scopes"),
            optional_string(&args, "region"),
            optional_string(&args, "machineId").or_else(|| optional_string(&args, "machine_id")),
            optional_string(&args, "email"),
            optional_string(&args, "expiresAt").or_else(|| optional_string(&args, "expires_at")),
        )
        .await
        .map(|result| json!(result)),
        "import_accounts" => {
            if let Some(raw) = args.get("json").and_then(Value::as_str) {
                account_cmd::import_accounts(state, raw).map(|count| json!(count))
            } else {
                let raw = args.to_string();
                account_cmd::import_accounts(state, &raw).map(|count| json!(count))
            }
        }
        "export_accounts" => {
            let ids = args
                .get("ids")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok());
            Ok(json!(account_cmd::export_accounts(state, ids)))
        }
        "delete_account" => {
            let id = arg_string(&args, "id")?;
            account_cmd::delete_account(state, &id)
                .then_some(Value::Null)
                .ok_or_else(|| "删除账号失败".to_string())
        }
        "delete_accounts" => {
            let ids: Vec<String> =
                serde_json::from_value(args.get("ids").cloned().unwrap_or_default())
                    .map_err(|e| e.to_string())?;
            Ok(json!(account_cmd::delete_accounts(state, ids)))
        }
        "delete_account_remote" => account_cmd::delete_account_remote(
            state,
            arg_string(&args, "id")?,
            args.get("deleteLocal")
                .or_else(|| args.get("delete_local"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await
        .map(|result| json!(result)),
        "update_account" => {
            let params: UpdateAccountParams =
                serde_json::from_value(args.get("params").cloned().unwrap_or(args.clone()))
                    .map_err(|e| e.to_string())?;
            account_cmd::update_account(state, params).map(|account| json!(account))
        }
        "refresh_token" => account_cmd::refresh_token(state, arg_string(&args, "id")?)
            .await
            .map(|account| json!(account)),
        "sync_account" => account_cmd::sync_account(state, arg_string(&args, "id")?)
            .await
            .map(|result| json!(result)),
        "get_usage_limits" => account_cmd::get_usage_limits(state, arg_string(&args, "id")?)
            .await
            .map(|result| json!(result)),
        "get_account_usage" => account_cmd::get_account_usage(
            required_string(&args, "accessToken", "access_token")?,
            optional_string(&args, "provider"),
            optional_string(&args, "machineId").or_else(|| optional_string(&args, "machine_id")),
        )
        .await
        .map(|result| json!(result)),
        "set_overage_status" => account_cmd::set_overage_status(
            state,
            arg_string(&args, "id")?,
            args.get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await
        .map(|result| json!(result)),
        "list_available_models" => account_cmd::list_available_models_for_state(
            &state,
            arg_string(&args, "id")?,
            args.get("forceRefresh")
                .or_else(|| args.get("force_refresh"))
                .and_then(Value::as_bool),
        )
        .await
        .map(|result| json!(result)),
        "check_token_status" => account_cmd::check_token_status(state, arg_string(&args, "id")?)
            .map(|result| json!(result)),
        "check_all_tokens_status" => {
            account_cmd::check_all_tokens_status(state).map(|result| json!(result))
        }
        "refresh_all_expiring_tokens" => account_cmd::refresh_all_expiring_tokens(
            state,
            args.get("onlyExpiring").and_then(Value::as_bool),
            args.get("maxConcurrent")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
        )
        .await
        .map(|result| json!(result)),
        "verify_account" => {
            let params =
                serde_json::from_value(args.get("params").cloned().unwrap_or(args.clone()))
                    .map_err(|error| error.to_string())?;
            account_cmd::verify_account(state, params)
                .await
                .map(|result| json!(result))
        }

        "get_gateway_config" => gateway::get_gateway_config().map(|config| json!(config)),
        "save_gateway_config" => {
            let config: GatewayConfig =
                serde_json::from_value(args.get("config").cloned().unwrap_or(args.clone()))
                    .map_err(|e| e.to_string())?;
            gateway::save_gateway_config(&config).map(|_| Value::Null)
        }
        "start_gateway" => {
            let config: GatewayConfig =
                serde_json::from_value(args.get("config").cloned().unwrap_or(args.clone()))
                    .map_err(|e| e.to_string())?;
            gateway_cmd::start_gateway(state, config)
                .await
                .map(|status| json!(status))
        }
        "stop_gateway" => gateway_cmd::stop_gateway(state).await.map(|_| Value::Null),
        "get_gateway_status" => gateway_cmd::get_gateway_status(state)
            .await
            .map(|status| json!(status)),
        "get_gateway_log_dir" => {
            gateway::gateway_log_dir_path().map(|path| json!(path.to_string_lossy().to_string()))
        }
        // A Railway process cannot open a folder on the user's desktop. Return
        // the persistent log path so the web UI can display/copy it instead.
        "open_gateway_log_dir" => {
            gateway::gateway_log_dir_path().map(|path| json!(path.to_string_lossy().to_string()))
        }
        "get_gateway_request_logs" => gateway_cmd::get_gateway_request_logs(
            state,
            args.get("limit")
                .and_then(Value::as_u64)
                .map(|v| v as usize),
        )
        .await
        .map(|logs| json!(logs)),
        "get_gateway_request_stats" => gateway_cmd::get_gateway_request_stats(state)
            .await
            .map(|stats| json!(stats)),
        "get_gateway_model_stats" => gateway_cmd::get_gateway_model_stats(state)
            .await
            .map(|stats| json!(stats)),
        "get_gateway_endpoint_stats" => gateway_cmd::get_gateway_endpoint_stats(state)
            .await
            .map(|stats| json!(stats)),
        "clear_gateway_request_logs" => gateway_cmd::clear_gateway_request_logs(state)
            .await
            .map(|_| Value::Null),
        "get_all_account_health" => gateway_cmd::get_all_account_health(state)
            .await
            .map(|health| json!(health)),
        "reset_account_health" => {
            gateway_cmd::reset_account_health(state, arg_string(&args, "accountId")?)
                .await
                .map(|_| Value::Null)
        }
        "get_rate_limited_accounts" => gateway_cmd::get_rate_limited_accounts(state)
            .await
            .map(|accounts| json!(accounts)),
        "clear_rate_limit_account" => {
            gateway_cmd::clear_rate_limit_account(state, arg_string(&args, "accountId")?)
                .await
                .map(|_| Value::Null)
        }
        "cleanup_stale_health" => gateway_cmd::cleanup_stale_health(state)
            .await
            .map(|_| Value::Null),
        "get_banned_accounts" => gateway_cmd::get_banned_accounts(state)
            .await
            .map(|accounts| json!(accounts)),
        "test_route_config" => {
            let config: GatewayConfig =
                serde_json::from_value(args.get("config").cloned().unwrap_or(args.clone()))
                    .map_err(|error| error.to_string())?;
            gateway_cmd::test_route_config(state, config)
                .await
                .map(|result| json!(result))
        }
        "configure_proxy_clients" => {
            let clients = args
                .get("clients")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(json!(clients
                .into_iter()
                .map(|client| json!({
                    "client": client,
                    "success": false,
                    "paths": [],
                    "error": "Railway Web 模式不能写入调用方本机配置，请手动使用网关 URL 和 API Key"
                }))
                .collect::<Vec<_>>()))
        }
        "get_available_models" => Ok(json!(gateway_cmd::get_available_models())),
        "get_cache_stats" => cache_cmd::get_cache_stats(state)
            .await
            .map(|stats| json!(stats)),
        "clear_all_cache" => cache_cmd::clear_all_cache(state).await.map(|_| Value::Null),
        "clear_session_cache" => {
            cache_cmd::clear_session_cache(state, arg_string(&args, "sessionId")?)
                .await
                .map(|_| Value::Null)
        }
        "cleanup_expired_cache" => cache_cmd::cleanup_expired_cache(state)
            .await
            .map(|count| json!(count)),

        "detect_system_proxy" => proxy_cmd::detect_system_proxy()
            .await
            .map(|result| json!(result)),
        "test_account_proxy" => {
            let proxy_config = serde_json::from_value(
                args.get("proxyConfig")
                    .or_else(|| args.get("proxy_config"))
                    .cloned()
                    .unwrap_or(args.clone()),
            )
            .map_err(|error| error.to_string())?;
            proxy_cmd::test_account_proxy(proxy_config)
                .await
                .map(|result| json!(result))
        }

        "get_app_settings" => app_settings_cmd::get_app_settings()
            .await
            .map(|settings| json!(settings)),
        "save_app_settings" => {
            let settings: AppSettings =
                serde_json::from_value(args.get("settings").cloned().unwrap_or(args.clone()))
                    .map_err(|e| e.to_string())?;
            app_settings_cmd::save_app_settings(settings)
                .await
                .map(|_| Value::Null)
        }
        "get_usage_history" => app_settings_cmd::get_usage_history()
            .await
            .map(|history| json!(history)),
        "save_usage_history_entry" => {
            let entry: app_settings_cmd::UsageHistoryEntry =
                serde_json::from_value(args.get("entry").cloned().unwrap_or(args.clone()))
                    .map_err(|e| e.to_string())?;
            app_settings_cmd::save_usage_history_entry(entry)
                .await
                .map(|_| Value::Null)
        }
        "get_app_data_dir" => Ok(json!(core::paths::app_data_dir_or_default()
            .display()
            .to_string())),
        "open_app_data_dir" => Ok(json!(core::paths::app_data_dir_or_default()
            .display()
            .to_string())),
        "get_custom_kiro_path" => app_settings_cmd::get_custom_kiro_path()
            .await
            .map(|path| json!(path)),
        "set_custom_kiro_path" => {
            app_settings_cmd::set_custom_kiro_path(arg_string(&args, "path")?)
                .await
                .map(|_| Value::Null)
        }
        "clear_custom_kiro_path" => app_settings_cmd::clear_custom_kiro_path()
            .await
            .map(|_| Value::Null),
        "open_kiro_settings_file" => {
            Err("Railway 无头服务没有本机 Kiro 设置文件；请通过管理 API 修改服务端设置".to_string())
        }

        "get_kiro_settings" => kiro_settings_cmd::get_kiro_settings()
            .await
            .map(|settings| json!(settings)),
        "set_kiro_proxy" => kiro_settings_cmd::set_kiro_proxy(arg_string(&args, "proxy")?)
            .await
            .map(|_| Value::Null),
        "set_kiro_model" => kiro_settings_cmd::set_kiro_model(arg_string(&args, "model")?)
            .await
            .map(|_| Value::Null),
        "set_kiro_codebase_indexing" => kiro_settings_cmd::set_kiro_codebase_indexing(
            args.get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await
        .map(|_| Value::Null),
        "set_kiro_agent_autonomy" => {
            kiro_settings_cmd::set_kiro_agent_autonomy(arg_string(&args, "autonomy")?)
                .await
                .map(|_| Value::Null)
        }
        "set_kiro_tab_autocomplete" => kiro_settings_cmd::set_kiro_tab_autocomplete(
            args.get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await
        .map(|_| Value::Null),
        "set_kiro_usage_summary" => kiro_settings_cmd::set_kiro_usage_summary(
            args.get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await
        .map(|_| Value::Null),
        "set_kiro_debug_logs" => kiro_settings_cmd::set_kiro_debug_logs(
            args.get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await
        .map(|_| Value::Null),
        "set_kiro_notification" => kiro_settings_cmd::set_kiro_notification(
            arg_string(&args, "key")?,
            args.get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await
        .map(|_| Value::Null),
        "set_kiro_reference_tracker" => kiro_settings_cmd::set_kiro_reference_tracker(
            args.get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await
        .map(|_| Value::Null),
        "set_kiro_configure_mcp" => {
            kiro_settings_cmd::set_kiro_configure_mcp(arg_string(&args, "mode")?)
                .await
                .map(|_| Value::Null)
        }
        "set_kiro_telemetry" => kiro_settings_cmd::set_kiro_telemetry(
            arg_string(&args, "key")?,
            args.get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await
        .map(|_| Value::Null),
        "set_kiro_agent_setting" => kiro_settings_cmd::set_kiro_agent_setting(
            arg_string(&args, "key")?,
            args.get("value").cloned().unwrap_or(Value::Null),
        )
        .await
        .map(|_| Value::Null),
        "read_cloud_config_enabled" => kiro_settings_cmd::read_cloud_config_enabled()
            .await
            .map(|value| json!(value)),
        "read_remote_sessions_env_override" => {
            kiro_settings_cmd::read_remote_sessions_env_override()
                .await
                .map(|value| json!(value))
        }

        "generate_machine_guid" => Ok(json!(machine_guid::generate_machine_guid())),
        "set_custom_machine_guid" => {
            machine_guid::set_custom_machine_guid(arg_string(&args, "newGuid")?)
                .await
                .map(|value| json!(value))
        }

        "get_permissions" => permissions_cmd::get_permissions(
            optional_string(&args, "scope"),
            optional_string(&args, "projectPath")
                .or_else(|| optional_string(&args, "project_path")),
        )
        .await
        .map(|policy| json!(policy)),
        "save_permissions" => {
            let policy =
                serde_json::from_value(args.get("policy").cloned().ok_or("缺少参数: policy")?)
                    .map_err(|error| error.to_string())?;
            permissions_cmd::save_permissions(
                policy,
                optional_string(&args, "scope"),
                optional_string(&args, "projectPath")
                    .or_else(|| optional_string(&args, "project_path")),
            )
            .await
            .map(|_| Value::Null)
        }
        "get_permission_capabilities" => permissions_cmd::get_permission_capabilities()
            .await
            .map(|value| json!(value)),
        "list_permission_workspace_roots" => permissions_cmd::list_permission_workspace_roots()
            .await
            .map(|value| json!(value)),

        "get_mcp_config" => mcp_cmd::get_mcp_config(
            optional_string(&args, "projectDir").or_else(|| optional_string(&args, "project_dir")),
        )
        .await
        .map(|value| json!(value)),
        "get_mcp_tool_stats" => mcp_cmd::get_mcp_tool_stats(
            optional_string(&args, "projectDir").or_else(|| optional_string(&args, "project_dir")),
        )
        .await
        .map(|value| json!(value)),
        "save_mcp_server" => {
            let config =
                serde_json::from_value(args.get("config").cloned().ok_or("缺少参数: config")?)
                    .map_err(|error| error.to_string())?;
            mcp_cmd::save_mcp_server(
                arg_string(&args, "name")?,
                config,
                optional_string(&args, "projectDir")
                    .or_else(|| optional_string(&args, "project_dir")),
            )
            .await
            .map(|_| Value::Null)
        }
        "delete_mcp_server" => mcp_cmd::delete_mcp_server(
            arg_string(&args, "name")?,
            optional_string(&args, "projectDir").or_else(|| optional_string(&args, "project_dir")),
        )
        .await
        .map(|_| Value::Null),
        "toggle_mcp_server" => mcp_cmd::toggle_mcp_server(
            arg_string(&args, "name")?,
            args.get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            optional_string(&args, "projectDir").or_else(|| optional_string(&args, "project_dir")),
        )
        .await
        .map(|_| Value::Null),

        // Web administration can manage the same Kiro project files as the
        // desktop UI. The managers themselves enforce path and file rules.
        "get_custom_agents" => custom_agents_cmd::get_custom_agents(project_dir(&args))
            .await
            .map(|v| json!(v)),
        "get_custom_agent" => custom_agents_cmd::get_custom_agent(
            arg_string(&args, "fileName")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|v| json!(v)),
        "save_custom_agent" => custom_agents_cmd::save_custom_agent(
            arg_string(&args, "fileName")?,
            arg_string(&args, "content")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|_| Value::Null),
        "delete_custom_agent" => custom_agents_cmd::delete_custom_agent(
            arg_string(&args, "fileName")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|_| Value::Null),
        "create_custom_agent" => custom_agents_cmd::create_custom_agent(
            arg_string(&args, "fileName")?,
            arg_string(&args, "content")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|v| json!(v)),

        "get_hooks" => hooks_cmd::get_hooks(project_dir(&args))
            .await
            .map(|v| json!(v)),
        "get_hook" => hooks_cmd::get_hook(
            arg_string(&args, "fileName")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|v| json!(v)),
        "save_hook" => hooks_cmd::save_hook(
            arg_string(&args, "fileName")?,
            arg_string(&args, "content")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|_| Value::Null),
        "delete_hook" => hooks_cmd::delete_hook(
            arg_string(&args, "fileName")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|_| Value::Null),
        "create_hook" => hooks_cmd::create_hook(
            arg_string(&args, "fileName")?,
            arg_string(&args, "content")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|v| json!(v)),

        "get_skills" => skills_cmd::get_skills(project_dir(&args))
            .await
            .map(|v| json!(v)),
        "get_skill" => skills_cmd::get_skill(
            arg_string(&args, "name")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|v| json!(v)),
        "save_skill" => skills_cmd::save_skill(
            arg_string(&args, "name")?,
            arg_string(&args, "content")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|_| Value::Null),
        "delete_skill" => skills_cmd::delete_skill(
            arg_string(&args, "name")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|_| Value::Null),
        "create_skill" => skills_cmd::create_skill(
            arg_string(&args, "name")?,
            arg_string(&args, "content")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|v| json!(v)),
        "import_skill_local" => skills_cmd::import_skill_local(
            arg_string(&args, "sourcePath")?,
            optional_string(&args, "targetName"),
            optional_string(&args, "scope"),
            project_dir(&args),
            args.get("overwrite").and_then(Value::as_bool),
        )
        .await
        .map(|v| json!(v)),
        "import_skill_from_github" => skills_cmd::import_skill_from_github(
            arg_string(&args, "repoUrl")?,
            optional_string(&args, "pathInRepo"),
            optional_string(&args, "branch"),
            optional_string(&args, "targetName"),
            optional_string(&args, "scope"),
            project_dir(&args),
            args.get("overwrite").and_then(Value::as_bool),
        )
        .await
        .map(|v| json!(v)),

        "get_steering_files" => steering_cmd::get_steering_files(project_dir(&args))
            .await
            .map(|v| json!(v)),
        "get_steering_file" => steering_cmd::get_steering_file(
            arg_string(&args, "fileName")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|v| json!(v)),
        "save_steering_file" => steering_cmd::save_steering_file(
            arg_string(&args, "fileName")?,
            arg_string(&args, "content")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|_| Value::Null),
        "delete_steering_file" => steering_cmd::delete_steering_file(
            arg_string(&args, "fileName")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|_| Value::Null),
        "create_steering_file" => steering_cmd::create_steering_file(
            arg_string(&args, "fileName")?,
            arg_string(&args, "content")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|v| json!(v)),
        "create_default_steering_file" => steering_cmd::create_default_steering_file(
            optional_string(&args, "fileName"),
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|v| json!(v)),
        "create_initial_project_steering" => {
            steering_cmd::create_initial_project_steering(arg_string(&args, "projectDir")?)
                .await
                .map(|v| json!(v))
        }
        "refine_steering_file" => steering_cmd::refine_steering_file(
            arg_string(&args, "fileName")?,
            optional_string(&args, "scope"),
            project_dir(&args),
        )
        .await
        .map(|v| json!(v)),
        "scan_agents_md" => steering_cmd::scan_agents_md(arg_string(&args, "projectDir")?)
            .await
            .map(|v| json!(v)),
        "list_agents_md_ignore_files" => {
            steering_cmd::list_agents_md_ignore_files(arg_string(&args, "projectDir")?)
                .await
                .map(|v| json!(v))
        }
        "get_agents_md" => steering_cmd::get_agents_md(
            arg_string(&args, "projectDir")?,
            arg_string(&args, "relPath")?,
        )
        .await
        .map(|v| json!(v)),
        "save_agents_md" => steering_cmd::save_agents_md(
            arg_string(&args, "projectDir")?,
            arg_string(&args, "relPath")?,
            arg_string(&args, "content")?,
        )
        .await
        .map(|_| Value::Null),

        "list_specs" => specs_cmd::list_specs(arg_string(&args, "scope")?, project_dir(&args))
            .await
            .map(|v| json!(v)),
        "read_spec" => specs_cmd::read_spec(
            arg_string(&args, "scope")?,
            project_dir(&args),
            arg_string(&args, "name")?,
        )
        .await
        .map(|v| json!(v)),
        "save_spec_file" => specs_cmd::save_spec_file(
            arg_string(&args, "scope")?,
            project_dir(&args),
            arg_string(&args, "name")?,
            arg_string(&args, "fileKind")?,
            arg_string(&args, "content")?,
        )
        .await
        .map(|_| Value::Null),
        "create_spec" => specs_cmd::create_spec(
            arg_string(&args, "scope")?,
            project_dir(&args),
            arg_string(&args, "name")?,
        )
        .await
        .map(|_| Value::Null),
        "delete_spec" => specs_cmd::delete_spec(
            arg_string(&args, "scope")?,
            project_dir(&args),
            arg_string(&args, "name")?,
        )
        .await
        .map(|_| Value::Null),

        "list_workflows" => {
            workflows_cmd::list_workflows(arg_string(&args, "scope")?, project_dir(&args))
                .await
                .map(|v| json!(v))
        }
        "read_workflow" => workflows_cmd::read_workflow(
            arg_string(&args, "scope")?,
            project_dir(&args),
            arg_string(&args, "fileName")?,
        )
        .await
        .map(|v| json!(v)),
        "save_workflow" => workflows_cmd::save_workflow(
            arg_string(&args, "scope")?,
            project_dir(&args),
            arg_string(&args, "fileName")?,
            arg_string(&args, "content")?,
        )
        .await
        .map(|_| Value::Null),
        "create_workflow" => workflows_cmd::create_workflow(
            arg_string(&args, "scope")?,
            project_dir(&args),
            arg_string(&args, "fileName")?,
        )
        .await
        .map(|_| Value::Null),
        "delete_workflow" => workflows_cmd::delete_workflow(
            arg_string(&args, "scope")?,
            project_dir(&args),
            arg_string(&args, "fileName")?,
        )
        .await
        .map(|_| Value::Null),

        "get_powers" => powers_cmd::get_powers().await.map(|v| json!(v)),
        "get_power" => powers_cmd::get_power(arg_string(&args, "name")?)
            .await
            .map(|v| json!(v)),
        "install_power" => powers_cmd::install_power(
            arg_string(&args, "name")?,
            arg_string(&args, "cloneUrl")?,
            arg_string(&args, "pathInRepo")?,
            arg_string(&args, "branch")?,
        )
        .await
        .map(|_| Value::Null),
        "install_power_from_local" => {
            powers_cmd::install_power_from_local(arg_string(&args, "sourceDir")?)
                .await
                .map(|v| json!(v))
        }
        "install_power_from_url" => powers_cmd::install_power_from_url(arg_string(&args, "url")?)
            .await
            .map(|v| json!(v)),
        "uninstall_power" => powers_cmd::uninstall_power(arg_string(&args, "name")?)
            .await
            .map(|_| Value::Null),
        "get_power_registries" => powers_cmd::get_power_registries().await.map(|v| json!(v)),
        "get_user_added_powers" => powers_cmd::get_user_added_powers().await.map(|v| json!(v)),
        "get_recommended_powers" => powers_cmd::get_recommended_powers().await.map(|v| json!(v)),

        "get_groups" => Ok(json!(
            kiro_account_manager::commands::group_tag_cmd::get_groups(state)
        )),
        "get_tags" => Ok(json!(
            kiro_account_manager::commands::group_tag_cmd::get_tags(state)
        )),
        "add_group" => kiro_account_manager::commands::group_tag_cmd::add_group(
            state,
            arg_string(&args, "name")?,
            optional_string(&args, "color"),
        )
        .map(|result| json!(result)),
        "update_group" => kiro_account_manager::commands::group_tag_cmd::update_group(
            state,
            arg_string(&args, "id")?.as_str(),
            optional_string(&args, "name"),
            optional_string(&args, "color"),
        )
        .map(|result| json!(result)),
        "delete_group" => Ok(json!(
            kiro_account_manager::commands::group_tag_cmd::delete_group(
                state,
                arg_string(&args, "id")?.as_str(),
            )
        )),
        "reorder_groups" => {
            let ids: Vec<String> =
                serde_json::from_value(args.get("ids").cloned().unwrap_or_default())
                    .map_err(|error| error.to_string())?;
            Ok(json!(
                kiro_account_manager::commands::group_tag_cmd::reorder_groups(state, ids)
            ))
        }
        "add_tag" => kiro_account_manager::commands::group_tag_cmd::add_tag(
            state,
            arg_string(&args, "name")?,
            required_string(&args, "color", "colour")?,
        )
        .map(|result| json!(result)),
        "update_tag" => kiro_account_manager::commands::group_tag_cmd::update_tag(
            state,
            arg_string(&args, "id")?.as_str(),
            optional_string(&args, "name"),
            optional_string(&args, "color"),
        )
        .map(|result| json!(result)),
        "delete_tag" => Ok(json!(
            kiro_account_manager::commands::group_tag_cmd::delete_tag(
                state,
                arg_string(&args, "id")?.as_str(),
            )
        )),
        "set_account_group" => kiro_account_manager::commands::group_tag_cmd::set_account_group(
            state,
            arg_string(&args, "accountId")?.as_str(),
            args.get("groupId")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        )
        .map(|_| Value::Null),
        "add_tag_to_account" => kiro_account_manager::commands::group_tag_cmd::add_tag_to_account(
            state,
            arg_string(&args, "accountId")?.as_str(),
            arg_string(&args, "tagId")?.as_str(),
        )
        .map(|_| Value::Null),
        "remove_tag_from_account" => {
            kiro_account_manager::commands::group_tag_cmd::remove_tag_from_account(
                state,
                arg_string(&args, "accountId")?.as_str(),
                arg_string(&args, "tagId")?.as_str(),
            )
            .map(|_| Value::Null)
        }
        "set_account_tags" => {
            let tag_ids: Vec<String> =
                serde_json::from_value(args.get("tagIds").cloned().unwrap_or_default())
                    .map_err(|error| error.to_string())?;
            kiro_account_manager::commands::group_tag_cmd::set_account_tags(
                state,
                arg_string(&args, "accountId")?.as_str(),
                tag_ids,
            )
            .map(|_| Value::Null)
        }
        "remove_account_tags" => {
            let tag_ids: Vec<String> =
                serde_json::from_value(args.get("tagIds").cloned().unwrap_or_default())
                    .map_err(|error| error.to_string())?;
            kiro_account_manager::commands::group_tag_cmd::remove_account_tags(
                state,
                arg_string(&args, "accountId")?.as_str(),
                tag_ids,
            )
            .map(|_| Value::Null)
        }
        _ => Err(format!("不支持的管理命令: {command}")),
    };
    result
}

fn arg_string(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("缺少参数: {key}"))
}

fn optional_string(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn project_dir(args: &Value) -> Option<String> {
    optional_string(args, "projectDir").or_else(|| optional_string(args, "project_dir"))
}

fn required_string(args: &Value, primary: &str, fallback: &str) -> Result<String, String> {
    optional_string(args, primary)
        .or_else(|| optional_string(args, fallback))
        .ok_or_else(|| format!("缺少参数: {primary}"))
}

async fn headless_start_login(ctx: &ServerContext, args: &Value) -> Result<Value, String> {
    let provider = args
        .get("provider")
        .and_then(Value::as_str)
        .ok_or("缺少 provider")?;
    let config =
        get_provider_config(provider).ok_or_else(|| format!("不支持的 provider: {provider}"))?;
    if config.auth_method != auth::providers::AuthMethod::Social {
        return Err(
            "Railway 无头登录目前支持 Google/Github；BuilderId/Enterprise 请导入账号 JSON"
                .to_string(),
        );
    }

    let machine_id = generate_account_machine_id();
    let state = uuid::Uuid::new_v4().to_string();
    let verifier = auth::auth_social::generate_code_verifier_social();
    let challenge = auth::auth_social::generate_code_challenge_social(&verifier);
    let client = KiroAuthServiceClient::new(&machine_id)?;
    let authorization_url =
        client.authorization_url(provider, ctx.redirect_uri.as_str(), &challenge, &state);

    *lock_store(&tauri_state(ctx).pending_login, "pending_login")? = Some(PendingLogin {
        provider: provider.to_string(),
        code_verifier: verifier,
        state: state.clone(),
        machineid: machine_id,
    });

    Ok(json!({ "authorizationUrl": authorization_url, "state": state }))
}

async fn oauth_callback(
    Extension(ctx): Extension<ServerContext>,
    Query(query): Query<OAuthQuery>,
) -> Response {
    if let Some(error) = query.error {
        return Html(format!("<h2>登录失败</h2><p>{}</p>", html_escape(&error))).into_response();
    }
    let code = match query.code {
        Some(code) if !code.trim().is_empty() => code,
        _ => return bad_request("OAuth 回调缺少 code"),
    };
    let callback_state = match query.state {
        Some(state) => state,
        None => return bad_request("OAuth 回调缺少 state"),
    };

    let pending = {
        let app_state = tauri_state(&ctx);
        let locked = lock_store(&app_state.pending_login, "pending_login");
        let pending = match locked {
            Ok(mut slot) => slot.take(),
            Err(error) => return internal_error(error),
        };
        // Keep the mutex guard's temporary out of the block tail. Without an
        // explicit binding, Rust can drop the Result after `app_state`, which
        // also makes the Axum handler future fail the `Send` bound.
        pending
    };
    let Some(pending) = pending else {
        return bad_request("登录会话不存在或已过期，请重新开始登录");
    };
    if pending.state != callback_state {
        return bad_request("OAuth state 校验失败");
    }

    let client = match KiroAuthServiceClient::new(&pending.machineid) {
        Ok(client) => client,
        Err(error) => return internal_error(error),
    };
    let token: auth::providers::SocialTokenResponse = match client
        .create_token(
            &code,
            &pending.code_verifier,
            ctx.redirect_uri.as_str(),
            None,
        )
        .await
    {
        Ok(token) => token,
        Err(error) => return internal_error(error),
    };
    let usage = match get_usage_by_provider_with_machine_id(
        &pending.provider,
        &token.access_token,
        &pending.machineid,
    )
    .await
    {
        Ok(usage) => usage,
        Err(error) => return internal_error(error),
    };
    if usage.is_banned {
        return bad_request("账号已被封禁");
    }

    let (email, user_id) =
        kiro_account_manager::commands::common::extract_user_info(&usage.usage_data);
    let display = email
        .clone()
        .or_else(|| user_id.clone())
        .unwrap_or_else(|| {
            format!(
                "{}_{}",
                pending.provider.to_lowercase(),
                &token.refresh_token[..8.min(token.refresh_token.len())]
            )
        });
    let account = {
        let app_state = tauri_state(&ctx);
        let locked = lock_store(&app_state.store, "store");
        let mut store = match locked {
            Ok(store) => store,
            Err(error) => return internal_error(error),
        };
        let existing = find_existing_account_idx(
            &store.accounts,
            email.as_ref(),
            &pending.provider,
            &token.refresh_token,
            user_id.as_ref(),
        );
        let account = if let Some(index) = existing {
            let account = &mut store.accounts[index];
            account.access_token = Some(token.access_token.clone());
            account.refresh_token = Some(token.refresh_token.clone());
            account.profile_arn = token.profile_arn.clone();
            account.user_id = user_id;
            account.usage_data = Some(usage.usage_data);
            account.machine_id = Some(pending.machineid.clone());
            update_account_status(account, usage.is_banned, usage.is_auth_error);
            account.clone()
        } else {
            let mut account = Account::new(display, format!("Kiro {} 账号", pending.provider));
            account.email = email;
            account.access_token = Some(token.access_token.clone());
            account.refresh_token = Some(token.refresh_token.clone());
            account.profile_arn = token.profile_arn.clone();
            account.provider = Some(pending.provider.clone());
            account.auth_method = Some("social".to_string());
            account.user_id = user_id;
            account.usage_data = Some(usage.usage_data);
            account.machine_id = Some(pending.machineid.clone());
            update_account_status(&mut account, usage.is_banned, usage.is_auth_error);
            store.accounts.insert(0, account.clone());
            account
        };
        if let Err(error) = save_store(&store) {
            return internal_error(error);
        }
        account
    };
    let _ = ctx.db.sync_now().await;
    Html(format!(
        "<h2>登录成功</h2><p>{}</p><p>可以关闭此页面并返回管理后台。</p>",
        html_escape(&account.get_display_id())
    ))
    .into_response()
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn build_routes(ctx: ServerContext) -> Router<gateway::RouterState> {
    let static_files = ServeDir::new("dist").not_found_service(ServeFile::new("dist/index.html"));
    Router::<gateway::RouterState>::new()
        .route("/healthz", get(healthz))
        .route("/api/invoke/{command}", post(invoke))
        .route("/api/auth/callback", get(oauth_callback))
        .fallback_service(static_files)
        .layer(Extension(ctx))
}

fn server_config(port: u16) -> Result<GatewayConfig, String> {
    let mut config = gateway::load_gateway_config().unwrap_or_default();
    config.enabled = true;
    config.host = "0.0.0.0".to_string();
    config.port = port;
    config.local_only = false;
    config.allowed_ips = vec!["0.0.0.0/0".to_string(), "::/0".to_string()];
    if gateway::effective_client_api_keys(&config).is_empty() {
        // Bootstrap a usable key so the administrator can immediately open
        // the web console and rotate/manage keys there. The value is written
        // to the database-backed gateway config below and is never logged.
        let key = format!("sk-{}", uuid::Uuid::new_v4().simple());
        config.client_api_keys = vec![key.clone()];
        config.access_token = Some(key);
    }
    if config.pool_account_ids.is_empty() && config.account_mode == "pool" {
        let accounts = AccountStore::new().get_all();
        config.pool_account_ids = accounts
            .into_iter()
            .filter(|account| account.enabled)
            .map(|account| account.id)
            .collect();
    }
    gateway::save_gateway_config(&config)?;
    Ok(config)
}

fn clean_env_value(value: String) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn database_url_from_env() -> anyhow::Result<String> {
    let names = [
        "DATABASE_URL",
        "DATABASE_PRIVATE_URL",
        "POSTGRES_URL",
        "POSTGRESQL_URL",
        "DATABASE_PUBLIC_URL",
    ];
    let mut invalid = Vec::new();
    for name in names {
        let Ok(raw) = env::var(name) else { continue };
        let value = clean_env_value(raw);
        if value.is_empty() || value.contains("${{") || value.contains("}}") {
            invalid.push(name);
            continue;
        }
        if value.starts_with("postgres://") || value.starts_with("postgresql://") {
            return Ok(value);
        }
        invalid.push(name);
    }
    if invalid.is_empty() {
        anyhow::bail!("必须设置 DATABASE_URL（Railway Postgres），且值必须是 postgres:// 或 postgresql:// URL");
    }
    anyhow::bail!(
        "Railway 数据库连接变量无效（{}）。请在 Variables 中引用 Postgres 服务的 DATABASE_PRIVATE_URL，且不要保留 ${{{{...}}}} 模板或引号",
        invalid.join(", ")
    )
}

fn public_base_url_from_env(port: u16) -> String {
    let raw = env::var("PUBLIC_BASE_URL")
        .or_else(|_| env::var("RAILWAY_PUBLIC_DOMAIN"))
        .ok()
        .map(clean_env_value)
        .filter(|value| !value.is_empty());
    let value = raw.unwrap_or_else(|| format!("http://127.0.0.1:{port}"));
    if value.starts_with("http://") || value.starts_with("https://") {
        value.trim_end_matches('/').to_string()
    } else {
        format!("https://{}", value.trim_end_matches('/'))
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let admin_token = clean_env_value(
        env::var("ADMIN_TOKEN").map_err(|_| anyhow::anyhow!("必须设置 ADMIN_TOKEN"))?,
    );
    if admin_token.trim().is_empty() {
        anyhow::bail!("ADMIN_TOKEN 不能为空");
    }
    let database_url = database_url_from_env()?;
    let port = env::var("PORT")
        .unwrap_or_else(|_| "8765".to_string())
        .parse::<u16>()?;
    let data_dir = core::paths::app_data_dir_or_default();
    tokio::fs::create_dir_all(&data_dir).await?;

    let db = Arc::new(DatabaseStore::connect(&database_url, data_dir).await?);
    db.restore().await?;
    let app = AppState {
        store: std::sync::Mutex::new(AccountStore::new()),
        group_tag_store: std::sync::Mutex::new(core::account::GroupTagStore::new()),
        auth: auth::AuthState::new(),
        pending_login: std::sync::Mutex::new(None),
        gateway: std::sync::Mutex::new(None),
    };
    let base_url = public_base_url_from_env(port);
    let tauri_app = tauri::test::mock_builder()
        .manage(app)
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .map_err(|error| anyhow::anyhow!("初始化 headless Tauri state 失败: {error}"))?;
    let ctx = ServerContext {
        handle: tauri_app.handle().clone(),
        db: db.clone(),
        admin_token: Arc::new(admin_token),
        redirect_uri: Arc::new(format!("{base_url}/api/auth/callback")),
    };

    gateway::install_extra_routes(build_routes(ctx.clone()))
        .map_err(|_| anyhow::anyhow!("重复安装 server routes"))?;
    let config = server_config(port).map_err(anyhow::Error::msg)?;
    let state = ctx.handle.state::<AppState>();
    gateway::start_gateway(&state, config)
        .await
        .map_err(anyhow::Error::msg)?;
    db.sync_now().await?;
    db.clone().spawn_sync_loop();
    log::info!("headless server listening on 0.0.0.0:{port}");

    shutdown_signal().await;
    gateway::stop_gateway(&state)
        .await
        .map_err(anyhow::Error::msg)?;
    db.sync_now().await?;
    log::info!("headless server stopped at {}", Utc::now());
    Ok(())
}
