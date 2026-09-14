// Kiro IDE 1.0 权限命令：读写全局 permissions.yaml
//
// IDE 1.0 用 permissions.yaml 取代 0.x 的 Trusted Commands / Command Denylist。
// 详见同层 ../kiro/settings/permissions.rs 的说明。本项目不再往 kiroAgent.* 写
// trustedCommands 等键（迁移一次后即被 IDE 忽略），改为直接管理权限规则文件。

use crate::kiro::settings::permissions::{
    known_capabilities, read_permissions, write_permissions, PermissionPolicy,
};

/// 读取全局权限策略（~/.kiro/settings/permissions.yaml）。
#[tauri::command]
pub async fn get_permissions() -> Result<PermissionPolicy, String> {
    tokio::task::spawn_blocking(read_permissions)
        .await
        .map_err(|e| format!("读取权限策略失败: {e}"))
}

/// 覆盖写入全局权限策略。前端负责把用户编辑后的完整规则列表回传，后端整体替换。
#[tauri::command]
pub async fn save_permissions(policy: PermissionPolicy) -> Result<(), String> {
    tokio::task::spawn_blocking(move || write_permissions(&policy))
        .await
        .map_err(|e| format!("保存权限策略失败: {e}"))?
}

/// 返回 IDE 1.0 已知的能力(capability)名列表，供前端下拉候选。
#[tauri::command]
pub async fn get_permission_capabilities() -> Result<Vec<String>, String> {
    Ok(known_capabilities())
}
