// 系统机器码管理模块 - 支持 Windows/macOS/Linux

#![allow(clippy::needless_pass_by_value)] // Tauri 命令需要按值传递参数

mod types;
mod utils;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub use types::SystemMachineInfo;
pub use utils::generate_random_machine_id;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
mod platform {
    use super::types::SystemMachineInfo;
    const ERR: &str = "此功能仅支持 Windows、macOS 和 Linux 系统";
    pub fn get_system_machine_guid_inner() -> Result<SystemMachineInfo, String> {
        Err(ERR.into())
    }
    pub fn reset_machine_guid_inner() -> Result<String, String> {
        Err(ERR.into())
    }
    pub fn set_custom_machine_guid_inner(_: String) -> Result<String, String> {
        Err(ERR.into())
    }
    pub fn clear_override_inner() -> Result<(), String> {
        Ok(())
    }
}

async fn run<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("Task failed: {e}"))
}

#[tauri::command]
pub async fn get_system_machine_guid() -> Result<SystemMachineInfo, String> {
    run(platform::get_system_machine_guid_inner).await?
}

#[tauri::command]
pub async fn reset_system_machine_guid() -> Result<String, String> {
    run(platform::reset_machine_guid_inner).await?
}

#[tauri::command]
pub async fn set_custom_machine_guid(new_guid: String) -> Result<String, String> {
    run(move || platform::set_custom_machine_guid_inner(new_guid)).await?
}

#[tauri::command]
pub async fn clear_macos_override() -> Result<(), String> {
    run(platform::clear_override_inner).await?
}

#[tauri::command]
pub fn generate_machine_guid() -> String {
    generate_random_machine_id()
}

/// 以管理员权限重启应用（仅 Windows）
#[tauri::command]
#[allow(unused_variables)]
pub async fn restart_as_admin(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let exe_path = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;

        // 使用 PowerShell 的 Start-Process -Verb RunAs 以管理员权限启动。
        // M13:旧实现 spawn 后硬睡 500ms 就无条件 app.exit(0)——若用户在 UAC 弹窗点了
        // "否"(或提权延迟),提权进程从未起来,旧进程却已退出,应用直接消失(假死)。
        // 改用阻塞 output():Start-Process(不带 -Wait)会在提权进程一启动就返回,UAC 被
        // 拒绝时会抛出终止错误使 PowerShell 非零退出。据此确认提权成功后再退旧进程,
        // 失败则原样返回错误、不退出,让用户可重试。
        // 把当前真实数据目录一并传给提权实例。
        // 背景：以「其他管理员账号」的凭据提权时，进程的 %APPDATA% 会变成那个管理员的，
        // 应用会读不到原用户的 accounts.json（表现为账号全部消失）。沿用原目录即可避免。
        // 单引号内的双引号是字面量，PowerShell 会把 --data-dir="..." 原样传给目标进程。
        let quote = '"';
        let data_dir = crate::core::paths::app_data_dir_or_default();
        let data_dir_arg = format!("--data-dir={quote}{}{quote}", data_dir.display());

        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "$ErrorActionPreference='Stop'; Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs",
                    exe_path.display().to_string().replace('\'', "''"),
                    data_dir_arg.replace('\'', "''")
                ),
            ])
            .output()
            .map_err(|e| format!("启动管理员进程失败: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "以管理员权限启动失败(可能是取消了 UAC 提权): {}",
                stderr.trim()
            ));
        }

        // 提权进程已确认启动,再退出当前进程。
        app.exit(0);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        let exe_path = std::env::current_exe().map_err(|e| format!("获取程序路径失败: {e}"))?;

        // 尝试使用 pkexec。pkexec 会作为提权进程的父进程一直存活(不能用阻塞 output()
        // 等它,那会挂到 app 退出),所以 spawn 后短暂 try_wait 探测:若 polkit 授权被取消,
        // pkexec 会很快以非零码退出——此时不退出旧进程,返回错误让用户重试(M13)。
        // 与 Windows 同理：root 下 HOME / XDG_DATA_HOME 会指向 root，显式传原数据目录。
        let data_dir = crate::core::paths::app_data_dir_or_default();
        let mut child = match Command::new("pkexec")
            .arg(&exe_path)
            .arg(format!("--data-dir={}", data_dir.display()))
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                return Err("请使用 sudo 或 pkexec 手动以 root 权限运行程序".to_string());
            }
        };

        std::thread::sleep(std::time::Duration::from_millis(500));
        match child.try_wait() {
            // 已退出且非成功 = 授权被取消 / 提权失败,别退当前进程
            Ok(Some(status)) if !status.success() => {
                Err("以管理员权限启动失败(可能是取消了授权),请重试或用 sudo 手动运行".to_string())
            }
            // 仍在运行(提权进程正常拉起,pkexec 作为父进程存活)= 成功,退出旧进程
            Ok(None) | Ok(Some(_)) => {
                app.exit(0);
                Ok(())
            }
            // try_wait 本身失败时不要误当成提权成功
            Err(e) => Err(format!("检测提权进程状态失败: {e}")),
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS 不需要管理员权限（写入用户目录）
        Err("macOS 不需要管理员权限".to_string())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("不支持的操作系统".to_string())
    }
}
