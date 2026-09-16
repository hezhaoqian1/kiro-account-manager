// Steering 管理（读取/编辑 ~/.kiro/steering/*.md 和 <project>/.kiro/steering/*.md）

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteeringFile {
    pub file_name: String,
    pub content: String,
    pub size: u64,
    pub modified_at: Option<String>,
    /// "user" 或 "project"
    pub scope: String,
}

pub struct SteeringManager;

impl SteeringManager {
    /// 获取用户级 steering 目录
    pub fn user_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".kiro").join("steering"))
    }

    /// 获取项目级 steering 目录
    pub fn project_dir(project_dir: &str) -> PathBuf {
        PathBuf::from(project_dir).join(".kiro").join("steering")
    }

    /// 从指定目录读取所有 steering 文件
    fn load_from_dir(dir: &PathBuf, scope: &str) -> Result<Vec<SteeringFile>, String> {
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut files = vec![];

        for entry in fs::read_dir(dir).map_err(|e| format!("读取目录失败: {e}"))? {
            let entry = entry.map_err(|e| format!("读取条目失败: {e}"))?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "md") {
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let metadata = fs::metadata(&path).ok();
                let size = metadata.as_ref().map_or(0, std::fs::Metadata::len);
                let modified_at = metadata.and_then(|m| m.modified().ok()).map(|t| {
                    let datetime: chrono::DateTime<chrono::Local> = t.into();
                    datetime.format("%Y/%m/%d %H:%M:%S").to_string()
                });

                let content = fs::read_to_string(&path).unwrap_or_default();

                files.push(SteeringFile {
                    file_name,
                    content,
                    size,
                    modified_at,
                    scope: scope.to_string(),
                });
            }
        }

        Ok(files)
    }

    /// 根据 scope 获取目标目录
    fn resolve_dir(scope: &str, project_dir: Option<&str>) -> Result<PathBuf, String> {
        match scope {
            "project" => {
                let pd = project_dir.ok_or("项目级操作需要提供项目目录")?;
                Ok(Self::project_dir(pd))
            }
            _ => Self::user_dir().ok_or_else(|| "无法获取用户目录".to_string()),
        }
    }

    fn sanitize_file_name(file_name: &str) -> Result<&str, String> {
        if file_name.trim().is_empty() {
            return Err("文件名不能为空".to_string());
        }
        if !file_name.ends_with(".md") {
            return Err("Steering 文件必须以 .md 结尾".to_string());
        }

        let path = Path::new(file_name);
        let mut components = path.components();
        let only_normal =
            matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
        if !only_normal {
            return Err("文件名不合法".to_string());
        }

        Ok(file_name)
    }

    fn parse_field(frontmatter: &str, field: &str) -> Option<String> {
        let pattern = format!(r#"{}:\s*['"]?([^'"\n]+)['"]?"#, field);
        regex::Regex::new(&pattern).ok().and_then(|regex| {
            regex
                .captures(frontmatter)
                .map(|captures| captures[1].trim().to_string())
        })
    }

    fn parse_parts(
        content: &str,
    ) -> (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
    ) {
        let match_result = regex::Regex::new(r"(?s)^---\n(.*?)\n---\n?(.*)$")
            .ok()
            .and_then(|regex| regex.captures(content));

        if let Some(captures) = match_result {
            let frontmatter = captures[1].to_string();
            let body = captures[2].to_string();
            let inclusion = Self::parse_field(&frontmatter, "inclusion")
                .unwrap_or_else(|| "always".to_string());
            let name = Self::parse_field(&frontmatter, "name");
            let description = Self::parse_field(&frontmatter, "description");
            let file_match_pattern = Self::parse_field(&frontmatter, "fileMatchPattern");
            (inclusion, name, description, file_match_pattern, body)
        } else {
            ("always".to_string(), None, None, None, content.to_string())
        }
    }

    fn normalize_body(body: &str) -> String {
        let normalized = body.replace("\r\n", "\n").replace('\r', "\n");
        let collapsed = regex::Regex::new(r"\n{3,}")
            .ok()
            .map(|regex| regex.replace_all(&normalized, "\n\n").to_string())
            .unwrap_or(normalized);
        collapsed.trim().to_string()
    }

    fn build_content(
        inclusion: &str,
        name: &str,
        description: &str,
        file_match_pattern: Option<&str>,
        body: &str,
    ) -> String {
        let mut frontmatter = format!("---\ninclusion: {inclusion}");

        if !name.trim().is_empty() {
            frontmatter.push_str(&format!("\nname: \"{}\"", name.trim()));
        }

        if !description.trim().is_empty() {
            frontmatter.push_str(&format!("\ndescription: \"{}\"", description.trim()));
        }

        if inclusion == "fileMatch" {
            let pattern = file_match_pattern.unwrap_or("**/*").trim();
            frontmatter.push_str(&format!("\nfileMatchPattern: '{pattern}'"));
        }

        format!("{frontmatter}\n---\n{}\n", body.trim())
    }

    fn create_default_body(file_name: &str) -> String {
        let title = file_name.trim_end_matches(".md");
        format!(
            "## 适用范围\n- 该规则适用于当前工作区的日常协作\n- 修改前先确认受影响目录和文件边界\n\n## 执行要求\n- 优先做最小改动，避免无关重构\n- 完成后至少运行与变更直接相关的构建或测试\n- 遇到不确定行为，先回读现有实现再调整\n\n## 备注\n- 当前模板由规则管理页面生成，可按项目需要继续细化\n- 模板标识：{title}"
        )
    }

    fn detect_workspace_signals(project_dir: &Path) -> Vec<String> {
        let mut signals = Vec::new();

        for entry in [
            "package.json",
            "Cargo.toml",
            "src",
            "src-tauri",
            "docs",
            "locales",
            "AGENTS.md",
        ] {
            if project_dir.join(entry).exists() {
                signals.push(entry.to_string());
            }
        }

        signals
    }

    fn build_initial_project_content(project_dir: &Path) -> String {
        let project_name = project_dir
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "当前工作区".to_string());
        let signals = Self::detect_workspace_signals(project_dir);
        let signal_lines = if signals.is_empty() {
            "- 未识别到典型的项目入口文件".to_string()
        } else {
            signals
                .iter()
                .map(|signal| format!("- {signal}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            "## 工作区概览\n- 项目目录：{project_name}\n- 下面这些入口已经在初始化时识别到：\n{signal_lines}\n\n## 建议执行方式\n- 优先在既有目录结构内修改，不新增无关文件\n- 前端改动后运行 `bun build`\n- Rust / Tauri 改动后运行 `cargo test`\n\n## 协作要求\n- 先确认文件边界，再做改动\n- 输出结论时附上验证证据\n- 避免把临时分析产物混入正式仓库"
        )
    }

    fn next_available_file_name(dir: &Path, base_name: &str) -> String {
        if !dir.join(base_name).exists() {
            return base_name.to_string();
        }

        let stem = base_name.trim_end_matches(".md");
        let mut index = 2;
        loop {
            let candidate = format!("{stem}-{index}.md");
            if !dir.join(&candidate).exists() {
                return candidate;
            }
            index += 1;
        }
    }

    fn resolve_file_path(
        file_name: &str,
        scope: &str,
        project_dir: Option<&str>,
    ) -> Result<PathBuf, String> {
        let safe_name = Self::sanitize_file_name(file_name)?;
        let dir = Self::resolve_dir(scope, project_dir)?;
        Ok(dir.join(safe_name))
    }

    /// 读取所有 steering 文件（合并用户级和项目级）
    pub fn load_all(project_dir: Option<&str>) -> Result<Vec<SteeringFile>, String> {
        let mut all_files = vec![];

        if let Some(dir) = Self::user_dir() {
            all_files.extend(Self::load_from_dir(&dir, "user")?);
        }

        if let Some(pd) = project_dir {
            let dir = Self::project_dir(pd);
            all_files.extend(Self::load_from_dir(&dir, "project")?);
        }

        Ok(all_files)
    }

    /// 读取单个 steering 文件
    pub fn load(
        file_name: &str,
        scope: &str,
        project_dir: Option<&str>,
    ) -> Result<SteeringFile, String> {
        let path = Self::resolve_file_path(file_name, scope, project_dir)?;

        if !path.exists() {
            return Err(format!("Steering 文件不存在: {file_name}"));
        }

        let content = fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {e}"))?;

        let metadata = fs::metadata(&path).ok();
        let size = metadata.as_ref().map_or(0, std::fs::Metadata::len);
        let modified_at = metadata.and_then(|m| m.modified().ok()).map(|t| {
            let datetime: chrono::DateTime<chrono::Local> = t.into();
            datetime.format("%Y/%m/%d %H:%M:%S").to_string()
        });

        Ok(SteeringFile {
            file_name: file_name.to_string(),
            content,
            size,
            modified_at,
            scope: scope.to_string(),
        })
    }

    /// 保存 steering 文件
    pub fn save(
        file_name: &str,
        content: &str,
        scope: &str,
        project_dir: Option<&str>,
    ) -> Result<(), String> {
        let dir = Self::resolve_dir(scope, project_dir)?;
        fs::create_dir_all(&dir).ok();

        let path = Self::resolve_file_path(file_name, scope, project_dir)?;
        fs::write(&path, content).map_err(|e| format!("写入失败: {e}"))
    }

    /// 删除 steering 文件
    pub fn delete(file_name: &str, scope: &str, project_dir: Option<&str>) -> Result<(), String> {
        let path = Self::resolve_file_path(file_name, scope, project_dir)?;

        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("删除失败: {e}"))?;
        }

        Ok(())
    }

    /// 创建新的 steering 文件
    pub fn create(
        file_name: &str,
        content: &str,
        scope: &str,
        project_dir: Option<&str>,
    ) -> Result<SteeringFile, String> {
        let dir = Self::resolve_dir(scope, project_dir)?;
        fs::create_dir_all(&dir).ok();

        let path = Self::resolve_file_path(file_name, scope, project_dir)?;

        if path.exists() {
            return Err(format!("文件已存在: {file_name}"));
        }

        fs::write(&path, content).map_err(|e| format!("写入失败: {e}"))?;

        Self::load(file_name, scope, project_dir)
    }

    pub fn create_default(
        file_name: Option<&str>,
        scope: &str,
        project_dir: Option<&str>,
    ) -> Result<SteeringFile, String> {
        let dir = Self::resolve_dir(scope, project_dir)?;
        fs::create_dir_all(&dir).map_err(|e| format!("创建 Steering 目录失败: {e}"))?;

        let preferred_name = file_name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                if value.ends_with(".md") {
                    value.to_string()
                } else {
                    format!("{value}.md")
                }
            })
            .unwrap_or_else(|| "default-rules.md".to_string());

        let actual_file_name = Self::next_available_file_name(&dir, &preferred_name);
        let display_name = actual_file_name.trim_end_matches(".md");
        let content = Self::build_content(
            "always",
            display_name,
            "规则页生成的默认 Steering 模板",
            None,
            &Self::create_default_body(&actual_file_name),
        );

        Self::create(&actual_file_name, &content, scope, project_dir)
    }

    pub fn create_initial_for_project(project_dir: &str) -> Result<Vec<SteeringFile>, String> {
        let steering_dir = Self::project_dir(project_dir);
        fs::create_dir_all(&steering_dir)
            .map_err(|e| format!("创建项目 Steering 目录失败: {e}"))?;

        let file_name = if steering_dir.join("workspace-context.md").exists() {
            "workspace-context.md".to_string()
        } else {
            Self::next_available_file_name(&steering_dir, "workspace-context.md")
        };

        let content = Self::build_content(
            "always",
            "workspace-context",
            "根据当前工作区结构生成的初始化规则",
            None,
            &Self::build_initial_project_content(&PathBuf::from(project_dir)),
        );

        let file = if steering_dir.join(&file_name).exists() {
            Self::load(&file_name, "project", Some(project_dir))?
        } else {
            Self::create(&file_name, &content, "project", Some(project_dir))?
        };

        Ok(vec![file])
    }

    pub fn refine_content(file_name: &str, content: &str) -> String {
        let (inclusion, name, description, file_match_pattern, body) = Self::parse_parts(content);
        let normalized_body = Self::normalize_body(&body);
        let final_body = if normalized_body.is_empty() {
            "## 规则说明\n- 在这里补充更具体的执行要求".to_string()
        } else {
            normalized_body
        };
        let inferred_name = name.unwrap_or_else(|| file_name.trim_end_matches(".md").to_string());
        let inferred_description = description.unwrap_or_else(|| match inclusion.as_str() {
            "auto" => "按需激活的上下文规则".to_string(),
            "fileMatch" => "按文件匹配自动加载的规则".to_string(),
            "manual" => "需要手动引用的规则".to_string(),
            _ => "整理后的 Steering 规则".to_string(),
        });

        Self::build_content(
            &inclusion,
            &inferred_name,
            &inferred_description,
            file_match_pattern.as_deref(),
            &final_body,
        )
    }

    pub fn refine_file(
        file_name: &str,
        scope: &str,
        project_dir: Option<&str>,
    ) -> Result<SteeringFile, String> {
        let existing = Self::load(file_name, scope, project_dir)?;
        let refined = Self::refine_content(file_name, &existing.content);
        Self::save(file_name, &refined, scope, project_dir)?;
        Self::load(file_name, scope, project_dir)
    }
}

