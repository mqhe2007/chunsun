#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeId {
    Cursor,
    Trae,
    Qoder,
    CodeBuddy,
    /// WorkBuddy：仅读取 `.workbuddy/skills`，不支持 `.workbuddy/commands` 与 `.workbuddy/rules`。
    WorkBuddy,
    /// Claude Code（依据官方文档 code.claude.com/docs）：
    /// - skills 与斜线命令合并（`.claude/skills/<name>/SKILL.md` 即注册 `/name`），旧 `.claude/commands/*.md` 仍有效；
    /// - 规则目录 `.claude/rules/*.md`：**没有 `alwaysApply` 字段**（那是 Cursor 的），省略 `paths` frontmatter 即全局规则、启动即加载；
    /// - 常驻桥接写仓库根 `CLAUDE.md`（官方读 CLAUDE.md 而非 AGENTS.md）。
    ClaudeCode,
    /// Agents：通用 agent 无关目录 `.agents`（skills / commands / rules 与早期 chunsun 布局一致），
    /// 适用于希望把技能/斜线命令/门禁规则集中到 `.agents` 的通用 Agent 工作流。
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

// 各家 IDE 的规则 frontmatter 独立维护：目录/格式不通用，新增 IDE 时在此单独加配置。
// 注意：alwaysApply: true 必须开启，否则规则不常驻（不注入每次对话上下文）。
const RULES_FRONTMATTER_CURSOR: &str =
    "---\ndescription: 春笋自主交付核心规则——验收定义、停点与状态边界\nalwaysApply: true\n---\n";
const RULES_FRONTMATTER_TRAE: &str =
    "---\ndescription: 春笋自主交付核心规则——验收定义、停点与状态边界\nalwaysApply: true\n---\n";
const RULES_FRONTMATTER_QODER: &str =
    "---\ndescription: 春笋自主交付核心规则——验收定义、停点与状态边界\nalwaysApply: true\n---\n";
const RULES_FRONTMATTER_CODEBUDDY: &str =
    "---\ndescription: 春笋自主交付核心规则——验收定义、停点与状态边界\nalwaysApply: true\n---\n";
// Claude Code 规则**没有 alwaysApply 字段**（官方文档仅支持 paths；无 paths 即全局规则、启动时无条件加载）。
// 故 frontmatter 留空，规则正文直接落盘，不包裹 Cursor 式 frontmatter。
const RULES_FRONTMATTER_CLAUDE_CODE: &str = "";
// Agents：通用 `.agents` 目录，沿用 Cursor 式 alwaysApply frontmatter（多数 agent 工具按此约定加载常驻规则）。
const RULES_FRONTMATTER_AGENTS: &str =
    "---\ndescription: 春笋自主交付核心规则——验收定义、停点与状态边界\nalwaysApply: true\n---\n";

#[derive(Debug, Clone)]
pub struct IdeTarget {
    pub id: IdeId,
    pub label: &'static str,
    pub commands_dir: &'static str,
    pub rules_dir: &'static str,
    pub rules_filename: &'static str,
    /// 规则文件的 frontmatter（按 IDE 单独定义；必须含 alwaysApply: true 才常驻生效）。
    pub rules_frontmatter: &'static str,
    pub skills_dir: &'static str,
    /// 是否安装门禁规则。WorkBuddy 不读取 `.workbuddy/rules`，故为 false。
    pub supports_rules: bool,
    /// 是否安装斜线命令。WorkBuddy 不读取 `.workbuddy/commands`，故为 false。
    pub supports_commands: bool,
    /// 是否额外在仓库根维护 `CLAUDE.md` 桥接段落。仅 Claude Code 需要——
    /// 官方读取 CLAUDE.md 而非 AGENTS.md（AGENTS.md 对其无效，须经 CLAUDE.md 才进上下文）；
    /// 其余 IDE 以 AGENTS.md 桥接为准。
    pub writes_claude_md: bool,
}

