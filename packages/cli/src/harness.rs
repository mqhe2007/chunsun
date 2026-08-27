use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::api::{ApiClient, ApiError};
use crate::ide::{default_ide_target, IdeId, IdeTarget, IDE_TARGETS};

/// 已安装模板的版本文件名（落在 `<ide>/skills/chunsun/`）。
pub const TEMPLATE_VERSION_FILENAME: &str = ".template-version";

/// AGENTS.md 桥接段落的 marker：只管理 marker 之间的内容，其余不动。
pub const AGENTS_BRIDGE_BEGIN: &str = "<!-- chunsun:begin -->";
pub const AGENTS_BRIDGE_END: &str = "<!-- chunsun:end -->";

/// 实例 `GET /harness/template` 返回的模板包（运行时真相源）。
#[derive(Debug, Clone)]
pub struct HarnessTemplateBundle {
    pub template_version: String,
    pub skill: String,
    pub commands: String,
    pub loop_rules: String,
    pub slash_chunsun: String,
    pub slash_chunsun_fix: String,
}

#[derive(Debug, Deserialize)]
struct TemplateApiResponse {
    success: bool,
    data: Option<TemplateApiData>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TemplateApiData {
    template_version: String,
    files: Value,
}

fn require_file(files: &Value, key: &str) -> Result<String, ApiError> {
    files
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::Message(format!("模板响应缺少文件：{key}")))
}

/// 从实例拉取 harness 模板（需合法 Bearer secretKey / JWT）。
pub fn fetch_harness_template(api: &ApiClient) -> Result<HarnessTemplateBundle, ApiError> {
    let res: TemplateApiResponse = api.get("/harness/template")?;
    if !res.success {
        return Err(ApiError::Message(
            res.error
                .unwrap_or_else(|| "拉取 harness 模板失败".into()),
        ));
    }
    let data = res
        .data
        .ok_or_else(|| ApiError::Message("拉取 harness 模板失败：响应无 data".into()))?;
    if data.template_version.trim().is_empty() {
        return Err(ApiError::Message(
            "拉取 harness 模板失败：templateVersion 为空".into(),
        ));
    }
    Ok(HarnessTemplateBundle {
        template_version: data.template_version,
        skill: require_file(&data.files, "SKILL.md")?,
        commands: require_file(&data.files, "commands.md")?,
        loop_rules: require_file(&data.files, "loop-rules.md")?,
        slash_chunsun: require_file(&data.files, "slash/chunsun.md")?,
        slash_chunsun_fix: require_file(&data.files, "slash/chunsun-fix.md")?,
    })
}

#[derive(Debug)]
pub struct WorkflowInstallFile {
    pub relative_path: String,
    pub content: String,
}

fn rules_for_ide(ide: &IdeTarget, loop_rules: &str) -> String {
    if ide.rules_frontmatter.is_empty() {
        // Claude Code：规则没有 alwaysApply 字段，省略 frontmatter 即全局规则（启动时无条件加载）。
        // 直接落盘正文，不包裹 Cursor 式 frontmatter。
        loop_rules.to_string()
    } else {
        format!("{}{}", ide.rules_frontmatter, loop_rules)
    }
}

pub fn list_workflow_install_files(
    ide: &IdeTarget,
    bundle: &HarnessTemplateBundle,
) -> Vec<WorkflowInstallFile> {
    let mut files = Vec::new();
    files.push(WorkflowInstallFile {
        relative_path: format!("{}/chunsun/references/loop-rules.md", ide.skills_dir),
        content: bundle.loop_rules.clone(),
    });
    // 仅当该 IDE 支持门禁规则时才安装（WorkBuddy 不读取 .workbuddy/rules）。
    if ide.supports_rules {
        files.push(WorkflowInstallFile {
            relative_path: format!("{}/{}", ide.rules_dir, ide.rules_filename),
            content: rules_for_ide(ide, &bundle.loop_rules),
        });
    }
    // 仅当该 IDE 支持斜线命令时才安装（WorkBuddy 不读取 .workbuddy/commands）。
    if ide.supports_commands {
        for (name, content) in [
            ("chunsun.md", bundle.slash_chunsun.as_str()),
            ("chunsun-fix.md", bundle.slash_chunsun_fix.as_str()),
        ] {
            files.push(WorkflowInstallFile {
                relative_path: format!("{}/{}", ide.commands_dir, name),
                content: content.to_string(),
            });
        }
    }
    files
}

/// AGENTS.md 桥接段落内容（指针式，引用而非复制规则正文）。
///
/// 仅当 IDE 支持门禁规则时才在桥接段中引用规则文件；
/// 否则（如 WorkBuddy）只指向技能，避免指向不存在的 `.workbuddy/rules` 路径。
fn agents_bridge_section(ide: &IdeTarget) -> String {
    let skill = format!("{}/chunsun/SKILL.md", ide.skills_dir);
    let mut body = format!(
        "{AGENTS_BRIDGE_BEGIN}\n## 春笋（chunsun）\n\n「春笋」是 AI 原生的项目交付平台。本仓库接入春笋自主交付 harness：涉及需求 / 缺陷 / 验收的工作一律走 `/chunsun <需求ID>` 或 `/chunsun-fix <缺陷ID>`。\n核心技能见 `{skill}`。"
    );
    if ide.supports_rules {
        let rules = format!("{}/{}", ide.rules_dir, ide.rules_filename);
        body.push_str(&format!(
            " 核心规则（验收定义 / 停点 / 状态 / 边界）见 `{rules}`（常驻生效）。"
        ));
    }
    body.push_str(&format!("\n{AGENTS_BRIDGE_END}"));
    body
}