// ---------- 嵌套 AGENTS.md（Kiro 1.1.14）----------
//
// 逆向结论（详见 docs/Kiro 1.1.14/嵌套AGENTS.md加载机制.md）：
// `AGENTS.md` 与 `.kiro/steering/*.md` 同属 Steering 子系统，Kiro 会递归找出
// 工作区里**每一层目录**的 AGENTS.md，与 steering 文档合并成同一份清单，且排序为
//   根 AGENTS.md  →  嵌套 AGENTS.md  →  .kiro/steering/*.md  →  全局 steering
// 因此这里把它挂在 SteeringManager 下，而不是另起模块。

/// 一个目录级的 `AGENTS.md`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsMdFile {
    /// 相对项目根的路径（正斜杠），如 `AGENTS.md` 或 `src/api/AGENTS.md`
    pub rel_path: String,
    /// 所在目录（相对项目根）；根目录为 `""`
    pub dir_rel: String,
    /// 距项目根的层级；`0` = 项目根的那个
    pub depth: usize,
    pub content: String,
    pub size: u64,
    pub modified_at: Option<String>,
    /// 目前固定为 `"project"`（IDE 只在工作区内扫描嵌套 AGENTS.md）
    pub scope: String,
}

pub const AGENTS_MD: &str = "AGENTS.md";

