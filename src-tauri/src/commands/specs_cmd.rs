// Specs 管理命令（Kiro 1.1.14）

use crate::commands::common::run_blocking_task;
use crate::kiro::settings::specs::{SpecInfo, SpecManager, SpecSummary};

/// 列出某一级别下的所有 spec。
#[command]
pub async fn list_specs(
    scope: String,
    project_dir: Option<String>,
) -> Result<Vec<SpecSummary>, String> {
    run_blocking_task(move || SpecManager::list_specs(&scope, project_dir.as_deref())).await
}

/// 读取一个 spec 的三个文件（requirements / design / tasks）。
#[command]
pub async fn read_spec(
    scope: String,
    project_dir: Option<String>,
    name: String,
) -> Result<SpecInfo, String> {
    run_blocking_task(move || SpecManager::read_spec(&scope, project_dir.as_deref(), &name)).await
}

/// 写入一个 spec 的某个文件。
#[command]
pub async fn save_spec_file(
    scope: String,
    project_dir: Option<String>,
    name: String,
    file_kind: String,
    content: String,
) -> Result<(), String> {
    run_blocking_task(move || {
        SpecManager::write_spec_file(&scope, project_dir.as_deref(), &name, &file_kind, &content)
    })
    .await
}

/// 新建一个空 spec 目录。
#[command]
pub async fn create_spec(
    scope: String,
    project_dir: Option<String>,
    name: String,
) -> Result<(), String> {
    run_blocking_task(move || SpecManager::create_spec(&scope, project_dir.as_deref(), &name)).await
}

/// 删除整个 spec 目录。
#[command]
pub async fn delete_spec(
    scope: String,
    project_dir: Option<String>,
    name: String,
) -> Result<(), String> {
    run_blocking_task(move || SpecManager::delete_spec(&scope, project_dir.as_deref(), &name)).await
}
