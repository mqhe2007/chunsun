#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeId {
    Cursor,
    Trae,
    Qoder,
    CodeBuddy,
    /// WorkBuddy：仅读取 `.workbuddy/skills`。
    WorkBuddy,
    /// Claude Code（依据官方文档 code.claude.com/docs）：
    /// `.claude/skills/<name>/SKILL.md` 即注册同名技能（可 "/" 调出）。
    ClaudeCode,
    /// Agents：通用 agent 无关目录 `.agents`（技能落在 `.agents/skills`），
    /// 适用于希望把技能集中到 `.agents` 的通用 Agent 工作流。
    Agents,
}

impl IdeId {
    pub fn as_str(self) -> &'static str {
        match self {
            IdeId::Cursor => "cursor",
            IdeId::Trae => "trae",
            IdeId::Qoder => "qoder",
            IdeId::CodeBuddy => "codebuddy",
            IdeId::WorkBuddy => "workbuddy",
            IdeId::ClaudeCode => "claude-code",
            IdeId::Agents => "agents",
        }
    }
}

impl std::fmt::Display for IdeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 2026-09-08-skill-only-harness：技能是唯一 harness 载体——不再安装
/// 斜线命令（`<ide>/commands`）与常驻规则（`<ide>/rules`）文件。
/// `commands_dir` / `rules_dir` / `rules_filename` 仅为**迁移清理**保留：
/// 升级时用于定位并删除历史版本装下的 chunsun 自有文件。
#[derive(Debug, Clone)]
pub struct IdeTarget {
    pub id: IdeId,
    pub label: &'static str,
    /// 技能安装目录（唯一安装面）。WorkBuddy 为 `.workbuddy/skills`。
    pub skills_dir: &'static str,
    /// 历史斜线命令目录（仅迁移清理用；WorkBuddy 从未安装，为空）。
    pub commands_dir: &'static str,
    /// 历史常驻规则目录与文件名（仅迁移清理用；WorkBuddy 从未安装，为空）。
    pub rules_dir: &'static str,
    pub rules_filename: &'static str,
}

pub const IDE_TARGETS: &[IdeTarget] = &[
    IdeTarget {
        id: IdeId::Cursor,
        label: "Cursor（.cursor/skills）",
        commands_dir: ".cursor/commands",
        rules_dir: ".cursor/rules",
        rules_filename: "chunsun-workflow-gates.mdc",
        skills_dir: ".cursor/skills",
    },
    IdeTarget {
        id: IdeId::Trae,
        label: "Trae（.trae/skills）",
        commands_dir: ".trae/commands",
        rules_dir: ".trae/rules",
        rules_filename: "chunsun-workflow-gates.md",
        skills_dir: ".trae/skills",
    },
    IdeTarget {
        id: IdeId::Qoder,
        label: "Qoder（.qoder/skills）",
        commands_dir: ".qoder/commands",
        rules_dir: ".qoder/rules",
        rules_filename: "chunsun-workflow-gates.md",
        skills_dir: ".qoder/skills",
    },
    IdeTarget {
        id: IdeId::CodeBuddy,
        label: "CodeBuddy（.codebuddy/skills）",
        commands_dir: ".codebuddy/commands",
        rules_dir: ".codebuddy/rules",
        rules_filename: "chunsun-workflow-gates.md",
        skills_dir: ".codebuddy/skills",
    },
    // WorkBuddy：只读取 `.workbuddy/skills`，历史上也未安装过斜线命令与常驻规则。
    IdeTarget {
        id: IdeId::WorkBuddy,
        label: "WorkBuddy（.workbuddy/skills）",
        commands_dir: "",
        rules_dir: "",
        rules_filename: "",
        skills_dir: ".workbuddy/skills",
    },
    // Claude Code：`.claude/skills/<name>/SKILL.md` 即注册同名技能（"/" 调出）。
    IdeTarget {
        id: IdeId::ClaudeCode,
        label: "Claude Code（.claude/skills）",
        commands_dir: ".claude/commands",
        rules_dir: ".claude/rules",
        rules_filename: "chunsun-workflow-gates.md",
        skills_dir: ".claude/skills",
    },
    // Agents：通用 `.agents` 目录，技能落在 `.agents/skills/chunsun/`。
    IdeTarget {
        id: IdeId::Agents,
        label: "Agents（.agents/skills）",
        commands_dir: ".agents/commands",
        rules_dir: ".agents/rules",
        rules_filename: "chunsun-workflow-gates.md",
        skills_dir: ".agents/skills",
    },
];

pub const DEFAULT_IDE_ID: IdeId = IdeId::Cursor;

pub fn get_ide_target(id: &str) -> Option<&'static IdeTarget> {
    IDE_TARGETS.iter().find(|t| t.id.as_str() == id)
}

pub fn default_ide_target() -> &'static IdeTarget {
    get_ide_target(DEFAULT_IDE_ID.as_str()).unwrap()
}
