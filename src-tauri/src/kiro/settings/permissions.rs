// Kiro IDE 1.0 权限模型 (permissions.yaml) 读写
//
// 背景：Kiro IDE 1.0 把 0.x 的 Trusted Commands / Command Denylist 统一替换为
// 基于「能力(capability)」的权限规则文件。旧的 kiroAgent.trustedCommands 等键只在
// 首次启动时由 IDE 一次性迁移进 permissions.yaml，之后即被忽略；permissions.yaml
// 才是 1.0 下权限裁决的唯一真相源。本项目据此适配，直接读写该文件。
//
// 参考实现从本地安装的 IDE 构建产物 (extension.js) 抠出，规则 schema 如下：
//   rules:
//     - capability: shell            # read|write|shell|web|web_fetch|web_search|subagent|spec|context|mcp|@mcp|@powers|@builtin|@subagent|@subagent-explicit
//       effect: allow                # allow | deny | ask
//       match: ["git *"]             # 可选：匹配模式（命令 / 工具名）
//       exclude: ["git push *"]      # 可选：排除模式
//   policies:                        # 可选：引用预设策略 id
//     - "some-preset"
// 文件位置（与 IDE 的 MOi 解析逻辑一致）：
//   全局： ~/.kiro/settings/permissions.yaml  (回退 permissions.json)
//   项目： ~/.kiro/workspace-roots/<workspace-id>/permissions.yaml
// 本项目当前只适配全局作用域（与旧 Trusted Commands 的全局性质一致）；项目级需要
// 复刻 IDE 的 workspace-id (sha256 截断) 哈希，留作后续工作。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 单条权限规则。
///
/// 注意：`match` 是 Rust 关键字，故字段命名为 `match_patterns` 并以
/// `#[serde(rename = "match")]` 映射到 YAML/JSON 中的 `match` 键。
/// capability / effect 一律用 `String` 而非枚举——与 hooks 的 trigger 同理：
/// IDE 会自行校验未知值（未知 capability 仅产生 warning 并跳过该规则，
/// 不会整体解析失败），用枚举反而会在 IDE 新增能力时导致本项目解析报错。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PermissionRule {
    pub capability: String,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub match_patterns: Option<Vec<String>>,
    pub effect: String,
    #[serde(rename = "exclude", skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
}

/// 完整权限策略（对应 permissions.yaml / permissions.json 的顶层结构）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PermissionPolicy {
    #[serde(default)]
    pub rules: Vec<PermissionRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policies: Option<Vec<String>>,
}

/// IDE 1.0 已知的能力列表（用于前端下拉候选；同时允许自由输入未知值）。
/// 来自 extension.js 的 capability 常量 + 实测 permissions.yaml 中出现的具体能力名。
pub const KNOWN_CAPABILITIES: &[&str] = &[
    "shell",
    "read",
    "write",
    "web",
    "web_fetch",
    "web_search",
    "subagent",
    "spec",
    "context",
    "mcp",
    "@mcp",
    "@powers",
    "@builtin",
    "@subagent",
    "@subagent-explicit",
];

/// 全局权限文件所在目录：`~/.kiro/settings`
fn permissions_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".kiro").join("settings"))
}

/// 解析权限文件路径，复刻 IDE 的 MOi 逻辑：
/// 优先用有内容的 permissions.yaml；否则用有内容的 permissions.json；
/// 两者皆空/不存在时返回 permissions.yaml（用于新建）。
fn permissions_file_path() -> Option<PathBuf> {
    let dir = permissions_dir()?;
    let yaml = dir.join("permissions.yaml");
    let json = dir.join("permissions.json");

    if file_has_content(&yaml) {
        return Some(yaml);
    }
    if file_has_content(&json) {
        return Some(json);
    }
    Some(yaml)
}

fn file_has_content(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|c| !c.trim().is_empty())
        .unwrap_or(false)
}

/// 读取全局权限策略。文件不存在 / 解析失败均安全回退为空策略。
///
/// 若全局 `permissions.yaml` 尚不存在，会尝试一次性迁移 0.x 的 `kiroAgent.*` 旧键
/// （见 `migrate_legacy_kiro_agent_permissions`）。已经存在权限文件时绝不触发迁移，
/// 以磁盘上的策略为准，避免覆盖用户已编辑的配置。
pub fn read_permissions() -> PermissionPolicy {
    let Some(path) = permissions_file_path() else {
        return PermissionPolicy::default();
    };
    if !path.exists() {
        return migrate_legacy_kiro_agent_permissions();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return PermissionPolicy::default(),
    };
    if content.trim().is_empty() {
        return PermissionPolicy::default();
    }

    if path.extension().and_then(|e| e.to_str()) == Some("json") {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        serde_yaml::from_str(&content).unwrap_or_default()
    }
}

