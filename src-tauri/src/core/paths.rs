//! 应用数据目录的统一入口。
//!
//! 真实数据位置（由用户确认保持不变）：`%APPDATA%\.kiro-account-manager`
//! （即 `AppData\Roaming\.kiro-account-manager`，点前缀旧约定）。
//!
//! 重要：`AppData\Local\com.kiro.account-manager\EBWebView` 只是 WebView2 控件自身的
//! 缓存目录，**不是**应用数据。应用数据必须走 Roaming 下的点前缀目录，改到 Local 或
//! 改成无点前缀名都会找不到存量账号数据。
//!
//! | 平台 | 解析结果 |
//! |------|---------|
//! | Windows | `%APPDATA%\<DATA_DIR_NAME>` |
//! | macOS   | `~/Library/Application Support/<DATA_DIR_NAME>` |
//! | Linux   | `$XDG_DATA_HOME` 或 `~/.local/share/<DATA_DIR_NAME>` |

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 应用数据目录名。真实数据位于 `%APPDATA%\.kiro-account-manager`，**勿改**。
pub const DATA_DIR_NAME: &str = ".kiro-account-manager";

/// 历史目录名，与当前 `DATA_DIR_NAME` 相同（用户决定不再迁移）。
/// 保留供 `main.rs` 日志与 `migrate_legacy_data` 兼容。
pub const LEGACY_DIR_NAME: &str = DATA_DIR_NAME;

/// bundle identifier 兜底值，必须与 `tauri.conf.json` 的 `identifier` 保持一致。
/// 正常路径下会由 `main()` 调用 [`set_identifier`] 注入真实值覆盖。
const FALLBACK_IDENTIFIER: &str = "com.kiro.account-manager";

static IDENTIFIER: OnceLock<String> = OnceLock::new();

/// 由 `main()` 在构造 Tauri Context 之后注入真正的 bundle identifier。
/// 仅用于日志展示，不再参与数据目录拼接。
pub fn set_identifier(id: impl Into<String>) {
    let _ = IDENTIFIER.set(id.into());
}

/// 当前使用的 bundle identifier（仅日志用）。
pub fn identifier() -> &'static str {
    IDENTIFIER
        .get()
        .map(|s| s.as_str())
        .unwrap_or(FALLBACK_IDENTIFIER)
}

/// 数据目录覆盖值，由 `--data-dir=` 命令行参数注入。
///
/// 背景（提权导致数据目录漂移）：以管理员身份重启时，若用户在 UAC 输入的是**其他管理员
/// 账号**的凭据，进程会以那个账号运行，`%APPDATA%` 也随之变成**该管理员**的目录。
/// 结果应用读不到原用户的 `accounts.json`，表现为「账号全部消失」，用户极易误判为数据丢失。
///
/// 处理：`restart_as_admin` 在提权前把当前真实数据目录通过 `--data-dir=` 传给提权实例，
/// 提权实例沿用该路径，不再依赖 `%APPDATA%` 重新解析。未传该参数时行为与以前完全一致。
static DATA_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// 注入数据目录覆盖值。**仅**应在 `main()` 解析到 `--data-dir=` 时调用（进程内一次）。
pub fn set_data_dir_override(dir: PathBuf) {
    let _ = DATA_DIR_OVERRIDE.set(dir);
}

/// 官方应用数据目录。系统目录不可用时返回 `None`。
/// 已通过 `--data-dir=` 注入覆盖值时（提权场景）优先返回覆盖值。
pub fn app_data_dir() -> Option<PathBuf> {
    if let Some(dir) = DATA_DIR_OVERRIDE.get() {
        return Some(dir.clone());
    }
    if let Ok(raw) = std::env::var("KIRO_DATA_DIR") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    dirs::data_dir().map(|dir| dir.join(DATA_DIR_NAME))
}

/// 宽松版：系统目录不可用时回落到用户主目录。
///
/// 用于原本 `dirs::data_dir().unwrap_or_default()` 的调用点——那个写法会退化成
/// **相对路径**，把数据写进当前工作目录。这里顺手修正为落到 `$HOME`。
pub fn app_data_dir_or_default() -> PathBuf {
    if let Some(dir) = DATA_DIR_OVERRIDE.get() {
        return dir.clone();
    }
    if let Ok(raw) = std::env::var("KIRO_DATA_DIR") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    match dirs::data_dir() {
        Some(dir) => dir.join(DATA_DIR_NAME),
        None => home_dir().join(DATA_DIR_NAME),
    }
}

fn home_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
}

/// 历史数据目录，**仅**迁移逻辑使用。当前与主目录相同。
pub fn legacy_app_data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|dir| dir.join(LEGACY_DIR_NAME))
}

/// 一次性迁移：旧目录 → 新目录。
///
/// 因用户决定保持点前缀旧约定，当前 `legacy_app_data_dir() == app_data_dir()`，
/// 本函数恒为 no-op（直接返回 `Ok(false)`），不会移动或复制任何数据。
pub fn migrate_legacy_data() -> std::io::Result<bool> {
    let (Some(old_dir), Some(new_dir)) = (legacy_app_data_dir(), app_data_dir()) else {
        return Ok(false);
    };

    if old_dir == new_dir || !old_dir.is_dir() || new_dir.exists() {
        return Ok(false);
    }

    std::fs::create_dir_all(&new_dir)?;
    copy_dir_recursive(&old_dir, &new_dir)?;
    Ok(true)
}

/// 递归复制目录内容（不删除源点，保留回滚能力）。
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_dir_resolves_to_roaming_dot_prefix() {
        let expected = dirs::data_dir().unwrap().join(DATA_DIR_NAME);
        assert_eq!(app_data_dir().unwrap(), expected);
    }

    #[test]
    fn app_data_dir_matches_legacy_location() {
        // 用户决定不迁移，主目录即历史目录（Roaming\.kiro-account-manager）。
        assert_eq!(legacy_app_data_dir().as_deref(), app_data_dir().as_deref());
    }
}
