use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::api::{ApiClient, ApiError};
use crate::ide::{default_ide_target, IdeId, IdeTarget, IDE_TARGETS};

/// 已安装模板的版本文件名（落在 `<ide>/skills/chunsun/`）。
pub const TEMPLATE_VERSION_FILENAME: &str = ".template-version";

/// 历史 AGENTS.md / CLAUDE.md 桥接段落的 marker。
/// 2026-09-08-skill-only-harness 起 harness 不再管理这两个文件；
/// marker 仅用于升级迁移时剥离旧桥接段落。
pub const AGENTS_BRIDGE_BEGIN: &str = "<!-- chunsun:begin -->";
pub const AGENTS_BRIDGE_END: &str = "<!-- chunsun:end -->";

/// 实例 `GET /harness/template` 返回的模板包（运行时真相源）。
///
/// 技能是唯一 harness 载体：斜线命令与常驻规则模板已并入技能模板，
/// payload 仅含 SKILL.md / commands.md / loop-rules.md 三个文件。
#[derive(Debug, Clone)]
pub struct HarnessTemplateBundle {
    pub template_version: String,
    pub skill: String,
    pub commands: String,
    pub loop_rules: String,
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
    })
}

#[derive(Debug)]
pub struct WorkflowInstallFile {
    pub relative_path: String,
    pub content: String,
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
    /// 本次安装迁移清理掉的旧 harness 产物（斜线命令 / 常驻规则 / 桥接段落）。
    pub cleaned: Vec<String>,
    pub refreshed: bool,
    pub previous_version: Option<String>,
    pub template_version: String,
    pub ide: IdeId,
}