pub const IDE_TARGETS: &[IdeTarget] = &[
    IdeTarget {
        id: IdeId::Cursor,
        label: "Cursor（.cursor/commands、.cursor/rules、.cursor/skills）",
        commands_dir: ".cursor/commands",
        rules_dir: ".cursor/rules",
        rules_filename: "chunsun-workflow-gates.mdc",
        rules_frontmatter: RULES_FRONTMATTER_CURSOR,
        skills_dir: ".cursor/skills",
        supports_rules: true,
        supports_commands: true,
        writes_claude_md: false,
    },
    IdeTarget {
        id: IdeId::Trae,
        label: "Trae（.trae/commands、.trae/rules、.trae/skills）",
        commands_dir: ".trae/commands",
        rules_dir: ".trae/rules",
        rules_filename: "chunsun-workflow-gates.md",
        rules_frontmatter: RULES_FRONTMATTER_TRAE,
        skills_dir: ".trae/skills",
        supports_rules: true,
        supports_commands: true,
        writes_claude_md: false,
    },
    IdeTarget {
        id: IdeId::Qoder,
        label: "Qoder（.qoder/commands、.qoder/rules、.qoder/skills）",
        commands_dir: ".qoder/commands",
        rules_dir: ".qoder/rules",
        rules_filename: "chunsun-workflow-gates.md",
        rules_frontmatter: RULES_FRONTMATTER_QODER,
        skills_dir: ".qoder/skills",
        supports_rules: true,
        supports_commands: true,
        writes_claude_md: false,
    },
    IdeTarget {
        id: IdeId::CodeBuddy,
        label: "CodeBuddy（.codebuddy/commands、.codebuddy/rules、.codebuddy/skills）",
        commands_dir: ".codebuddy/commands",
        rules_dir: ".codebuddy/rules",
        rules_filename: "chunsun-workflow-gates.md",
        rules_frontmatter: RULES_FRONTMATTER_CODEBUDDY,
        skills_dir: ".codebuddy/skills",
        supports_rules: true,
        supports_commands: true,
        writes_claude_md: false,
    },
    // WorkBuddy：只读取 `.workbuddy/skills`，不支持 commands / rules。
    // 因此只安装技能，斜线命令与门禁规则目录置空、能力开关关闭，不去补齐它不支持的部分。
    IdeTarget {
        id: IdeId::WorkBuddy,
        label: "WorkBuddy（.workbuddy/skills）",
        commands_dir: "",
        rules_dir: "",
        rules_filename: "",
        rules_frontmatter: "",
        skills_dir: ".workbuddy/skills",
        supports_rules: false,
        supports_commands: false,
        writes_claude_md: false,
    },
    // Claude Code：`.claude/skills`（skill 即注册同名斜线命令，`/chunsun` 由 skill 承担）、
    // `.claude/commands`（旧格式仍有效，`/chunsun-fix` 走命令文件）、`.claude/rules`（全局规则省略 frontmatter）。
    // 官方读 CLAUDE.md 而非 AGENTS.md，故额外维护仓库根 CLAUDE.md 桥接。
    IdeTarget {
        id: IdeId::ClaudeCode,
        label: "Claude Code（.claude/skills、.claude/commands、.claude/rules）",
        commands_dir: ".claude/commands",
        rules_dir: ".claude/rules",
        rules_filename: "chunsun-workflow-gates.md",
        rules_frontmatter: RULES_FRONTMATTER_CLAUDE_CODE,
        skills_dir: ".claude/skills",
        supports_rules: true,
        supports_commands: true,
        writes_claude_md: true,
    },
    // Agents：通用 agent 无关目录 `.agents`（skills/commands/rules 与早期 chunsun 布局一致），
    // 适用于希望把技能/斜线命令/门禁规则集中到 `.agents` 的通用 Agent 工作流。
    // 桥接仍写仓库根 AGENTS.md（AGENTS.md 是跨 IDE 常驻层，.agents 亦通用）。
    IdeTarget {
        id: IdeId::Agents,
        label: "Agents（.agents/skills、.agents/commands、.agents/rules）",
        commands_dir: ".agents/commands",
        rules_dir: ".agents/rules",
        rules_filename: "chunsun-workflow-gates.md",
        rules_frontmatter: RULES_FRONTMATTER_AGENTS,
        skills_dir: ".agents/skills",
        supports_rules: true,
        supports_commands: true,
        writes_claude_md: false,
    },
];

pub const DEFAULT_IDE_ID: IdeId = IdeId::Cursor;

pub fn get_ide_target(id: &str) -> Option<&'static IdeTarget> {
    IDE_TARGETS.iter().find(|t| t.id.as_str() == id)
}

pub fn default_ide_target() -> &'static IdeTarget {
    get_ide_target(DEFAULT_IDE_ID.as_str()).unwrap()
}
