// Kiro IDE 权限命令：读写 permissions.yaml（全局 / 项目级）
//
// IDE 用 permissions.yaml 取代 0.x 的 Trusted Commands / Command Denylist。
// 详见同层 ../kiro/settings/permissions.rs 的说明。本项目不再往 kiroAgent.* 写
// trustedCommands 等键（迁移一次后即被 IDE 忽略），改为直接管理权限规则文件。
//
// 自 1.1.14 适配起支持两级作用域：
//   全局 → ~/.kiro/settings/permissions.yaml
//   项目 → ~/.kiro/workspace-roots/<workspace-id>/permissions.yaml
// workspace-id 由 `resolve_workspace_id` 推导（优先反查 .trust-migration.json
// 里记录的 root，查不到再按 sha256 算法现算），详见 permissions.rs。

use crate::kiro::settings::permissions::{
    known_capabilities, list_workspace_roots, read_permissions_scoped, write_permissions_scoped,
    PermissionPolicy, PermissionScope, WorkspaceRootInfo,
};

/// 读取权限策略。
///
/// - `scope`: `"global"`（默认）或 `"project"`
/// - `projectPath`: `scope == "project"` 时必填；缺失会退化为全局
#[tauri::command]
pub async fn get_permissions(
    scope: Option<String>,
    project_path: Option<String>,
) -> Result<PermissionPolicy, String> {
    let scope = PermissionScope::from_id(scope.as_deref(), project_path);
    tokio::task::spawn_blocking(move || read_permissions_scoped(&scope))
        .await
        .map_err(|e| format!("读取权限策略失败: {e}"))
}

/// 覆盖写入权限策略。前端负责把用户编辑后的完整规则列表回传，后端整体替换。
#[tauri::command]
pub async fn save_permissions(
    policy: PermissionPolicy,
    scope: Option<String>,
    project_path: Option<String>,
) -> Result<(), String> {
    let scope = PermissionScope::from_id(scope.as_deref(), project_path);
    tokio::task::spawn_blocking(move || write_permissions_scoped(&scope, &policy))
        .await
        .map_err(|e| format!("保存权限策略失败: {e}"))?
}

/// 返回已知的能力(capability)名列表，供前端下拉候选。
#[tauri::command]
pub async fn get_permission_capabilities() -> Result<Vec<String>, String> {
    Ok(known_capabilities())
}

/// 列出机器上所有已知 workspace-root，作为「项目级」作用域的候选。
///
/// 只包含 IDE 已经建过目录的项目（即 `~/.kiro/workspace-roots/*`）。
/// `projectPath` 为 `null` 表示该目录没有 `.trust-migration.json` 记录，
/// 此时前端应提示用户手动指定项目路径。
#[tauri::command]
pub async fn list_permission_workspace_roots() -> Result<Vec<WorkspaceRootInfo>, String> {
    tokio::task::spawn_blocking(list_workspace_roots)
        .await
        .map_err(|e| format!("列举 workspace-roots 失败: {e}"))
}