/// 技能目录（`<ide>/skills/chunsun/`）是唯一安装面：
/// SKILL.md + references/commands.md + references/loop-rules.md + .template-version。
fn skill_install_files(ide: &IdeTarget, bundle: &HarnessTemplateBundle) -> Vec<WorkflowInstallFile> {
    vec![
        WorkflowInstallFile {
            relative_path: format!("{}/chunsun/SKILL.md", ide.skills_dir),
            content: bundle.skill.clone(),
        },
        WorkflowInstallFile {
            relative_path: format!("{}/chunsun/references/commands.md", ide.skills_dir),
            content: bundle.commands.clone(),
        },
        WorkflowInstallFile {
            relative_path: format!("{}/chunsun/references/loop-rules.md", ide.skills_dir),
            content: bundle.loop_rules.clone(),
        },
    ]
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
    let mut cleaned = Vec::new();
    let skill_root = cwd.join(ide.skills_dir).join("chunsun");
    let previous_version = read_installed_template_version(cwd);
    let refreshed =
        force || previous_version.as_deref() != Some(bundle.template_version.as_str());

    for file in &skill_install_files(ide, bundle) {
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

    // 迁移清理：历史版本装在各 IDE 目录下的斜线命令与常驻规则、
    // 旧 `.agents` 布局，以及 AGENTS.md / CLAUDE.md 桥接段落。
    // harness 不再管理这些文件，装完即清理（幂等：不存在则无操作）。
    cleanup_legacy_harness_files(cwd, ide, &mut cleaned);

    Ok(InstallSkillWorkspaceResult {
        skill_root,
        reused,
        written,
        cleaned,
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

/// 检测已安装的 IDE 目标：技能目录（`<ide>/skills/chunsun`）对每家 IDE 都会安装。
pub fn detect_installed_ide_targets(cwd: &Path) -> Vec<&'static IdeTarget> {
    IDE_TARGETS
        .iter()
        .filter(|t| path_exists(&cwd.join(t.skills_dir).join("chunsun")))
        .collect()
}

/// 从 markdown 内容中剥离全部 chunsun 桥接段落（含孤立 marker），返回剩余内容。
fn strip_bridge_sections(content: &str) -> String {
    let mut s = content.to_string();
    while let Some(begin) = s.find(AGENTS_BRIDGE_BEGIN) {
        match s[begin..].find(AGENTS_BRIDGE_END) {
            Some(rel_end) => {
                let end = begin + rel_end + AGENTS_BRIDGE_END.len();
                s.replace_range(begin..end, "");
            }
            None => {
                // 孤立 begin marker（损坏状态）：仅移除 marker 本身
                s.replace_range(begin..begin + AGENTS_BRIDGE_BEGIN.len(), "");
            }
        }
    }
    // 残留的孤立 end marker 一并移除
    s = s.replace(AGENTS_BRIDGE_END, "");
    // 收敛段落移除后留下的连续空行
    while s.contains("\n\n\n") {
        s = s.replace("\n\n\n", "\n\n");
    }
    s.trim_end().to_string() + "\n"
}

/// 仓库根 markdown（AGENTS.md / CLAUDE.md）桥接清理：
/// - 剥离 marker 段落后若仍有实质内容（除自动生成的标题外）→ 回写剩余内容；
/// - 只剩自动创建时的标题壳（`# AGENTS.md` / `# CLAUDE.md`）或全空 → 删除文件。
fn remove_bridge_file(cwd: &Path, filename: &str, cleaned: &mut Vec<String>) {
    let path = cwd.join(filename);
    let Ok(existing) = fs::read_to_string(&path) else {
        return;
    };
    if !existing.contains(AGENTS_BRIDGE_BEGIN) && !existing.contains(AGENTS_BRIDGE_END) {
        return;
    }
    let stripped = strip_bridge_sections(&existing);
    let remainder = stripped.trim();
    let header_only = remainder == format!("# {filename}");
    if remainder.is_empty() || header_only {
        // 文件本就是 chunsun 自动创建的空壳，直接删除
        let _ = fs::remove_file(&path);
    } else {
        let _ = fs::write(&path, stripped);
    }
    cleaned.push(filename.to_string());
}

fn remove_dir_if_empty(dir: &Path) {
    if dir.exists() {
        let is_empty = fs::read_dir(dir)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if is_empty {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

/// 移除历史版本装在各 IDE 目录下的 chunsun 自有斜线命令与常驻规则文件。
///
/// 覆盖全部 IDE 目标（含 `.agents`）：这些文件不再安装，均为遗留产物；
/// 同目录下非 chunsun 文件不动，目录清空后移除空目录。
fn cleanup_legacy_commands_and_rules(cwd: &Path, cleaned: &mut Vec<String>) {
    for target in IDE_TARGETS {
        if !target.commands_dir.is_empty() {
            let dir = cwd.join(target.commands_dir);
            if dir.exists() {
                for name in ["chunsun.md", "chunsun-fix.md"] {
                    let p = dir.join(name);
                    if p.exists() {
                        let _ = fs::remove_file(&p);
                        cleaned.push(format!("{}/{}", target.commands_dir, name));
                    }
                }
                remove_dir_if_empty(&dir);
            }
        }
        if !target.rules_dir.is_empty() {
            let dir = cwd.join(target.rules_dir);
            if dir.exists() {
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let s = name.to_string_lossy();
                        if s.starts_with("chunsun-workflow-gates") {
                            let _ = fs::remove_file(entry.path());
                            cleaned.push(format!("{}/{}", target.rules_dir, s));
                        }
                    }
                }
                remove_dir_if_empty(&dir);
            }
        }
    }
}

/// 移除旧版 `.agents` 布局下 chunsun 自有的技能目录（迁移到 IDE 专属 skills 目录后不再维护）。
///
/// 仅当安装目标不是 `.agents` 时清理（目标即 Agents 时为合法安装产物）；
/// `.agents` 下的 commands / rules 由 `cleanup_legacy_commands_and_rules` 统一处理。
fn cleanup_legacy_agents_skills(cwd: &Path, cleaned: &mut Vec<String>) {
    let legacy_skill = cwd.join(".agents").join("skills").join("chunsun");
    if legacy_skill.exists() {
        let _ = fs::remove_dir_all(&legacy_skill);
        cleaned.push(".agents/skills/chunsun".to_string());
    }
    remove_dir_if_empty(&cwd.join(".agents").join("skills"));
    remove_dir_if_empty(&cwd.join(".agents"));
}

/// 迁移清理总入口：历史斜线命令 / 常驻规则 / `.agents` 技能旧布局 / AGENTS.md·CLAUDE.md 桥接。
pub fn cleanup_legacy_harness_files(cwd: &Path, ide: &IdeTarget, cleaned: &mut Vec<String>) {
    cleanup_legacy_commands_and_rules(cwd, cleaned);
    if ide.id != IdeId::Agents {
        cleanup_legacy_agents_skills(cwd, cleaned);
    }
    remove_bridge_file(cwd, "AGENTS.md", cleaned);
    remove_bridge_file(cwd, "CLAUDE.md", cleaned);
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

    /// 技能单点安装：所有 IDE 目标只落盘技能目录，不写斜线命令、常驻规则，
    /// 也不创建/修改仓库根 AGENTS.md 与 CLAUDE.md。
    #[test]
    fn skill_is_the_only_install_surface() {
        for ide in IDE_TARGETS {
            let dir = tempdir().unwrap();
            install_skill_workspace(dir.path(), false, Some(ide), &fixture_bundle()).unwrap();

            // 技能本体 + 引用 + 版本文件
            assert!(
                dir.path().join(format!("{}/chunsun/SKILL.md", ide.skills_dir)).is_file(),
                "{} 应安装 SKILL.md",
                ide.id,
            );
            assert!(dir
                .path()
                .join(format!("{}/chunsun/references/commands.md", ide.skills_dir))
                .is_file());
            assert!(dir
                .path()
                .join(format!("{}/chunsun/references/loop-rules.md", ide.skills_dir))
                .is_file());
            assert!(dir
                .path()
                .join(format!("{}/chunsun/{}", ide.skills_dir, TEMPLATE_VERSION_FILENAME))
                .is_file());

            // 不安装斜线命令与常驻规则
            if !ide.commands_dir.is_empty() {
                assert!(
                    !dir.path().join(format!("{}/chunsun.md", ide.commands_dir)).exists(),
                    "{} 不应安装斜线命令",
                    ide.id,
                );
                assert!(
                    !dir.path().join(format!("{}/chunsun-fix.md", ide.commands_dir)).exists(),
                    "{} 不应安装斜线命令",
                    ide.id,
                );
            }
            if !ide.rules_dir.is_empty() {
                assert!(
                    !dir.path().join(format!("{}/{}", ide.rules_dir, ide.rules_filename)).exists(),
                    "{} 不应安装常驻规则",
                    ide.id,
                );
            }

            // 不管理仓库根 AGENTS.md / CLAUDE.md
            assert!(
                !dir.path().join("AGENTS.md").exists(),
                "{} 不应创建 AGENTS.md",
                ide.id,
            );
            assert!(
                !dir.path().join("CLAUDE.md").exists(),
                "{} 不应创建 CLAUDE.md",
                ide.id,
            );
        }
    }

    /// 旧命令不再落盘（历史硬切回归）。
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
        assert!(!dir.path().join(".cursor/commands/chunsun.md").exists());
        assert!(!dir.path().join(".cursor/commands/chunsun-fix.md").exists());
        assert!(
            !dir.path().join(".cursor/skills/chunsun/references/门禁.md").exists(),
            "门禁引用不应再安装",
        );
        assert!(
            dir.path().join(".cursor/skills/chunsun/references/loop-rules.md").is_file(),
            "核心规则应作为技能引用安装",
        );
    }

    /// 技能从 .agents 迁移到所选 IDE 的 skills 目录，并清理旧 .agents 布局
    /// （skills + commands + rules 全部为遗留产物）。
    #[test]
    fn migrates_legacy_agents_to_ide_skills() {
        let dir = tempdir().unwrap();
        // 模拟旧版布局：.agents/ 下技能、斜线命令、门禁规则俱全
        let legacy_root = dir.path().join(".agents/skills/chunsun");
        fs::create_dir_all(legacy_root.join("references")).unwrap();
        fs::write(legacy_root.join("SKILL.md"), "legacy skill").unwrap();
        fs::write(
            legacy_root.join(TEMPLATE_VERSION_FILENAME),
            "2026-08-06-harness-long-loop\n",
        )
        .unwrap();
        let legacy_commands = dir.path().join(".agents/commands");
        fs::create_dir_all(&legacy_commands).unwrap();
        fs::write(legacy_commands.join("chunsun.md"), "legacy cmd").unwrap();
        fs::write(legacy_commands.join("chunsun-fix.md"), "legacy fix").unwrap();
        let legacy_rules = dir.path().join(".agents/rules");
        fs::create_dir_all(&legacy_rules).unwrap();
        fs::write(
            legacy_rules.join("chunsun-workflow-gates.md"),
            "legacy gates",
        )
        .unwrap();

        // 以 Cursor 为目标重新初始化
        install_skill_workspace(dir.path(), false, Some(default_ide_target()), &fixture_bundle())
            .unwrap();

        // 新位置已写入，旧 .agents 三处 chunsun 产物均被清理
        assert!(dir.path().join(".cursor/skills/chunsun/SKILL.md").is_file());
        assert!(dir.path().join(".cursor/skills/chunsun/.template-version").is_file());
        assert!(!legacy_root.exists(), "旧 .agents/skills/chunsun 应被移除");
        assert!(!legacy_commands.join("chunsun.md").exists());
        assert!(!legacy_commands.join("chunsun-fix.md").exists());
        assert!(!legacy_rules.join("chunsun-workflow-gates.md").exists());
        assert!(!dir.path().join(".agents").exists(), ".agents 清空后应整体移除");
        assert_eq!(
            read_installed_template_version(dir.path()),
            Some(fixture_version())
        );
    }

    /// 升级迁移：历史版本装在各 IDE 目录下的斜线命令 / 常驻规则被清理；
    /// 同目录非 chunsun 文件保留；AGENTS.md / CLAUDE.md 桥接段落剥离、用户内容保留。
    #[test]
    fn legacy_cleanup_removes_commands_rules_and_bridges() {
        let dir = tempdir().unwrap();
        let wb = get_ide_target("codebuddy").unwrap();

        // 旧版产物：codebuddy 与 cursor 两套命令 + 规则
        for (commands_dir, rules_dir, rules_name) in [
            (".codebuddy/commands", ".codebuddy/rules", "chunsun-workflow-gates.md"),
            (".cursor/commands", ".cursor/rules", "chunsun-workflow-gates.mdc"),
        ] {
            fs::create_dir_all(dir.path().join(commands_dir)).unwrap();
            fs::write(dir.path().join(format!("{commands_dir}/chunsun.md")), "old").unwrap();
            fs::write(dir.path().join(format!("{commands_dir}/chunsun-fix.md")), "old").unwrap();
            fs::write(dir.path().join(format!("{commands_dir}/other-tool.md")), "keep").unwrap();
            fs::create_dir_all(dir.path().join(rules_dir)).unwrap();
            fs::write(dir.path().join(format!("{rules_dir}/{rules_name}")), "old").unwrap();
        }

        // AGENTS.md / CLAUDE.md 带桥接段落与用户内容
        fs::write(
            dir.path().join("AGENTS.md"),
            format!(
                "# 项目说明\n\n用户自己的内容。\n\n{AGENTS_BRIDGE_BEGIN}\n## 春笋（chunsun）\n旧桥接段落。\n{AGENTS_BRIDGE_END}\n\n## 其它章节\n\n保留我。\n"
            ),
        )
        .unwrap();
        fs::write(
            dir.path().join("CLAUDE.md"),
            format!(
                "# 我的项目\n\n技术栈说明。\n\n{AGENTS_BRIDGE_BEGIN}\n旧段落\n{AGENTS_BRIDGE_END}\n\n## 部署\n\n保留我。\n"
            ),
        )
        .unwrap();

        let result = install_skill_workspace(dir.path(), false, Some(wb), &fixture_bundle()).unwrap();

        // 斜线命令与规则被清理；非 chunsun 文件保留
        for (commands_dir, rules_dir, rules_name) in [
            (".codebuddy/commands", ".codebuddy/rules", "chunsun-workflow-gates.md"),
            (".cursor/commands", ".cursor/rules", "chunsun-workflow-gates.mdc"),
        ] {
            assert!(!dir.path().join(format!("{commands_dir}/chunsun.md")).exists());
            assert!(!dir.path().join(format!("{commands_dir}/chunsun-fix.md")).exists());
            assert!(dir.path().join(format!("{commands_dir}/other-tool.md")).exists());
            assert!(!dir.path().join(format!("{rules_dir}/{rules_name}")).exists());
            // 规则目录清空后移除；命令目录因含非 chunsun 文件保留
            assert!(!dir.path().join(rules_dir).exists());
            assert!(dir.path().join(commands_dir).exists());
        }

        // 桥接段落剥离，marker 前后用户内容保留
        let agents = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert!(!agents.contains(AGENTS_BRIDGE_BEGIN));
        assert!(!agents.contains(AGENTS_BRIDGE_END));
        assert!(agents.contains("用户自己的内容。"));
        assert!(agents.contains("保留我。"));
        assert!(!agents.contains("旧桥接段落。"));
        let claude = fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(claude.contains("技术栈说明。"));
        assert!(claude.contains("保留我。"));
        assert!(!claude.contains(AGENTS_BRIDGE_BEGIN));

        // 清理留痕
        assert!(result.cleaned.iter().any(|p| p == "AGENTS.md"));
        assert!(result.cleaned.iter().any(|p| p == "CLAUDE.md"));
        assert!(result
            .cleaned
            .iter()
            .any(|p| p == ".codebuddy/commands/chunsun.md"));
        assert!(result
            .cleaned
            .iter()
            .any(|p| p == ".cursor/rules/chunsun-workflow-gates.mdc"));
    }

    /// 桥接清理：文件只剩 chunsun 自动创建的标题壳时，升级后整个文件删除。
    #[test]
    fn bridge_only_file_is_deleted_after_cleanup() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("AGENTS.md"),
            format!("# AGENTS.md\n\n{AGENTS_BRIDGE_BEGIN}\n## 春笋\n旧段落\n{AGENTS_BRIDGE_END}\n"),
        )
        .unwrap();
        install_skill_workspace(dir.path(), false, None, &fixture_bundle()).unwrap();
        assert!(
            !dir.path().join("AGENTS.md").exists(),
            "只剩自动创建壳的 AGENTS.md 应被删除",
        );
    }

    /// Agents 目标：技能装在 `.agents/skills`（合法目标，不清理），
    /// 但 `.agents/commands` / `.agents/rules` 下的 chunsun 旧产物仍被清理。
    #[test]
    fn agents_target_keeps_skills_but_cleans_commands_and_rules() {
        let dir = tempdir().unwrap();
        let agents = get_ide_target("agents").unwrap();

        // 预置旧产物（模拟更早版本装在 .agents 下）
        let legacy_commands = dir.path().join(".agents/commands");
        fs::create_dir_all(&legacy_commands).unwrap();
        fs::write(legacy_commands.join("chunsun.md"), "legacy cmd").unwrap();
        let legacy_rules = dir.path().join(".agents/rules");
        fs::create_dir_all(&legacy_rules).unwrap();
        fs::write(
            legacy_rules.join("chunsun-workflow-gates.md"),
            "legacy gates",
        )
        .unwrap();

        install_skill_workspace(dir.path(), false, Some(agents), &fixture_bundle()).unwrap();

        // 技能本体保留（.agents 是本次安装目标）
        assert!(dir.path().join(".agents/skills/chunsun/SKILL.md").is_file());
        assert!(dir
            .path()
            .join(".agents/skills/chunsun/references/loop-rules.md")
            .is_file());
        assert!(dir
            .path()
            .join(".agents/skills/chunsun/.template-version")
            .is_file());
        // 旧命令与规则仍被清理（不再安装）
        assert!(!legacy_commands.join("chunsun.md").exists());
        assert!(!legacy_rules.join("chunsun-workflow-gates.md").exists());
    }

    /// 夹具版本须与 backend templates/VERSION 一致（生产走实例接口，不再内嵌常量）。
    #[test]
    fn fixture_version_matches_backend_ssot() {
        assert_eq!(
            fixture_bundle().template_version,
            "2026-09-09-memory-writing-rules",
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

    /// WorkBuddy 只安装技能到 `.workbuddy/skills`（历史上也未装过命令/规则）。
    #[test]
    fn installs_workbuddy_skills_only() {
        let dir = tempdir().unwrap();
        let wb = get_ide_target("workbuddy").expect("workbuddy 应在 IDE 列表中");
        install_skill_workspace(dir.path(), false, Some(wb), &fixture_bundle()).unwrap();

        assert!(dir.path().join(".workbuddy/skills/chunsun/SKILL.md").is_file());
        assert!(dir
            .path()
            .join(".workbuddy/skills/chunsun/references/commands.md")
            .is_file());
        assert!(dir
            .path()
            .join(".workbuddy/skills/chunsun/references/loop-rules.md")
            .is_file());
        assert!(!dir.path().join(".workbuddy/commands").exists());
        assert!(!dir.path().join(".workbuddy/rules").exists());
    }

    /// Claude Code 目标：技能装 `.claude/skills`，旧 commands/rules 产物清理，
    /// CLAUDE.md 桥接剥离。
    #[test]
    fn installs_claude_code_target() {
        let dir = tempdir().unwrap();
        let cc = get_ide_target("claude-code").expect("claude-code 应在 IDE 列表中");

        // 预置旧版产物
        fs::create_dir_all(dir.path().join(".claude/commands")).unwrap();
        fs::write(dir.path().join(".claude/commands/chunsun.md"), "old").unwrap();
        fs::write(dir.path().join(".claude/commands/chunsun-fix.md"), "old").unwrap();
        fs::create_dir_all(dir.path().join(".claude/rules")).unwrap();
        fs::write(
            dir.path().join(".claude/rules/chunsun-workflow-gates.md"),
            "old gates",
        )
        .unwrap();
        fs::write(
            dir.path().join("CLAUDE.md"),
            format!(
                "# 我的项目\n\n技术栈说明。\n\n{AGENTS_BRIDGE_BEGIN}\n旧段落\n{AGENTS_BRIDGE_END}\n\n## 部署\n\n保留我。\n"
            ),
        )
        .unwrap();

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

        // 旧命令与规则被清理
        assert!(!dir.path().join(".claude/commands/chunsun.md").exists());
        assert!(!dir.path().join(".claude/commands/chunsun-fix.md").exists());
        assert!(!dir.path().join(".claude/rules/chunsun-workflow-gates.md").exists());
        assert!(!dir.path().join(".claude/commands").exists());
        assert!(!dir.path().join(".claude/rules").exists());

        // CLAUDE.md 桥接剥离、用户内容保留
        let claude = fs::read_to_string(dir.path().join("CLAUDE.md")).unwrap();
        assert!(claude.contains("技术栈说明。"));
        assert!(claude.contains("保留我。"));
        assert!(!claude.contains(AGENTS_BRIDGE_BEGIN));
        assert!(!claude.contains("旧段落"));
    }

    /// 孤立 marker（损坏状态）也能安全剥离。
    #[test]
    fn strip_bridge_handles_orphan_markers() {
        let content = format!(
            "# 项目\n\n正文。\n\n{AGENTS_BRIDGE_BEGIN}\n只有开头没有结尾\n\n另一段。\n"
        );
        let stripped = strip_bridge_sections(&content);
        assert!(!stripped.contains(AGENTS_BRIDGE_BEGIN));
        assert!(stripped.contains("正文。"));
        assert!(stripped.contains("另一段。"));
    }

    /// 无桥接段落的 AGENTS.md / CLAUDE.md 完全不动。
    #[test]
    fn untouched_md_files_without_bridge() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# 已有 AGENTS\n\n别动我。\n").unwrap();
        install_skill_workspace(dir.path(), false, None, &fixture_bundle()).unwrap();
        let content = fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
        assert_eq!(content, "# 已有 AGENTS\n\n别动我。\n");
    }
}