/// 递归时跳过的目录：版本控制、依赖、构建产物、编辑器配置。
/// 与 IDE 的忽略规则不完全等价（IDE 走 fs_read 权限 + .gitignore + .kiroignore），
/// 但能覆盖绝大多数场景，且不需要引入额外的依赖。
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".venv",
    "venv",
    "__pycache__",
    ".idea",
    ".vscode",
];

/// 递归深度上限，防止超大仓库或异常符号链接导致长时间遍历。
const MAX_DEPTH: usize = 12;

impl SteeringManager {
    /// 递归扫描项目内的所有 `AGENTS.md`。
    ///
    /// 结果**按 Kiro 的顺序排序**：根目录的排最前，其后按层级、再按路径。
    /// 目录不存在 / 无权限的子目录会被静默跳过，不整体失败。
    pub fn scan_agents_md(project_dir: &str) -> Result<Vec<AgentsMdFile>, String> {
        let root = PathBuf::from(project_dir);
        if !root.exists() {
            return Err(format!("项目目录不存在: {project_dir}"));
        }
        if !root.is_dir() {
            return Err(format!("不是目录: {project_dir}"));
        }

        let mut out: Vec<AgentsMdFile> = Vec::new();
        Self::walk_agents_md(&root, &root, 0, &mut out);
        out.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.rel_path.cmp(&b.rel_path)));
        Ok(out)
    }

    fn walk_agents_md(root: &Path, dir: &Path, depth: usize, out: &mut Vec<AgentsMdFile>) {
        if depth > MAX_DEPTH {
            return;
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            // 无权限 / 已删除等一律跳过，不让单点故障中断整个扫描
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            if path.is_dir() {
                if SKIP_DIRS.iter().any(|d| name.eq_ignore_ascii_case(d)) {
                    continue;
                }
                Self::walk_agents_md(root, &path, depth + 1, out);
                continue;
            }

            if !name.eq_ignore_ascii_case(AGENTS_MD) {
                continue;
            }

            let rel = path.strip_prefix(root).unwrap_or(Path::new(""));
            let rel_path = rel.to_string_lossy().replace('\\', "/");
            let dir_rel = rel
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();

            let metadata = fs::metadata(&path).ok();
            let size = metadata.as_ref().map_or(0, std::fs::Metadata::len);
            let modified_at = metadata.and_then(|m| m.modified().ok()).map(|t| {
                let datetime: chrono::DateTime<chrono::Local> = t.into();
                datetime.format("%Y/%m/%d %H:%M:%S").to_string()
            });
            let content = fs::read_to_string(&path).unwrap_or_default();

            out.push(AgentsMdFile {
                rel_path,
                dir_rel,
                depth,
                content,
                size,
                modified_at,
                scope: "project".to_string(),
            });
        }
    }

    /// 解析出一个安全的 `AGENTS.md` 绝对路径。
    ///
    /// 三重校验，防止路径穿越：
    /// 1. 拒绝绝对路径与 `..` 组件
    /// 2. canonicalize 后必须仍在项目根之下
    /// 3. 文件名必须是 `AGENTS.md`（不区分大小写）
    fn resolve_agents_md_path(project_dir: &str, rel_path: &str) -> Result<PathBuf, String> {
        let rel = Path::new(rel_path);
        if rel.is_absolute()
            || rel
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::RootDir))
        {
            return Err("非法的相对路径".to_string());
        }

        let root = PathBuf::from(project_dir)
            .canonicalize()
            .map_err(|e| format!("项目目录解析失败: {e}"))?;
        if !root.is_dir() {
            return Err("项目路径不是目录".to_string());
        }

        let full = root
            .join(rel)
            .canonicalize()
            .map_err(|e| format!("文件解析失败: {e}"))?;
        if !full.starts_with(&root) {
            return Err("路径越界".to_string());
        }

        match full.file_name().and_then(|n| n.to_str()) {
            Some(n) if n.eq_ignore_ascii_case(AGENTS_MD) => Ok(full),
            _ => Err(format!("只允许操作 {AGENTS_MD}")),
        }
    }

    /// 读取指定 `AGENTS.md` 的内容。
    pub fn read_agents_md(project_dir: &str, rel_path: &str) -> Result<String, String> {
        let path = Self::resolve_agents_md_path(project_dir, rel_path)?;
        fs::read_to_string(&path).map_err(|e| format!("读取 {AGENTS_MD} 失败: {e}"))
    }

    /// 写入指定 `AGENTS.md`（不存在则创建，父目录自动创建）。
    ///
    /// 与读取不同：这里**不能** canonicalize 目标文件（它可能还不存在），
    /// 因此改为「拒绝绝对路径与 `..` 组件 + 拼接已规范化的项目根」——
    /// 这两条约束已足以保证路径不越界。
    pub fn write_agents_md(project_dir: &str, rel_path: &str, content: &str) -> Result<(), String> {
        let rel = Path::new(rel_path);
        if rel.is_absolute()
            || rel
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::RootDir))
        {
            return Err("非法的相对路径".to_string());
        }
        let root = PathBuf::from(project_dir)
            .canonicalize()
            .map_err(|e| format!("项目目录解析失败: {e}"))?;
        let path = root.join(rel);

        match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if n.eq_ignore_ascii_case(AGENTS_MD) => {}
            _ => return Err(format!("只允许操作 {AGENTS_MD}")),
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }
        fs::write(&path, content).map_err(|e| format!("写入 {AGENTS_MD} 失败: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::SteeringManager;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "kiro-account-manager-steering-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).expect("temp dir should be created");
        path
    }

    #[test]
    fn create_initial_project_steering_generates_workspace_file() {
        let project_root = temp_dir("project");
        fs::create_dir_all(project_root.join("src")).expect("src dir should exist");
        fs::write(project_root.join("package.json"), "{ \"name\": \"demo\" }")
            .expect("package.json should exist");
        fs::write(
            project_root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .expect("Cargo.toml should exist");

        let generated =
            SteeringManager::create_initial_for_project(project_root.to_string_lossy().as_ref())
                .expect("initial steering should be generated");

        assert!(
            !generated.is_empty(),
            "initial steering should create at least one file"
        );
        let content = &generated[0].content;
        assert!(
            content.contains("package.json"),
            "workspace summary should mention package.json"
        );
        assert!(
            content.contains("Cargo.toml"),
            "workspace summary should mention Cargo.toml"
        );

        fs::remove_dir_all(project_root).ok();
    }

    #[test]
    fn refine_content_adds_missing_frontmatter_and_normalizes_body() {
        let refined = SteeringManager::refine_content(
            "workspace-guidelines.md",
            "请严格控制变更范围。\n\n\n优先跑构建验证。\n",
        );

        assert!(
            refined.contains("name: \"workspace-guidelines\""),
            "refine should add name"
        );
        assert!(
            refined.contains("description:"),
            "refine should add description"
        );
        assert!(
            refined.contains("请严格控制变更范围。"),
            "body should be preserved"
        );
        assert!(
            !refined.contains("\n\n\n"),
            "extra blank lines should be collapsed"
        );
    }
}
