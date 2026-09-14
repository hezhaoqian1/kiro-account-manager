// Hooks 管理命令
//
// scope 参数："user" 表示用户级（~/.kiro/hooks，Kiro IDE 1.0.182+），
// "project" 表示项目级（<project>/.kiro/hooks）。
// 为保持与旧前端兼容，scope 缺省时按 "project" 处理。

use crate::commands::common::run_blocking_task;
use crate::kiro::settings::hooks::{HookFile, HooksManager};
use tauri::command;

#[command]
pub async fn get_hooks(project_dir: Option<String>) -> Result<Vec<HookFile>, String> {
    run_blocking_task(move || HooksManager::load_all(project_dir.as_deref())).await
}

#[command]
pub async fn get_hook(
    file_name: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<HookFile, String> {
    run_blocking_task(move || {
        HooksManager::load(
            &file_name,
            scope.as_deref().unwrap_or("project"),
            project_dir.as_deref(),
        )
    })
    .await
}

#[command]
pub async fn save_hook(
    file_name: String,
    content: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<(), String> {
    run_blocking_task(move || {
        HooksManager::save(
            &file_name,
            &content,
            scope.as_deref().unwrap_or("project"),
            project_dir.as_deref(),
        )
    })
    .await
}

#[command]
pub async fn delete_hook(
    file_name: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<(), String> {
    run_blocking_task(move || {
        HooksManager::delete(
            &file_name,
            scope.as_deref().unwrap_or("project"),
            project_dir.as_deref(),
        )
    })
    .await
}

#[command]
pub async fn create_hook(
    file_name: String,
    content: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<HookFile, String> {
    run_blocking_task(move || {
        HooksManager::create(
            &file_name,
            &content,
            scope.as_deref().unwrap_or("project"),
            project_dir.as_deref(),
        )
    })
    .await
}