/// 在仓库根 markdown 文件维护 chunsun 桥接段落（跨 IDE 常驻层双保险）。
///
/// - 文件不存在 → 创建，仅含桥接段落；
/// - 有完整 marker → 整体替换 marker 之间的内容，其余原样保留；
/// - 有孤立 begin marker（损坏）→ 移除该 marker 后追加完整段落，不丢用户内容；
/// - 无 marker → 末尾追加段落。
///
/// 幂等：内容无变化时计入 reused，不写盘。
///
/// 常规 IDE（Cursor 等）桥接到 `AGENTS.md`；Claude Code 官方读取 `CLAUDE.md` 而非
/// AGENTS.md，故桥接到 `CLAUDE.md`（见 `IdeTarget::writes_claude_md`）。
fn upsert_md_bridge(
    cwd: &Path,
    filename: &str,
    ide: &IdeTarget,
    reused: &mut Vec<String>,
    written: &mut Vec<String>,
) -> std::io::Result<()> {
    let path = cwd.join(filename);
    let section = agents_bridge_section(ide);
    let existing = fs::read_to_string(&path).unwrap_or_default();

    let next = if existing.is_empty() {
        format!("# {filename}\n\n{section}\n")
    } else if let Some(begin) = existing.find(AGENTS_BRIDGE_BEGIN) {
        match existing[begin..].find(AGENTS_BRIDGE_END) {
            Some(rel_end) => {
                let end = begin + rel_end + AGENTS_BRIDGE_END.len();
                format!("{}{}{}", &existing[..begin], section, &existing[end..])
            }
            None => {
                let cleaned = existing.replacen(AGENTS_BRIDGE_BEGIN, "", 1);
                let mut s = cleaned;
                if !s.ends_with('\n') {
                    s.push('\n');
                }
                format!("{s}\n{section}\n")
            }
        }
    } else {
        let mut s = existing.clone();
        if !s.ends_with('\n') {
            s.push('\n');
        }
        format!("{s}\n{section}\n")
    };

    if next == existing {
        reused.push(filename.to_string());
        return Ok(());
    }
    fs::write(&path, next)?;
    written.push(filename.to_string());
    Ok(())
}

/// 仓库根 AGENTS.md 桥接（常规 IDE 的跨 IDE 常驻层双保险）。
fn upsert_agents_bridge(
    cwd: &Path,
    ide: &IdeTarget,
    reused: &mut Vec<String>,
    written: &mut Vec<String>,
) -> std::io::Result<()> {
    upsert_md_bridge(cwd, "AGENTS.md", ide, reused, written)
}

pub fn template_version_path(cwd: &Path, ide: &IdeTarget) -> PathBuf {
    cwd.join(ide.skills_dir)
        .join("chunsun")
        .join(TEMPLATE_VERSION_FILENAME)
}

/// 读取已安装的模板版本号。
///
/// 优先从检测到的 IDE（或默认 IDE）的 `<ide>/skills/chunsun/.template-version` 读取；
/// 若新位置不存在，回退到旧版 `.agents/skills/chunsun/.template-version`（迁移兼容）。
pub fn read_installed_template_version(cwd: &Path) -> Option<String> {
    let detected = detect_installed_ide_targets(cwd);
    let candidates: Vec<&IdeTarget> = if detected.is_empty() {
        vec![default_ide_target()]
    } else {
        detected
    };
    for ide in candidates {
        if let Some(v) = read_version_at(cwd, ide) {
            return Some(v);
        }
    }
    // 兼容旧版 .agents/skills/chunsun 位置
    let legacy = cwd
        .join(".agents")
        .join("skills")
        .join("chunsun")
        .join(TEMPLATE_VERSION_FILENAME);
    if let Ok(raw) = fs::read_to_string(&legacy) {
        let version = raw.trim();
        if !version.is_empty() {
            return Some(version.to_string());
        }
    }
    None
}

