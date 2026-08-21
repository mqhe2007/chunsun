# Prompt templates MANIFEST

**Location (SSOT):** `packages/backend/templates/`（实例 `GET /api/v1/harness/template` 与 CLI `include_str!` 共用）。
版本号见同目录 `VERSION`（须与 backend / CLI 的 `TEMPLATE_VERSION` 一致）。

Historically extracted from TypeScript string-template prompts under `packages/cli/src_ts/prompts/`
(formerly `packages/cli/src/prompts/`) for Rust `include_str!` consumption.

> Note: On-disk TypeScript sources currently live in `src_ts/` after the CLI Rust migration;
> git historically tracked them as `src/prompts/`.
>
> 2026-08-06 harness 硬切后：阶段/门禁/排期模板全部移除，仅保留自主交付协议模板。
>
> 2026-08-21：模板从 `packages/cli/templates/` 迁到本目录；新增实例下载端点。
> 2026-08-21（续）：CLI `init` / `update` 改为运行时 `GET /harness/template` 拉取，不再 `include_str!` 内嵌正文。
> 2026-08-21-finished-stop：技能停点 CLI 从废弃的 `paused` 改为 `finished`（与 run-status-v2 对齐）。
> 2026-08-21-host-dual-mode：Pre-flight 前增加宿主选择——存在 `chunsun_*` Agent 工具则走工具直连，否则走 CLI；斜线命令 / commands / loop-rules 加宿主指针与边界措辞对齐。
>
> 2026-08-06-ide-skills：技能本体（SKILL.md + references）从 `.agents/skills/chunsun/` 迁到所选
> IDE 的 `<ide>/skills/chunsun/`（Cursor/Trae/Qoder/CodeBuddy 分别落在 `.cursor`/`.trae`/`.qoder`/`.codebuddy`）；
> 不再维护 `.agents`。`init` 按交互/ `--ide` 选择目标 IDE。
>
> 2026-08-07-rules-agents-bridge：四家 IDE 的规则文件全部包裹含 `alwaysApply: true` 的 frontmatter
> （此前仅 Cursor 包裹，Trae/Qoder/CodeBuddy 裸 .md 不常驻生效）；各 IDE frontmatter 在 `src/ide.rs`
> 独立维护，新增 IDE 单独加配置。新增仓库根 `AGENTS.md` 桥接段落（marker 管理、幂等）作跨 IDE 常驻层双保险；
> `skill.md` 正文的验收定义/停点/三层边界收敛为对 `references/loop-rules.md` 的引用（引用而非复制）。
>
> 2026-08-13-claude-code：新增 ClaudeCode 目标（`.claude/skills|commands|rules`）。Claude Code 与 Cursor 有
> 两处关键差异：① 规则**没有 `alwaysApply` 字段**——官方文档仅支持 `paths`，省略 frontmatter 即全局规则、
> 启动时无条件加载，故 Claude Code 的规则文件不包裹 frontmatter，直接落盘正文（`RULES_FRONTMATTER_CLAUDE_CODE` 为空）；
> ② 官方读取仓库根 `CLAUDE.md` 而非 AGENTS.md，故 ClaudeCode 额外维护 `CLAUDE.md` 桥接（同一 marker 语义、幂等，
> 由 `IdeTarget::writes_claude_md` 门控），AGENTS.md 桥接仍写以保留给其它 IDE。斜线命令仍装 `.claude/commands/*.md`
> （官方确认旧格式有效）；同名 skill 优先，`/chunsun` 由 `.claude/skills/chunsun/SKILL.md` 承担（内容更完整）。

| Template file | TS export / source | Install path (when relevant) |
| --- | --- | --- |
| `skill.md` | `SKILL_CONTENT` from `skill.ts` | `<ide.skillsDir>/chunsun/SKILL.md`（按 `init` 所选 IDE） |
| `commands.md` | `COMMANDS_CONTENT` from `commands.ts` | `<ide.skillsDir>/chunsun/references/commands.md` |
| `loop-rules.md` | Body of `LOOP_RULES_RULE` from `workflow/loopRules.ts` **without** Cursor YAML frontmatter | `<ide.skillsDir>/chunsun/references/loop-rules.md`（plain）；另按 IDE 包裹 frontmatter 装到 `{ide.rulesDir}/{ide.rulesFilename}`（Cursor/Trae/Qoder/CodeBuddy 含 `alwaysApply: true`，Cursor 为 `.mdc`、其余 `.md`；**Claude Code 例外**：无 frontmatter，省略即全局加载） |
| `slash/chunsun.md` | `SLASH_COMMANDS["chunsun.md"]` from `workflow/slashCommands.ts` | `{ide.commandsDir}/chunsun.md` |
| `slash/chunsun-fix.md` | `SLASH_COMMANDS["chunsun-fix.md"]` from `workflow/slashCommands.ts` | `{ide.commandsDir}/chunsun-fix.md` |
| （Rust 生成，非模板文件） | `agents_bridge_section` in `harness.rs` | 仓库根 `AGENTS.md`（全部 IDE）与 `CLAUDE.md`（仅 ClaudeCode）的 `<!-- chunsun:begin/end -->` 段落：marker 内整体替换，marker 外不动；无 marker 追加，无文件创建 |

## Removed in harness hard-cut (2026-08-06)

| Removed template | Reason |
| --- | --- |
| `stages/*`（11 个阶段） | 阶段漏斗取消，自主交付无阶段 |
| `slash/探索.md` 等 11 个旧命令 | 命令集收敛为 `/chunsun` `/chunsun-fix` |
| `gates.md` | 门禁矩阵取消（平台不再管阶段流转） |
| `schedule-confirm.md` | 排期取消 |

## Not extracted as standalone files

| TS export | Reason |
| --- | --- |
| `LOOP_RULES_RULE` (full, with frontmatter) | Full Cursor `.mdc` form kept only in TS; `loop-rules.md` is the plain body. Rust may re-wrap frontmatter per IDE. |
| `listWorkflowInstallFiles` / index re-exports | Orchestration only, not prompt bodies. |

## Naming notes

- `loop-rules.md` 无 frontmatter（仅正文）；Rust 端按 IDE 包裹各自的 alwaysApply frontmatter
  （Cursor/Trae/Qoder/CodeBuddy 需要，否则规则不常驻生效；Claude Code 例外——无 alwaysApply 字段，省略 frontmatter 即全局加载）。
- 中文文件名仅存在于历史版本；harness 硬切后斜线命令统一英文名（`chunsun.md` / `chunsun-fix.md`）。