/// 一次性迁移 0.x 的 `kiroAgent.*` 旧键到 1.0 的 `permissions.yaml` 规则。
///
/// 仅作为兜底：Kiro IDE 在 1.0 首次启动时会自己做同样的迁移（并写入
/// `~/.kiro/.trust-migration.json`）。本项目在用户尚未启动过 IDE 1.0、却已用本应用
/// 配置过旧版 Trusted Commands 的情况下，保证这些配置不丢。迁移是幂等的——
/// 只要 `permissions.yaml` 已存在就直接返回默认空策略，绝不覆盖。
fn migrate_legacy_kiro_agent_permissions() -> PermissionPolicy {
    let Some(path) = legacy_kiro_user_settings_path() else {
        return PermissionPolicy::default();
    };
    if !path.exists() {
        return PermissionPolicy::default();
    }
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return PermissionPolicy::default(),
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return PermissionPolicy::default(),
    };
    let Some(kiro) = json.get("kiroAgent").and_then(|v| v.as_object()) else {
        return PermissionPolicy::default();
    };

    let rules = build_rules_from_legacy_kiro_agent(kiro);
    if rules.is_empty() {
        return PermissionPolicy::default();
    }

    let policy = PermissionPolicy {
        rules,
        policies: None,
    };

    // 仅当目标文件确实不存在时才写入（避免与 IDE 的迁移竞态覆盖用户配置）。
    if let Some(dir) = permissions_dir() {
        let target = dir.join("permissions.yaml");
        if !target.exists() {
            let _ = write_permissions(&policy);
        }
    }
    policy
}

/// 把旧版 `kiroAgent` 配置对象翻译成 1.0 的权限规则。纯函数，便于单测。
///
/// 映射规则（对齐 IDE 1.0 的迁移语义）：
/// - `trustedCommands == "all"` 或 `autoApproveAgentCommands == true` → `shell allow`（match: "*"）
/// - `commandDenylist: string[]` → `shell deny`（match: 各模式）
/// - `trustedTools: string[]` → 每个工具一条 `<tool> allow`
/// `trustedCommands` 为 `"none"`/`"common"` 时不产生 allow 规则（common 的固定命令集
/// 由 IDE 内部维护，本项目无法精确还原，交由用户在面板里手动配置）。
fn build_rules_from_legacy_kiro_agent(kiro: &serde_json::Map<String, serde_json::Value>) -> Vec<PermissionRule> {
    let mut rules: Vec<PermissionRule> = Vec::new();

    let trusted_commands = kiro
        .get("trustedCommands")
        .and_then(|v| v.as_str());
    let auto_approve = kiro
        .get("autoApproveAgentCommands")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if trusted_commands == Some("all") || auto_approve {
        rules.push(PermissionRule {
            capability: "shell".to_string(),
            match_patterns: Some(vec!["*".to_string()]),
            effect: "allow".to_string(),
            exclude: None,
        });
    }

    if let Some(denylist) = kiro.get("commandDenylist").and_then(|v| v.as_array()) {
        let patterns: Vec<String> = denylist
            .iter()
            .filter_map(|i| i.as_str().map(String::from))
            .collect();
        if !patterns.is_empty() {
            rules.push(PermissionRule {
                capability: "shell".to_string(),
                match_patterns: Some(patterns),
                effect: "deny".to_string(),
                exclude: None,
            });
        }
    }

    if let Some(tools) = kiro.get("trustedTools").and_then(|v| v.as_array()) {
        for tool in tools.iter().filter_map(|i| i.as_str()) {
            rules.push(PermissionRule {
                capability: tool.to_string(),
                match_patterns: None,
                effect: "allow".to_string(),
                exclude: None,
            });
        }
    }

    rules
}

