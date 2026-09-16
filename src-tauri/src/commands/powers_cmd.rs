// Powers 管理命令

use crate::commands::common::run_blocking_task;
use crate::kiro::settings::powers::{
    PowerInfo, PowersManager, RecommendedPower, RegistryInfo, UserAddedPowerEntry,
    REGISTRY_ID_RECOMMENDED,
};
use tauri::command;

#[command]
pub async fn install_power(
    name: String,
    clone_url: String,
    path_in_repo: String,
    branch: String,
) -> Result<(), String> {
    // 面板的「推荐 Powers」安装走内置推荐源
    run_blocking_task(move || {
        PowersManager::install(
            &name,
            &clone_url,
            &path_in_repo,
            &branch,
            REGISTRY_ID_RECOMMENDED,
        )
    })
    .await
}

/// 从本地文件夹安装 Power（对应 Kiro「Import power from a folder」）。
/// 返回实际使用的 Power 名称（由目录名 sanitize 得出，可能与目录名不同）。
#[command]
pub async fn install_power_from_local(source_dir: String) -> Result<String, String> {
    run_blocking_task(move || PowersManager::install_from_local(&source_dir)).await
}

/// 从公开 GitHub URL 导入 Power（对应 Kiro「Import power from GitHub」）。
/// 返回实际使用的 Power 名称。
#[command]
pub async fn install_power_from_url(url: String) -> Result<String, String> {
    run_blocking_task(move || PowersManager::install_from_github_url(&url)).await
}

#[command]
pub async fn get_powers() -> Result<Vec<PowerInfo>, String> {
    run_blocking_task(PowersManager::load_all).await
}

#[command]
pub async fn get_power(name: String) -> Result<PowerInfo, String> {
    run_blocking_task(move || PowersManager::load(&name)).await
}

#[command]
pub async fn uninstall_power(name: String) -> Result<(), String> {
    run_blocking_task(move || PowersManager::uninstall(&name)).await
}

#[command]
pub async fn get_power_registries() -> Result<Vec<RegistryInfo>, String> {
    run_blocking_task(PowersManager::list_registries).await
}

/// 读取用户自建来源注册表（本地文件夹 / GitHub URL 导入的 Power 来源信息）。
/// 用于在面板中展示自定义 Power 的来源类型与路径。
#[command]
pub async fn get_user_added_powers() -> Result<Vec<UserAddedPowerEntry>, String> {
    run_blocking_task(|| {
        Ok(PowersManager::load_user_added_registry()?.powers)
    })
    .await
}

#[command]
pub async fn get_recommended_powers() -> Result<Vec<RecommendedPower>, String> {
    PowersManager::fetch_recommended().await
}
