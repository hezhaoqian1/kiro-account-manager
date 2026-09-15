// MCP 配置文件读写
//
// 字段定义以 Kiro 1.1.28 扩展内的 zod schema 为准：
//   oauth = { clientId?, redirectUri?, clientMetadataUrl? }
//   公共   = { env?, timeout?, waitForReady?, disabledTools?, autoApprove?, versionNegotiation? }
//   stdio = { command?, args?, cwd? }
//   http  = { url?, headers?, oauth?, oauthScopes? }
//   元信息 = { disabled?, type? }
// 一个服务器条目是上述所有字段的扁平并集，靠 command / url 区分传输方式。

use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, McpServer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub powers: Option<PowersMcpConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PowersMcpConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, PowerMcpServer>,
}

/// MCP 服务器条目：按传输方式分为 stdio（command）与 http（url）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServer {
    Command(McpServerCommand),
    Url(McpServerUrl),
}

/// MCP OAuth 客户端配置。
/// Kiro 1.0.395 起支持自带 OAuth client identity：把 clientMetadataUrl 指向托管的
/// client metadata 文档。这些字段若丢失，服务器需要重新走一次 OAuth 授权。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpOAuthConfig {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "clientId")]
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "redirectUri")]
    pub redirect_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "clientMetadataUrl")]
    pub client_metadata_url: Option<String>,
}

/// stdio 传输的 MCP 服务器（command / args / cwd）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerCommand {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default, rename = "autoApprove")]
    pub auto_approve: Vec<String>,
    #[serde(default, rename = "disabledTools", skip_serializing_if = "Vec::is_empty")]
    pub disabled_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(default, rename = "waitForReady", skip_serializing_if = "Option::is_none")]
    pub wait_for_ready: Option<bool>,
    /// 取值 "auto" | "legacy"
    #[serde(default, rename = "versionNegotiation", skip_serializing_if = "Option::is_none")]
    pub version_negotiation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub server_type: Option<String>,
}

/// http 传输的 MCP 服务器（url / headers / oauth / oauthScopes）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerUrl {
    pub url: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default, rename = "disabledTools")]
    pub disabled_tools: Vec<String>,
    #[serde(default, rename = "autoApprove", skip_serializing_if = "Vec::is_empty")]
    pub auto_approve: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<McpOAuthConfig>,
    #[serde(default, rename = "oauthScopes", skip_serializing_if = "Vec::is_empty")]
    pub oauth_scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    /// 取值 "auto" | "legacy"
    #[serde(default, rename = "versionNegotiation", skip_serializing_if = "Option::is_none")]
    pub version_negotiation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub server_type: Option<String>,
}

/// powers 段内的 MCP 服务器（同为 http 形态）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerMcpServer {
    pub url: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default, rename = "disabledTools")]
    pub disabled_tools: Vec<String>,
    #[serde(default, rename = "autoApprove", skip_serializing_if = "Vec::is_empty")]
    pub auto_approve: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<McpOAuthConfig>,
    #[serde(default, rename = "oauthScopes", skip_serializing_if = "Vec::is_empty")]
    pub oauth_scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub server_type: Option<String>,
}