/// 旧版 Kiro IDE 用户设置路径（与 `commands::kiro_settings_cmd::get_kiro_settings_path`
/// 保持一致的副本——`permissions` 模块不能反向依赖 `commands` 以避免循环引用，故此处
/// 复制一份。若修改此处，请同步修改 kiro_settings_cmd 中的同名函数）。
/// 0.x 的 `kiroAgent.*` 旧键就存放在该 `settings.json` 中。
fn legacy_kiro_user_settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA").ok().map(|appdata| {
            PathBuf::from(appdata)
                .join("Kiro")
                .join("User")
                .join("settings.json")
        })
    }
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|home| {
            home.join("Library")
                .join("Application Support")
                .join("Kiro")
                .join("User")
                .join("settings.json")
        })
    }
    #[cfg(target_os = "linux")]
    {
        dirs::home_dir().map(|home| {
            home.join(".config")
                .join("Kiro")
                .join("User")
                .join("settings.json")
        })
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// 写入全局权限策略（始终写 permissions.yaml，与 IDE 的 canonical 形式一致）。
pub fn write_permissions(policy: &PermissionPolicy) -> Result<(), String> {
    let dir = permissions_dir().ok_or("无法定位 ~/.kiro/settings 目录（HOME 未设置？）")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 ~/.kiro/settings 目录失败: {e}"))?;

    let path = dir.join("permissions.yaml");
    // 没有规则时序列化为 `rules: []`，保证文件可被 IDE 正确解析。
    let yaml =
        serde_yaml::to_string(policy).map_err(|e| format!("序列化 permissions 失败: {e}"))?;
    std::fs::write(&path, yaml).map_err(|e| format!("写入 permissions.yaml 失败: {e}"))?;
    Ok(())
}

/// 返回 IDE 1.0 已知的能力名列表，供前端构建下拉候选。
pub fn known_capabilities() -> Vec<String> {
    KNOWN_CAPABILITIES.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kiro(map: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        map.as_object().unwrap().clone()
    }

    #[test]
    fn legacy_all_trusted_commands_maps_to_shell_allow() {
        let kiro = kiro(serde_json::json!({
            "trustedCommands": "all",
        }));
        let rules = build_rules_from_legacy_kiro_agent(&kiro);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].capability, "shell");
        assert_eq!(rules[0].effect, "allow");
        assert_eq!(rules[0].match_patterns.as_deref(), Some(&["*".to_string()][..]));
    }

    #[test]
    fn legacy_auto_approve_maps_to_shell_allow() {
        let kiro = kiro(serde_json::json!({
            "autoApproveAgentCommands": true,
        }));
        let rules = build_rules_from_legacy_kiro_agent(&kiro);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].capability, "shell");
        assert_eq!(rules[0].effect, "allow");
    }

    #[test]
    fn legacy_none_and_common_produce_no_allow_rule() {
        // "none" 与 "common" 不应产生 shell allow 规则（common 固定命令集不还原）。
        let none = kiro(serde_json::json!({ "trustedCommands": "none" }));
        assert!(build_rules_from_legacy_kiro_agent(&none).is_empty());

        let common = kiro(serde_json::json!({ "trustedCommands": "common" }));
        assert!(build_rules_from_legacy_kiro_agent(&common).is_empty());
    }

    #[test]
    fn legacy_command_denylist_maps_to_shell_deny() {
        let kiro = kiro(serde_json::json!({
            "commandDenylist": ["rm -rf *", "git push --force"],
        }));
        let rules = build_rules_from_legacy_kiro_agent(&kiro);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].capability, "shell");
        assert_eq!(rules[0].effect, "deny");
        assert_eq!(
            rules[0].match_patterns.as_deref(),
            Some(&["rm -rf *".to_string(), "git push --force".to_string()][..])
        );
    }

    #[test]
    fn legacy_trusted_tools_map_to_capability_allow() {
        let kiro = kiro(serde_json::json!({
            "trustedTools": ["webFetch", "remote_web_search"],
        }));
        let rules = build_rules_from_legacy_kiro_agent(&kiro);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].capability, "webFetch");
        assert_eq!(rules[0].effect, "allow");
        assert_eq!(rules[1].capability, "remote_web_search");
        assert_eq!(rules[1].effect, "allow");
    }

    #[test]
    fn legacy_combined_produces_all_expected_rules() {
        let kiro = kiro(serde_json::json!({
            "trustedCommands": "all",
            "commandDenylist": ["rm *"],
            "trustedTools": ["webFetch"],
        }));
        let rules = build_rules_from_legacy_kiro_agent(&kiro);
        assert_eq!(rules.len(), 3);
        assert!(rules.iter().any(|r| r.capability == "shell" && r.effect == "allow"));
        assert!(rules.iter().any(|r| r.capability == "shell" && r.effect == "deny"));
        assert!(rules.iter().any(|r| r.capability == "webFetch" && r.effect == "allow"));
    }

    #[test]
    fn known_capabilities_contains_core_and_specific() {
        let caps = known_capabilities();
        assert!(caps.contains(&"shell".to_string()));
        assert!(caps.contains(&"web_fetch".to_string()));
        assert!(caps.contains(&"mcp".to_string()));
        assert!(caps.contains(&"@subagent".to_string()));
    }
}
