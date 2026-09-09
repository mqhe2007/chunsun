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
> 2026-08-27-knowledge-load-strategy：context→knowledge/memory 重命名落地到技能协议；知识文档增加 eager/lazy 加载策略与 knowledge index/doc 按需拉取。
> 2026-09-02-dependency-scheduling：Agent 依赖感知与调度落地——交付协议新增「执行前依赖检查 / 调度决策 / 完成后解锁」三步；技能新增「依赖调度」节；commands.md 新增 `chunsun dependency list|schedule|blocked|unlock` 命令参考（后端新增对应调度分析端点）。
> 2026-09-08-skill-only-harness：**斜线命令模板与规则模板并入技能模板**——技能成为唯一 harness 载体：
> ①删除 `slash/chunsun.md`、`slash/chunsun-fix.md`，/chunsun-fix 派生流程并入 skill.md「缺陷修复派生」节；
> ②skill.md 新增「意图路由」节（自然语言 / "/" 调出均由技能分析意图路由：需求交付 / 缺陷修复 / 查询 / 重来 / 豁免）；
> ③核心规则不再安装为各 IDE 常驻规则文件（`<ide>/rules/chunsun-workflow-gates.*`），改为技能激活期间恒生效（启动必读 `references/loop-rules.md`）；
> ④CLI 不再安装 `<ide>/commands`、`<ide>/rules` 下任何文件，并在升级时迁移清理旧产物与 AGENTS.md/CLAUDE.md 桥接段落；
> ⑤知识库渐进式加载机制（eager/lazy + index/doc 按需拉取）不受影响。
> 旧 CLI（≤0.5.1）对新实例 `init` 会因 payload 缺 slash 文件报错，经 `chunsun update` 升级二进制后自动收敛。
>
> 2026-09-08-memory-overhaul：工作记忆机制彻底优化（缺陷 dutztumK-Sjz）——
> ①skill.md「Memory」节新增「写入时机（强制）」：边界澄清/待决策/关键实现随手记、每轮收尾前必写 lastRunSummary、
> 「声称即写入」（只在回复里声称写入等同没写）；交付协议收尾步骤补「先写 lastRunSummary 再迁移 Run 状态」；
> ②loop-rules.md 新增「Memory（工作记忆）」节（声称即写入 / 收尾前必写 / 20k 上限与 SNAPSHOT_REQUIRED 拒绝）；
> ③配套实现：CLI `run remind` 新增「本轮未写记忆」柔性提醒；后端 PUT memory 缺 snapshot 字段 400、超 20k 字符 400；
> ④console 需求详情「工作记忆」面板完整渲染五字段（此前只显示 openDecisions+codeLandmarks，是「记忆没生效」症状的展示层根因）。
>
> 2026-09-09-memory-markdown：工作记忆从 JSON 五字段改为自由 Markdown 文本——
> ①snapshot 列从 JSONB 改为 TEXT（允许 NULL），后端 PUT 校验改为字符串长度 ≤20k；
> ②去掉 openDecisions 通知触发与 completed 硬门禁中的 openDecisions 检查（决策由 AI 在会话中主动提出）；
> ③CLI `memory put` 从增量合并改为全量覆盖（拉取-修改-保存全流程），去掉 merge_snapshot；
> ④console 面板从五区块结构化渲染改为 markdown-it 单区块渲染（XSS 防护：html:false）；
> ⑤skill.md「Memory」节重写为 Markdown 模板（需求边界/待决策/代码标记/环境变量/阻塞原因/本轮总结），loop-rules.md 同步更新。
>
> 2026-09-09-project-memory：新增项目级记忆机制（需求 aKek1T5nctZQ）——
> ①后端新增 `project_memory` 表（1:1 每项目一份）+ `GET/PUT /projects/{projectId}/memory`（无 DELETE，仅可编辑不可删除）；
> ②记忆容量上限统一调整为 10k 字符（需求工作记忆同改），校验上移至 `core::memory` 共用；
> ③项目记忆属知识库成员：`knowledge index` 固定 `key=memory` system 条目，知识概览/列表恒 eager 展示；
> ④CLI 新增顶层 `chunsun memory get|put`；
> ⑤skill.md 新增「项目记忆」节（存储/命令/容量/记录时机/边界），loop-rules.md 与 commands.md 同步；
> ⑥console 知识页像宪法一样展示项目记忆（可编辑、不可删除）。
>
> 2026-08-21-host-dual-mode：Pre-flight 前增加宿主选择——存在 `chunsun_*` Agent 工具则走工具直连，否则走 CLI。
>
> 2026-08-06-ide-skills：技能本体（SKILL.md + references）从 `.agents/skills/chunsun/` 迁到所选
> IDE 的 `<ide>/skills/chunsun/`（Cursor/Trae/Qoder/CodeBuddy 分别落在 `.cursor`/`.trae`/`.qoder`/`.codebuddy`）；
> 不再维护 `.agents`。`init` 按交互/ `--ide` 选择目标 IDE。
>
> 2026-08-07-rules-agents-bridge（已于 2026-09-08-skill-only-harness 撤销）：四家 IDE 的规则文件包裹含
> `alwaysApply: true` 的 frontmatter；新增仓库根 `AGENTS.md` 桥接段落。2026-09-08 起 harness 不再管理
> AGENTS.md / CLAUDE.md，也不再安装规则文件——相关内容全部由技能暴露。
>
> 2026-08-13-claude-code：新增 ClaudeCode 目标（`.claude/skills`）。2026-09-08 起斜线命令与规则文件
> 不再安装，仅技能本体落在 `.claude/skills/chunsun/`。

| Template file | Install path |
| --- | --- |
| `skill.md` | `<ide.skillsDir>/chunsun/SKILL.md`（按 `init` 所选 IDE；唯一 harness 载体，含意图路由/交付协议/缺陷修复派生） |
| `commands.md` | `<ide.skillsDir>/chunsun/references/commands.md` |
| `loop-rules.md` | `<ide.skillsDir>/chunsun/references/loop-rules.md`（技能激活期间恒生效；不再另行安装到 `<ide>/rules`） |

## Removed in 2026-09-08-skill-only-harness

| Removed template | Reason |
| --- | --- |
| `slash/chunsun.md` | 斜线命令模板并入技能：技能可由自然语言或 "/" 调出，交付协议即 skill.md「自主交付协议」 |
| `slash/chunsun-fix.md` | 派生修复流程并入 skill.md「缺陷修复派生」节 |
| （规则文件安装，非模板删除）`<ide>/rules/chunsun-workflow-gates.*` | 规则随技能激活恒生效，读 `references/loop-rules.md`，不再单独安装常驻规则文件 |
| （AGENTS.md / CLAUDE.md 桥接，Rust 生成） | harness 不再管理仓库根桥接文件；升级时自动剥离旧 marker 段落 |

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
| `LOOP_RULES_RULE` (full, with frontmatter) | Full Cursor `.mdc` form kept only in TS; `loop-rules.md` is the plain body.（2026-09-08 起规则不再按 IDE 包裹 frontmatter 安装） |
| `listWorkflowInstallFiles` / index re-exports | Orchestration only, not prompt bodies. |

## Naming notes

- `loop-rules.md` 无 frontmatter（仅正文），作为技能引用安装（`references/loop-rules.md`），技能激活期间恒生效；不再按 IDE 包裹 frontmatter 安装常驻规则文件。
- 中文文件名仅存在于历史版本；斜线命令模板已于 2026-09-08 并入技能（skill.md「意图路由」+「缺陷修复派生」）。