fn read_version_at(cwd: &Path, ide: &IdeTarget) -> Option<String> {
    let raw = fs::read_to_string(template_version_path(cwd, ide)).ok()?;
    let version = raw.trim();
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

pub fn is_skill_template_stale(cwd: &Path, remote_version: &str) -> bool {
    read_installed_template_version(cwd).as_deref() != Some(remote_version)
}

fn path_exists(p: &Path) -> bool {
    p.exists()
}

fn write_file_tracked(
    file_path: &Path,
    content: &str,
    reused: &mut Vec<String>,
    written: &mut Vec<String>,
    relative_to: &Path,
    overwrite: bool,
) -> std::io::Result<()> {
    let relative = file_path
        .strip_prefix(relative_to)
        .unwrap_or(file_path)
        .to_string_lossy()
        .into_owned();
    if path_exists(file_path) && !overwrite {
        reused.push(relative);
        return Ok(());
    }
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(file_path, content)?;
    written.push(relative);
    Ok(())
}

#[derive(Debug)]
pub struct InstallSkillWorkspaceResult {
    pub skill_root: PathBuf,
    pub reused: Vec<String>,
    pub written: Vec<String>,
    pub refreshed: bool,
    pub previous_version: Option<String>,
    pub template_version: String,
    pub ide: IdeId,
}

pub fn install_skill_workspace(
    cwd: &Path,
    force: bool,
    ide: Option<&IdeTarget>,
    bundle: &HarnessTemplateBundle,
) -> std::io::Result<InstallSkillWorkspaceResult> {
    let ide = ide.unwrap_or_else(|| default_ide_target());
    let mut reused = Vec::new();
    let mut written = Vec::new();
    let skill_root = cwd.join(ide.skills_dir).join("chunsun");
    let previous_version = read_installed_template_version(cwd);
    let refreshed =
        force || previous_version.as_deref() != Some(bundle.template_version.as_str());

    let mut core_files = vec![
        WorkflowInstallFile {
            relative_path: format!("{}/chunsun/SKILL.md", ide.skills_dir),
            content: bundle.skill.clone(),
        },
        WorkflowInstallFile {
            relative_path: format!("{}/chunsun/references/commands.md", ide.skills_dir),
            content: bundle.commands.clone(),
        },
    ];
    core_files.extend(list_workflow_install_files(ide, bundle));

    for file in &core_files {
        let abs = cwd.join(&file.relative_path);
        write_file_tracked(
            &abs,
            &file.content,
            &mut reused,
            &mut written,
            cwd,
            refreshed,
        )?;
    }

    let version_abs = template_version_path(cwd, ide);
    if let Some(parent) = version_abs.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&version_abs, format!("{}\n", bundle.template_version))?;
    let version_rel = version_abs
        .strip_prefix(cwd)
        .unwrap_or(&version_abs)
        .to_string_lossy()
        .into_owned();
    if !written.contains(&version_rel) && !reused.contains(&version_rel) {
        written.push(version_rel);
    }

    // AGENTS.md 桥接段落（marker 管理、幂等）：跨 IDE 的常驻层双保险。
    upsert_agents_bridge(cwd, ide, &mut reused, &mut written)?;

    // Claude Code 官方读取 CLAUDE.md 而非 AGENTS.md：额外维护仓库根 CLAUDE.md 桥接
    // （同一 marker 语义、幂等；AGENTS.md 桥接对 Claude Code 无效但保留给其它 IDE）。
    if ide.writes_claude_md {
        upsert_md_bridge(cwd, "CLAUDE.md", ide, &mut reused, &mut written)?;
    }

    // 迁移清理：不再维护 `.agents`，装完即移除其下 chunsun 旧产物（skills / commands / rules）。
    // 目标即 Agents（`.agents`）时为合法安装目标，跳过清理避免误删本次产物。
    if ide.id != IdeId::Agents {
        cleanup_legacy_agents(cwd);
    }

    Ok(InstallSkillWorkspaceResult {
        skill_root,
        reused,
        written,
        refreshed,
        previous_version,
        template_version: bundle.template_version.clone(),
        ide: ide.id,
    })
}

/// 从实例拉取模板后安装到当前仓库（`init` / `update` 入口）。
pub fn install_skill_workspace_from_api(
    api: &ApiClient,
    cwd: &Path,
    force: bool,
    ide: Option<&IdeTarget>,
) -> Result<InstallSkillWorkspaceResult, ApiError> {
    let bundle = fetch_harness_template(api)?;
    install_skill_workspace(cwd, force, ide, &bundle).map_err(|e| ApiError::Message(e.to_string()))
}

/// 检测已安装的 IDE 目标：技能目录（`<ide>/skills/chunsun`）对每家 IDE 都会安装，
/// 比仅靠 commands_dir 更稳健——WorkBuddy 没有 commands_dir，但仍会被正确识别。
pub fn detect_installed_ide_targets(cwd: &Path) -> Vec<&'static IdeTarget> {
    IDE_TARGETS
        .iter()
        .filter(|t| path_exists(&cwd.join(t.skills_dir).join("chunsun")))
        .collect()
}

