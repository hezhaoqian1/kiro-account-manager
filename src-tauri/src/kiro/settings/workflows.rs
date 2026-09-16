// Workflows 管理（读取/编辑 <project>/.kiro/workflows/*.workflow.{json,yaml,yml}
// 以及 ~/.kiro/workflows/*）
//
// 逆向依据（Kiro 1.1.14，见 docs/Kiro 1.1.14/未覆盖功能与权限预设.md）：
// - 文件：`.workflow.json` / `.workflow.yaml` / `.workflow.yml`（产物中分别 5/6 次）
// - 四个合法根目录（优先级）：工作区 `.kiro/workflows/` > `~/.kiro/workflows/` >
//   `~/.kiro/cloud-cache/<scope>/config/workflows/`（云端只读）> bundled（`bundled://<name>`）
// - 另有 `generated://<id>`（workflow-creator 生成，一次性，运行即消费）
// - Schema：name + inputs（模板变量）+ steps[] 节点；可设 workflow 级默认 modelId/effortLevel
// - catalog 总长上限 12e3 字符
//
// 本管理端只覆盖「用户级 + 项目级」两类可写 workflow 文件（与 Steering/Specs 同级），
// bundled/generated/cloud 属运行时/只读来源，不在此编辑。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowFile {
    pub file_name: String,
    pub content: String,
    pub size: u64,
    pub modified_at: Option<String>,
    /// "user" 或 "project"
    pub scope: String,
}

pub struct WorkflowManager;

/// 合法的工作流文件扩展名。
const WORKFLOW_EXTS: &[&str] = &["workflow.json", "workflow.yaml", "workflow.yml"];

impl WorkflowManager {
    /// 用户级 workflows 目录：`~/.kiro/workflows`
    fn user_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".kiro").join("workflows"))
    }

    /// 项目级 workflows 目录：`<project>/.kiro/workflows`
    fn project_dir(project_dir: &str) -> PathBuf {
        PathBuf::from(project_dir).join(".kiro").join("workflows")
    }

    fn resolve_root(scope: &str, project_dir: Option<&str>) -> Result<PathBuf, String> {
        match scope {
            "project" => {
                let pd = project_dir.ok_or("项目级操作需要提供项目目录")?;
                Ok(Self::project_dir(pd))
            }
            _ => Self::user_dir().ok_or_else(|| "无法获取用户目录".to_string()),
        }
    }

    /// 校验文件名：必须以某个合法扩展名结尾，且整体是单个普通路径组件（防穿越）。
    fn sanitize_file_name(file_name: &str) -> Result<String, String> {
        let trimmed = file_name.trim();
        if trimmed.is_empty() {
            return Err("文件名不能为空".to_string());
        }
        if !WORKFLOW_EXTS.iter().any(|e| trimmed.ends_with(e)) {
            return Err("工作流文件必须以 .workflow.json / .workflow.yaml / .workflow.yml 结尾".to_string());
        }
        let path = Path::new(trimmed);
        let mut components = path.components();
        let only_normal =
            matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
        if !only_normal {
            return Err("文件名不合法".to_string());
        }
        Ok(trimmed.to_string())
    }

    /// 列出某一级别下的所有 workflow 文件。
    pub fn list_workflows(scope: &str, project_dir: Option<&str>) -> Result<Vec<WorkflowFile>, String> {
        let root = Self::resolve_root(scope, project_dir)?;
        if !root.exists() {
            return Ok(vec![]);
        }
        let mut files = vec![];
        for entry in fs::read_dir(&root).map_err(|e| format!("读取 workflows 目录失败: {e}"))? {
            let entry = entry.map_err(|e| format!("读取条目失败: {e}"))?;
            let path = entry.path();
            if path.is_file() {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !WORKFLOW_EXTS.iter().any(|e| name.ends_with(e)) {
                    continue;
                }
                let metadata = fs::metadata(&path).ok();
                let size = metadata.as_ref().map_or(0, std::fs::Metadata::len);
                let modified_at = metadata.and_then(|m| m.modified().ok()).map(|t| {
                    let datetime: chrono::DateTime<chrono::Local> = t.into();
                    datetime.format("%Y/%m/%d %H:%M:%S").to_string()
                });
                let content = fs::read_to_string(&path).unwrap_or_default();
                files.push(WorkflowFile {
                    file_name: name,
                    content,
                    size,
                    modified_at,
                    scope: scope.to_string(),
                });
            }
        }
        files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        Ok(files)
    }

    /// 读取单个 workflow 文件（不存在则报错，便于前端区分「未选中」与「文件缺失」）。
    pub fn read_workflow(
        scope: &str,
        project_dir: Option<&str>,
        file_name: &str,
    ) -> Result<WorkflowFile, String> {
        let file_name = Self::sanitize_file_name(file_name)?;
        let root = Self::resolve_root(scope, project_dir)?;
        let path = root.join(&file_name);
        if !path.exists() {
            return Err(format!("工作流文件不存在: {file_name}"));
        }
        let metadata = fs::metadata(&path).map_err(|e| format!("读取元数据失败: {e}"))?;
        let size = std::fs::Metadata::len(&metadata);
        let modified_at = metadata.modified().ok().map(|t| {
            let datetime: chrono::DateTime<chrono::Local> = t.into();
            datetime.format("%Y/%m/%d %H:%M:%S").to_string()
        });
        let content = fs::read_to_string(&path).map_err(|e| format!("读取 {file_name} 失败: {e}"))?;
        Ok(WorkflowFile {
            file_name,
            content,
            size,
            modified_at,
            scope: scope.to_string(),
        })
    }

    /// 写入单个 workflow 文件（不存在则创建，父目录自动创建）。
    pub fn write_workflow(
        scope: &str,
        project_dir: Option<&str>,
        file_name: &str,
        content: &str,
    ) -> Result<(), String> {
        let file_name = Self::sanitize_file_name(file_name)?;
        let root = Self::resolve_root(scope, project_dir)?;
        fs::create_dir_all(&root).map_err(|e| format!("创建 workflows 目录失败: {e}"))?;
        let path = root.join(&file_name);
        fs::write(&path, content).map_err(|e| format!("写入 {file_name} 失败: {e}"))
    }

    /// 删除单个 workflow 文件。
    pub fn delete_workflow(
        scope: &str,
        project_dir: Option<&str>,
        file_name: &str,
    ) -> Result<(), String> {
        let file_name = Self::sanitize_file_name(file_name)?;
        let root = Self::resolve_root(scope, project_dir)?;
        let path = root.join(&file_name);
        if !path.exists() {
            return Ok(());
        }
        fs::remove_file(&path).map_err(|e| format!("删除 {file_name} 失败: {e}"))
    }
}