impl McpConfig {
    /// 获取用户级 MCP 配置文件路径
    pub fn config_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".kiro").join("settings").join("mcp.json"))
    }

    /// 获取项目级 MCP 配置文件路径
    pub fn project_config_path(project_dir: &str) -> PathBuf {
        PathBuf::from(project_dir)
            .join(".kiro")
            .join("settings")
            .join("mcp.json")
    }

    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path).map_err(|e| format!("读取配置文件失败: {e}"))?;

        serde_json::from_str(&content).map_err(|e| format!("解析配置文件失败: {e}"))
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }

        let content =
            serde_json::to_string_pretty(self).map_err(|e| format!("序列化配置失败: {e}"))?;

        fs::write(path, content).map_err(|e| format!("写入配置文件失败: {e}"))
    }

    /// 读取用户级配置（写操作使用）
    pub fn load() -> Result<Self, String> {
        let path = Self::config_path().ok_or("无法获取用户目录")?;
        Self::load_from_path(&path)
    }

    /// 读取有效配置：user -> workspace -> powers（后者覆盖前者）
    pub fn load_merged(project_dir: Option<&str>) -> Result<Self, String> {
        let user_path = Self::config_path().ok_or("无法获取用户目录")?;
        let user_config = Self::load_from_path(&user_path)?;

        let workspace_config = if let Some(pd) = project_dir {
            let ws_path = Self::project_config_path(pd);
            Self::load_from_path(&ws_path)?
        } else {
            Self::default()
        };

        let mut merged_servers = user_config.mcp_servers.clone();
        for (name, server) in workspace_config.mcp_servers {
            merged_servers.insert(name, server);
        }

        if let Some(powers) = user_config.powers.as_ref() {
            for (name, server) in &powers.mcp_servers {
                merged_servers.insert(name.clone(), McpServer::Url(McpServerUrl::from(server.clone())));
            }
        }

        Ok(Self {
            mcp_servers: merged_servers,
            powers: user_config.powers,
        })
    }

    /// 保存用户级配置文件
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path().ok_or("无法获取用户目录")?;
        self.save_to_path(&path)
    }
}

impl From<PowerMcpServer> for McpServerUrl {
    fn from(value: PowerMcpServer) -> Self {
        Self {
            url: value.url,
            disabled: value.disabled,
            disabled_tools: value.disabled_tools,
            auto_approve: value.auto_approve,
            headers: value.headers,
            oauth: value.oauth,
            oauth_scopes: value.oauth_scopes,
            env: HashMap::new(),
            timeout: value.timeout,
            version_negotiation: None,
            server_type: value.server_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_server_keeps_oauth_and_headers_on_round_trip() {
        let json = r#"{
            "mcpServers": {
                "enterprise": {
                    "url": "https://example.com/mcp",
                    "headers": { "X-Tenant": "acme" },
                    "oauth": {
                        "clientId": "cid",
                        "redirectUri": "http://127.0.0.1/cb",
                        "clientMetadataUrl": "https://idp.example.com/metadata.json"
                    },
                    "oauthScopes": ["mcp:read"],
                    "versionNegotiation": "auto",
                    "timeout": 30000
                }
            }
        }"#;

        let config: McpConfig = serde_json::from_str(json).expect("config should parse");
        let out = serde_json::to_string(
            config.mcp_servers.get("enterprise").expect("server exists"),
        )
        .expect("serialize");

        assert!(out.contains("clientMetadataUrl"), "oauth 配置必须保留: {out}");
        assert!(out.contains("X-Tenant"), "headers 必须保留: {out}");
        assert!(out.contains("oauthScopes"), "oauthScopes 必须保留: {out}");
        assert!(out.contains("versionNegotiation"), "versionNegotiation 必须保留: {out}");
        assert!(out.contains("30000"), "timeout 必须保留: {out}");
    }

    #[test]
    fn stdio_server_keeps_cwd_and_wait_for_ready_on_round_trip() {
        let json = r#"{
            "mcpServers": {
                "local": {
                    "command": "node",
                    "args": ["server.js"],
                    "cwd": "/srv/mcp",
                    "waitForReady": true,
                    "timeout": 1500
                }
            }
        }"#;

        let config: McpConfig = serde_json::from_str(json).expect("config should parse");
        let out =
            serde_json::to_string(config.mcp_servers.get("local").expect("server exists")).expect("serialize");

        assert!(out.contains("/srv/mcp"), "cwd 必须保留: {out}");
        assert!(out.contains("waitForReady"), "waitForReady 必须保留: {out}");
        assert!(out.contains("\"command\":\"node\""), "已知字段照常序列化: {out}");
    }

    #[test]
    fn absent_optional_fields_are_not_written_back() {
        let json = r#"{"mcpServers":{"s":{"url":"https://x.dev"}}}"#;

        let config: McpConfig = serde_json::from_str(json).expect("config should parse");
        let out =
            serde_json::to_string(config.mcp_servers.get("s").expect("server exists")).expect("serialize");

        assert!(!out.contains("oauth"), "未配置的 oauth 不应写出: {out}");
        assert!(!out.contains("timeout"), "未配置的 timeout 不应写出: {out}");
    }
}
