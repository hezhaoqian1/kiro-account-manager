// Steering 管理命令

use crate::commands::common::run_blocking_task;
use crate::kiro::settings::steering::{AgentsMdFile, SteeringFile, SteeringManager};
use tauri::command;

#[command]
pub async fn get_steering_files(project_dir: Option<String>) -> Result<Vec<SteeringFile>, String> {
    run_blocking_task(move || SteeringManager::load_all(project_dir.as_deref())).await
}

#[command]
pub async fn get_steering_file(
    file_name: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<SteeringFile, String> {
    let scope = scope.unwrap_or_else(|| "user".to_string());
    run_blocking_task(move || SteeringManager::load(&file_name, &scope, project_dir.as_deref()))
        .await
}

#[command]
pub async fn save_steering_file(
    file_name: String,
    content: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<(), String> {
    let scope = scope.unwrap_or_else(|| "user".to_string());
    run_blocking_task(move || {
        SteeringManager::save(&file_name, &content, &scope, project_dir.as_deref())
    })
    .await
}

#[command]
pub async fn delete_steering_file(
    file_name: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<(), String> {
    let scope = scope.unwrap_or_else(|| "user".to_string());
    run_blocking_task(move || SteeringManager::delete(&file_name, &scope, project_dir.as_deref()))
        .await
}

#[command]
pub async fn create_steering_file(
    file_name: String,
    content: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<SteeringFile, String> {
    let scope = scope.unwrap_or_else(|| "user".to_string());
    run_blocking_task(move || {
        SteeringManager::create(&file_name, &content, &scope, project_dir.as_deref())
    })
    .await
}

#[command]
pub async fn create_default_steering_file(
    file_name: Option<String>,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<SteeringFile, String> {
    let scope = scope.unwrap_or_else(|| "user".to_string());
    run_blocking_task(move || {
        SteeringManager::create_default(file_name.as_deref(), &scope, project_dir.as_deref())
    })
    .await
}

#[command]
pub async fn create_initial_project_steering(
    project_dir: String,
) -> Result<Vec<SteeringFile>, String> {
    run_blocking_task(move || SteeringManager::create_initial_for_project(&project_dir)).await
}

#[command]
pub async fn refine_steering_file(
    file_name: String,
    scope: Option<String>,
    project_dir: Option<String>,
) -> Result<SteeringFile, String> {
    let scope = scope.unwrap_or_else(|| "user".to_string());
    run_blocking_task(move || {
        SteeringManager::refine_file(&file_name, &scope, project_dir.as_deref())
    })
    .await
}

// ---------- 嵌套 AGENTS.md（Kiro 1.1.14）----------

/// 递归扫描项目内所有目录级 `AGENTS.md`。
/// 结果已按 Kiro 的顺序排序：项目根的最前，其后按层级、再按路径。
#[command]
pub async fn scan_agents_md(project_dir: String) -> Result<Vec<AgentsMdFile>, String> {
    run_blocking_task(move || SteeringManager::scan_agents_md(&project_dir)).await
}

/// 读取指定 `AGENTS.md`（rel_path 相对项目根，如 `src/api/AGENTS.md`）。
#[command]
pub async fn get_agents_md(project_dir: String, rel_path: String) -> Result<String, String> {
    run_blocking_task(move || SteeringManager::read_agents_md(&project_dir, &rel_path)).await
}

/// 写入指定 `AGENTS.md`，不存在则创建。
#[command]
pub async fn save_agents_md(
    project_dir: String,
    rel_path: String,
    content: String,
) -> Result<(), String> {
    run_blocking_task(move || SteeringManager::write_agents_md(&project_dir, &rel_path, &content))
        .await
}