/// 移除旧版 `.agents` 下 chunsun 自有的安装产物（迁移到 IDE 专属目录后不再维护 `.agents`）。
///
/// 旧版 chunsun 曾把技能、斜线命令、门禁规则全部装在 `.agents/` 下：
/// - `.agents/skills/chunsun/`
/// - `.agents/commands/chunsun.md`、`chunsun-fix.md`
/// - `.agents/rules/chunsun-workflow-gates.*`（扩展名随旧 IDE 而定）
///
/// 仅删除 chunsun 明确拥有的文件/子目录，不动 `.agents` 下其它内容；
/// 若 `skills`/`commands`/`rules` 因此变空则移除空目录，`.agents` 自身为空也一并移除。
pub fn cleanup_legacy_agents(cwd: &Path) {
    let agents = cwd.join(".agents");

    // 1) 技能目录（整目录）
    let legacy_skill = agents.join("skills").join("chunsun");
    if legacy_skill.exists() {
        let _ = fs::remove_dir_all(&legacy_skill);
    }

    // 2) 斜线命令：旧版整体装在 `.agents/commands` 下的 chunsun 命令文件
    let legacy_commands = agents.join("commands");
    if legacy_commands.exists() {
        for name in ["chunsun.md", "chunsun-fix.md"] {
            let p = legacy_commands.join(name);
            if p.exists() {
                let _ = fs::remove_file(&p);
            }
        }
    }

    // 3) 门禁规则：`.agents/rules/chunsun-workflow-gates.*`（扩展名随旧 IDE 而定）
    let legacy_rules = agents.join("rules");
    if legacy_rules.exists() {
        if let Ok(entries) = fs::read_dir(&legacy_rules) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let s = name.to_string_lossy();
                if s.starts_with("chunsun-workflow-gates") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    // 4) 子目录若因此变空则移除；`.agents` 自身为空也一并移除。
    for sub in ["skills", "commands", "rules"] {
        let dir = agents.join(sub);
        if dir.exists() {
            let is_empty = fs::read_dir(&dir)
                .map(|mut it| it.next().is_none())
                .unwrap_or(false);
            if is_empty {
                let _ = fs::remove_dir_all(&dir);
            }
        }
    }
    if agents.exists() {
        let is_empty = fs::read_dir(&agents)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if is_empty {
            let _ = fs::remove_dir_all(&agents);
        }
    }
}

#[derive(Debug)]
pub struct SkillTemplateRefreshResult {
    pub skill_installed: bool,
    pub refreshed: bool,
    pub previous_version: Option<String>,
    pub template_version: String,
    pub ides: Vec<IdeId>,
}

pub fn refresh_installed_skill_templates(
    api: &ApiClient,
    cwd: &Path,
) -> Result<SkillTemplateRefreshResult, ApiError> {
    let bundle = fetch_harness_template(api)?;
    let detected = detect_installed_ide_targets(cwd);
    let targets: Vec<&IdeTarget> = if detected.is_empty() {
        vec![default_ide_target()]
    } else {
        detected
    };
    let previous_version = read_installed_template_version(cwd);
    let skill_installed = targets
        .iter()
        .any(|ide| path_exists(&cwd.join(ide.skills_dir).join("chunsun")))
        || path_exists(&cwd.join(".agents").join("skills").join("chunsun"));

    if !skill_installed {
        return Ok(SkillTemplateRefreshResult {
            skill_installed: false,
            refreshed: false,
            previous_version,
            template_version: bundle.template_version,
            ides: vec![],
        });
    }

    let detected = detect_installed_ide_targets(cwd);
    let targets: Vec<&IdeTarget> = if detected.is_empty() {
        vec![default_ide_target()]
    } else {
        detected
    };
    let stale = is_skill_template_stale(cwd, &bundle.template_version);

    for ide in &targets {
        install_skill_workspace(cwd, stale, Some(ide), &bundle)
            .map_err(|e| ApiError::Message(e.to_string()))?;
    }

    Ok(SkillTemplateRefreshResult {
        skill_installed: true,
        refreshed: stale,
        previous_version,
        template_version: bundle.template_version,
        ides: targets.iter().map(|t| t.id).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ide::get_ide_target;
    use tempfile::tempdir;

    /// 单测夹具：只在 test cfg 下 include 后端模板正文，生产二进制不嵌入。
    fn fixture_version() -> String {
        include_str!("../../backend/templates/VERSION").trim().to_string()
    }

    fn fixture_bundle() -> HarnessTemplateBundle {
        HarnessTemplateBundle {
            template_version: include_str!("../../backend/templates/VERSION")
                .trim()
                .to_string(),
            skill: include_str!("../../backend/templates/skill.md").to_string(),
            commands: include_str!("../../backend/templates/commands.md").to_string(),
            loop_rules: include_str!("../../backend/templates/loop-rules.md").to_string(),
            slash_chunsun: include_str!("../../backend/templates/slash/chunsun.md").to_string(),
            slash_chunsun_fix: include_str!("../../backend/templates/slash/chunsun-fix.md")
                .to_string(),
        }
    }

    #[test]
    fn install_and_refresh_templates() {
        let dir = tempdir().unwrap();
        let result = install_skill_workspace(dir.path(), false, None, &fixture_bundle()).unwrap();
        assert!(result.refreshed);
        assert!(dir.path().join(".cursor/skills/chunsun/SKILL.md").is_file());
        assert_eq!(
            read_installed_template_version(dir.path()),
            Some(fixture_version())
        );

        let again = install_skill_workspace(dir.path(), false, None, &fixture_bundle()).unwrap();
        assert!(!again.refreshed);
        assert!(!again.reused.is_empty());
    }

    /// 硬切：只安装 /chunsun /chunsun-fix 两个斜线命令，旧命令与门禁/排期/阶段引用不再落盘。
    #[test]
    fn harness_hard_cut_slash_and_no_gates() {
        let dir = tempdir().unwrap();
        install_skill_workspace(dir.path(), false, None, &fixture_bundle()).unwrap();
        for old in ["探索.md", "实施.md", "测试.md", "暂停.md", "废弃.md", "恢复.md", "修复.md"] {
            assert!(
                !dir.path().join(".cursor/commands").join(old).exists(),
                "旧斜线命令 {old} 不应再安装",
            );
        }
        assert!(dir.path().join(".cursor/commands/chunsun.md").is_file());
        assert!(dir.path().join(".cursor/commands/chunsun-fix.md").is_file());
        assert!(
            !dir.path().join(".cursor/skills/chunsun/references/门禁.md").exists(),
            "门禁引用不应再安装",
        );
        assert!(
            !dir.path().join(".cursor/skills/chunsun/references/排期确认.md").exists(),
            "排期引用不应再安装",
        );
        assert!(
            !dir.path().join(".cursor/skills/chunsun/references/stages").exists(),
            "阶段引用不应再安装",
        );
        assert!(
            dir.path().join(".cursor/skills/chunsun/references/loop-rules.md").is_file(),
            "自主交付核心规则应安装",
        );
    }

    /// 技能从 .agents 迁移到所选 IDE 的 skills 目录，并清理旧 .agents 目录。
    #[test]
    fn migrates_legacy_agents_to_ide_skills() {
        let dir = tempdir().unwrap();
        // 模拟旧版布局：.agents/skills/chunsun/ 下装有技能与旧版本号
        let legacy_root = dir.path().join(".agents/skills/chunsun");
        fs::create_dir_all(legacy_root.join("references")).unwrap();
        fs::write(legacy_root.join("SKILL.md"), "legacy skill").unwrap();
        fs::write(
            legacy_root.join(TEMPLATE_VERSION_FILENAME),
            "2026-08-06-harness-long-loop\n",
        )
        .unwrap();
        assert!(legacy_root.exists());

        // 以 Cursor 为目标重新初始化
        install_skill_workspace(dir.path(), false, Some(default_ide_target()), &fixture_bundle()).unwrap();

        // 新位置已写入
        assert!(dir.path().join(".cursor/skills/chunsun/SKILL.md").is_file());
        assert!(dir.path().join(".cursor/skills/chunsun/.template-version").is_file());
        // 旧 .agents/skills/chunsun 已被清理
        assert!(
            !legacy_root.exists(),
            "迁移后旧 .agents/skills/chunsun 应被移除",
        );
        // 版本号已更新为新版
        assert_eq!(
            read_installed_template_version(dir.path()),
            Some(fixture_version())
        );
    }

    /// 旧版整体装在 `.agents` 下（skills + commands + rules）时，迁移后三者均被清理。
    #[test]
    fn migrates_legacy_agents_commands_and_rules() {
        let dir = tempdir().unwrap();
        // 模拟更早的旧版布局：技能、斜线命令、门禁规则都在 `.agents/` 下
        let legacy_skill = dir.path().join(".agents/skills/chunsun");
        fs::create_dir_all(legacy_skill.join("references")).unwrap();
        fs::write(legacy_skill.join("SKILL.md"), "legacy skill").unwrap();
        fs::write(
            legacy_skill.join(TEMPLATE_VERSION_FILENAME),
            "2026-08-06-harness-long-loop\n",
        )
        .unwrap();

        let legacy_commands = dir.path().join(".agents/commands");
        fs::create_dir_all(&legacy_commands).unwrap();
        fs::write(legacy_commands.join("chunsun.md"), "legacy cmd").unwrap();
        fs::write(legacy_commands.join("chunsun-fix.md"), "legacy fix").unwrap();
        // 同目录下若存在非 chunsun 文件，不应被误删
        fs::write(legacy_commands.join("other-tool.md"), "keep me").unwrap();

        let legacy_rules = dir.path().join(".agents/rules");
        fs::create_dir_all(&legacy_rules).unwrap();
        fs::write(
            legacy_rules.join("chunsun-workflow-gates.md"),
            "legacy gates",
        )
        .unwrap();

        // 以 CodeBuddy 为目标重新初始化
        let wb = get_ide_target("codebuddy").expect("codebuddy 应在 IDE 列表中");
        install_skill_workspace(dir.path(), false, Some(wb), &fixture_bundle()).unwrap();

        // 新位置已写入
        assert!(dir.path().join(".codebuddy/skills/chunsun/SKILL.md").is_file());
        assert!(dir.path().join(".codebuddy/commands/chunsun.md").is_file());
        assert!(dir.path().join(".codebuddy/rules/chunsun-workflow-gates.md").is_file());

        // 旧 .agents 三处 chunsun 产物均被清理
        assert!(
            !legacy_skill.exists(),
            "旧 .agents/skills/chunsun 应被移除",
        );
        assert!(
            !legacy_commands.join("chunsun.md").exists(),
            "旧 .agents/commands/chunsun.md 应被移除",
        );
        assert!(
            !legacy_commands.join("chunsun-fix.md").exists(),
            "旧 .agents/commands/chunsun-fix.md 应被移除",
        );
        assert!(
            !legacy_rules.join("chunsun-workflow-gates.md").exists(),
            "旧 .agents/rules/chunsun-workflow-gates.md 应被移除",
        );
        // 非 chunsun 文件保留；目录本身因仍含该文件而保留
        assert!(
            legacy_commands.join("other-tool.md").exists(),
            "非 chunsun 文件不应被误删",
        );
        assert!(
            legacy_commands.exists(),
            ".agents/commands 因含非 chunsun 文件不应被移除",
        );
        // .agents/skills 与 .agents/rules 已清空移除
        assert!(
            !dir.path().join(".agents/skills").exists(),
            "空的 .agents/skills 应被移除",
        );
        assert!(
            !legacy_rules.exists(),
            "空的 .agents/rules 应被移除",
        );
    }

    /// CodeBuddy 目标写入 `.codebuddy/skills/chunsun/` 与 `.codebuddy/commands/`。
    #[test]
    fn installs_to_codebuddy_skills_dir() {
        let dir = tempdir().unwrap();
        let wb = get_ide_target("codebuddy").expect("codebuddy 应在 IDE 列表中");
        install_skill_workspace(dir.path(), false, Some(wb), &fixture_bundle()).unwrap();
        assert!(dir.path().join(".codebuddy/skills/chunsun/SKILL.md").is_file());
        assert!(dir.path().join(".codebuddy/skills/chunsun/references/commands.md").is_file());
        assert!(dir.path().join(".codebuddy/commands/chunsun.md").is_file());
        assert!(dir.path().join(".codebuddy/commands/chunsun-fix.md").is_file());
        assert!(dir.path().join(".codebuddy/rules/chunsun-workflow-gates.md").is_file());
        assert!(!dir.path().join(".agents/skills/chunsun/SKILL.md").exists());
    }

    /// 夹具版本须与 backend templates/VERSION 一致（生产走实例接口，不再内嵌常量）。
    #[test]
    fn fixture_version_matches_backend_ssot() {
        assert_eq!(
            fixture_bundle().template_version,
            "2026-08-27-knowledge-load-strategy",
            "templates/VERSION 应已 bump 为新版本号",
        );
    }

    /// 模板版本号写入 .template-version（来自 bundle，非编译期常量）。
    #[test]
    fn template_version_written_from_bundle() {
        let dir = tempdir().unwrap();
        let bundle = fixture_bundle();
        install_skill_workspace(dir.path(), false, None, &bundle).unwrap();
        let installed =
            read_installed_template_version(dir.path()).expect("应已写入 .template-version");
        assert_eq!(
            installed, bundle.template_version,
            ".template-version 文件内容应等于 bundle.template_version",
        );
    }

    /// 各 IDE 的规则文件安装形态：
    /// - Cursor/Trae/Qoder/CodeBuddy：带 frontmatter 且 alwaysApply: true（否则规则不常驻生效）；
    ///   Cursor 用 .mdc，其余用 .md。
    /// - Claude Code：规则**没有 alwaysApply 字段**（官方仅 paths；省略 frontmatter 即全局加载），
    ///   直接落盘正文，不包裹 Cursor 式 frontmatter。
    #[test]
    fn rules_always_apply_frontmatter_per_ide() {
        for ide in IDE_TARGETS {
            // WorkBuddy 不安装门禁规则，跳过
            if !ide.supports_rules {
                continue;
            }
            let files = list_workflow_install_files(ide, &fixture_bundle());
            let rules_path = format!("{}/{}", ide.rules_dir, ide.rules_filename);
            let rules = files
                .iter()
                .find(|f| f.relative_path == rules_path)
                .expect("应包含规则文件");
            if ide.id == IdeId::ClaudeCode {
                assert!(
                    !rules.content.starts_with("---\n"),
                    "{} 规则不应包裹 frontmatter（省略即全局加载）",
                    ide.id,
                );
                assert!(
                    !rules.content.contains("alwaysApply"),
                    "{} 规则不应含 alwaysApply（那是 Cursor 的字段）",
                    ide.id,
                );
                assert!(
                    rules.content.starts_with("# 自主交付核心规则"),
                    "{} 规则应直接落盘正文",
                    ide.id,
                );
                continue;
            }
            assert!(
                rules.content.starts_with("---\n"),
                "{} 规则应带 frontmatter",
                ide.id,
            );
            assert!(
                rules.content.contains("alwaysApply: true"),
                "{} 规则必须 alwaysApply: true 才常驻",
                ide.id,
            );
        }
        assert!(get_ide_target("cursor").unwrap().rules_filename.ends_with(".mdc"));
        for id in ["trae", "qoder", "codebuddy"] {
            assert!(get_ide_target(id).unwrap().rules_filename.ends_with(".md"));
        }
        assert_eq!(
            get_ide_target("claude-code").unwrap().rules_filename,
            "chunsun-workflow-gates.md",
        );
    }

    /// WorkBuddy 只安装技能到 `.workbuddy/skills`，不安装斜线命令与门禁规则
    /// （WorkBuddy 不读取 `.workbuddy/commands` 与 `.workbuddy/rules`）；
    /// AGENTS.md 桥接只指向技能，不指向不存在的规则路径。
    #[test]
    fn installs_workbuddy_skills_only() {
        let dir = tempdir().unwrap();
        let wb = get_ide_target("workbuddy").expect("workbuddy 应在 IDE 列表中");
        install_skill_workspace(dir.path(), false, Some(wb), &fixture_bundle()).unwrap();

        // 技能（含引用）安装到位
        assert!(dir.path().join(".workbuddy/skills/chunsun/SKILL.md").is_file());
        assert!(dir
            .path()
            .join(".workbuddy/skills/chunsun/references/commands.md")
            .is_file());
        assert!(dir
            .path()
            .join(".workbuddy/skills/chunsun/references/loop-rules.md")
            .is_file());

        // 不安装斜线命令与门禁规则（WorkBuddy 不支持这两个目录）
        assert!(!dir.path().join(".workbuddy/commands/chunsun.md").exists());
        assert!(!dir
            .path()
            .join(".workbuddy/commands/chunsun-fix.md")
            .exists());
        assert!(!dir
            .path()
            .join(".workbuddy/rules/chunsun-workflow-gates.md")
            .exists());

        // AGENTS.md 桥接只指向技能，不指向不存在的规则路径
        let agents = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert!(agents.contains(".workbuddy/skills/chunsun/SKILL.md"));
        assert!(!agents.contains(".workbuddy/rules/chunsun-workflow-gates"));
    }

    /// AGENTS.md 桥接：创建、幂等、只替换 marker 内内容、无 marker 时追加。
    #[test]
    fn agents_bridge_created_merged_and_idempotent() {
        let dir = tempdir().unwrap();
        let wb = get_ide_target("codebuddy").expect("codebuddy 应在 IDE 列表中");
        install_skill_workspace(dir.path(), false, Some(wb), &fixture_bundle()).unwrap();
        let agents = dir.path().join("AGENTS.md");
        let content = fs::read_to_string(&agents).unwrap();
        assert!(content.contains(AGENTS_BRIDGE_BEGIN));
        assert!(content.contains(AGENTS_BRIDGE_END));
        assert!(content.contains(".codebuddy/rules/chunsun-workflow-gates.md"));

        // 幂等：再次安装内容不变，计入 reused
        let before = content.clone();
        let again = install_skill_workspace(dir.path(), true, Some(wb), &fixture_bundle()).unwrap();
        let after = fs::read_to_string(&agents).unwrap();
        assert_eq!(before, after, "重复安装不应改动 AGENTS.md");
        assert!(again.reused.iter().any(|p| p == "AGENTS.md"));

        // 用户在 marker 外有内容：只替换 marker 内段落
        let custom = format!(
            "# 项目说明\n\n用户自己的内容。\n\n{AGENTS_BRIDGE_BEGIN}\n段落内旧内容\n{AGENTS_BRIDGE_END}\n\n## 其它章节\n\n保留我。\n"
        );
        fs::write(&agents, &custom).unwrap();
        install_skill_workspace(dir.path(), true, Some(wb), &fixture_bundle()).unwrap();
        let merged = fs::read_to_string(&agents).unwrap();
        assert!(merged.contains("用户自己的内容。"), "marker 前内容应保留");
        assert!(merged.contains("保留我。"), "marker 后内容应保留");
        assert!(!merged.contains("段落内旧内容"), "marker 内旧段落应被替换");
        assert!(merged.contains(".codebuddy/skills/chunsun/SKILL.md"));

        // 无 marker 的既有 AGENTS.md：追加而非覆盖
        let dir2 = tempdir().unwrap();
        fs::write(dir2.path().join("AGENTS.md"), "# 已有 AGENTS\n\n别动我。\n").unwrap();
        install_skill_workspace(dir2.path(), false, Some(wb), &fixture_bundle()).unwrap();
        let appended = fs::read_to_string(dir2.path().join("AGENTS.md")).unwrap();
        assert!(appended.contains("别动我。"), "既有内容应保留");
        assert!(appended.contains(AGENTS_BRIDGE_BEGIN), "应追加桥接段落");
    }

    /// Claude Code 目标写入 `.claude/skills`、`.claude/commands`、`.claude/rules`：
    /// 规则不带 alwaysApply frontmatter（官方省略即全局加载）、斜线命令旧格式仍装、
    /// 额外在仓库根维护 CLAUDE.md 桥接（官方读 CLAUDE.md 而非 AGENTS.md）。
    #[test]
    fn installs_claude_code_target() {
        let dir = tempdir().unwrap();
        let cc = get_ide_target("claude-code").expect("claude-code 应在 IDE 列表中");
        install_skill_workspace(dir.path(), false, Some(cc), &fixture_bundle()).unwrap();

        // 技能（含引用）与版本文件
        assert!(dir.path().join(".claude/skills/chunsun/SKILL.md").is_file());
        assert!(dir
            .path()
            .join(".claude/skills/chunsun/references/commands.md")
            .is_file());
        assert!(dir
            .path()
            .join(".claude/skills/chunsun/references/loop-rules.md")
            .is_file());
        assert!(dir
            .path()
            .join(".claude/skills/chunsun/.template-version")
            .is_file());

        // 斜线命令（官方：.claude/commands/*.md 旧格式仍有效）
        assert!(dir.path().join(".claude/commands/chunsun.md").is_file());
        assert!(dir.path().join(".claude/commands/chunsun-fix.md").is_file());

        // 规则：无 frontmatter（Claude Code 无 alwaysApply，省略即全局加载）
        let rules =
            fs::read_to_string(dir.path().join(".claude/rules/chunsun-workflow-gates.md")).unwrap();
        assert!(!rules.starts_with("---\n"), "Claude Code 规则不应包裹 frontmatter");
        assert!(!rules.contains("alwaysApply"), "Claude Code 规则不应含 alwaysApply 字段");
        assert!(rules.starts_with("# 自主交付核心规则"), "规则正文直接落盘");

        // CLAUDE.md 桥接：指向 .claude 路径
        let claude = fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(claude.contains(AGENTS_BRIDGE_BEGIN));
        assert!(claude.contains(AGENTS_BRIDGE_END));
        assert!(claude.contains(".claude/skills/chunsun/SKILL.md"));
        assert!(claude.contains(".claude/rules/chunsun-workflow-gates.md"));

        // AGENTS.md 桥接仍写（保留给其它 IDE），引用 .claude 路径
        let agents = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert!(agents.contains(".claude/skills/chunsun/SKILL.md"));

        // 幂等：重复安装 CLAUDE.md 内容不变
        let before = claude.clone();
        let again = install_skill_workspace(dir.path(), true, Some(cc), &fixture_bundle()).unwrap();
        assert_eq!(
            before,
            fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap(),
            "重复安装不应改动 CLAUDE.md",
        );
        assert!(again.reused.iter().any(|p| p == "CLAUDE.md"));
    }

    /// Agents 目标写入 `.agents/skills`、`.agents/commands`、`.agents/rules`，
    /// 且不会被迁移清理误删（`.agents` 是合法安装目标而非遗留物）。
    #[test]
    fn installs_agents_target_and_skips_cleanup() {
        let dir = tempdir().unwrap();
        let agents = get_ide_target("agents").expect("agents 应在 IDE 列表中");
        install_skill_workspace(dir.path(), false, Some(agents), &fixture_bundle()).unwrap();

        // 技能（含引用）与版本文件
        assert!(dir.path().join(".agents/skills/chunsun/SKILL.md").is_file());
        assert!(dir
            .path()
            .join(".agents/skills/chunsun/references/commands.md")
            .is_file());
        assert!(dir
            .path()
            .join(".agents/skills/chunsun/references/loop-rules.md")
            .is_file());
        assert!(dir
            .path()
            .join(".agents/skills/chunsun/.template-version")
            .is_file());

        // 斜线命令与门禁规则
        assert!(dir.path().join(".agents/commands/chunsun.md").is_file());
        assert!(dir.path().join(".agents/commands/chunsun-fix.md").is_file());
        assert!(dir
            .path()
            .join(".agents/rules/chunsun-workflow-gates.md")
            .is_file());

        // AGENTS.md 桥接指向 .agents 路径
        let agents_md = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert!(agents_md.contains(".agents/skills/chunsun/SKILL.md"));
        assert!(agents_md.contains(".agents/rules/chunsun-workflow-gates.md"));
    }

    /// Claude Code 的 CLAUDE.md 桥接：用户内容在 marker 外保留、marker 内幂等替换。
    #[test]
    fn claude_md_bridge_preserves_user_content() {
        let dir = tempdir().unwrap();
        let cc = get_ide_target("claude-code").expect("claude-code 应在 IDE 列表中");
        let custom = format!(
            "# 我的项目\n\n技术栈说明。\n\n{AGENTS_BRIDGE_BEGIN}\n旧段落\n{AGENTS_BRIDGE_END}\n\n## 部署\n\n保留我。\n"
        );
        fs::write(dir.path().join("CLAUDE.md"), &custom).unwrap();
        install_skill_workspace(dir.path(), true, Some(cc), &fixture_bundle()).unwrap();
        let merged = fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(merged.contains("技术栈说明。"), "marker 前用户内容应保留");
        assert!(merged.contains("保留我。"), "marker 后用户内容应保留");
        assert!(!merged.contains("旧段落"), "marker 内旧段落应被替换");
        assert!(merged.contains(".claude/skills/chunsun/SKILL.md"));
    }
}
