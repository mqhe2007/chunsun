---
name: chunsun
description: 春笋自主交付工作技能。用户表达「开始/继续/迭代某个需求」「修一个缺陷」等意图时触发。平台 SSOT：需求/轮次/步骤/Context/场景/用例/缺陷存平台；本地为 .env + 本技能 + 两个斜线命令（/chunsun /chunsun-fix）。自主交付：一次 /chunsun 连续工作到验收绿 / 需用户决策 / 用户打断才停。Trigger on start/continue/iterate a requirement, or fix a defect. Platform SSOT.
argument-hint: '<requirement-id>'
---

# 春笋自主交付（Harness）

**平台唯一真相源。** 本地保留 `.env`（`CHUNSUN_SECRET_KEY`，可选 `CHUNSUN_API_URL`）、本技能（所选 IDE 的 `skills/chunsun/`）、常驻规则（`<ide>/rules/chunsun-workflow-gates.*`）、两个斜线命令（`<ide>/commands/`）与仓库根桥接段落（`AGENTS.md`；Claude Code 为 `CLAUDE.md`）。

## 宿主选择（自动鉴别，二选一）

本技能支持两种执行宿主，按当前 Agent 实际可用的工具自动鉴别，**只走其中一条**，不要同时混用。

**鉴别方法**：检查当前 Agent 的工具（tool）目录里是否存在 `chunsun_run_start` 与 `chunsun_step_add` 这两个工具。

### 情况 A：存在 `chunsun_*` Agent 工具（如 DeepSeek Harness 的 dsh-pm 插件）

走 **Agent 工具**执行，不再调用 `chunsun` CLI。每个语义动作对应一个结构化工具，参数取自工具 schema（下文 CLI 命令仅作语义参考，勿执行）：

| 语义动作 | Agent 工具 | 对应 CLI |
| --- | --- | --- |
| 开新 Run | `chunsun_run_start` | `chunsun run start` |
| 撞锁接管 | `chunsun_run_takeover` | `chunsun run takeover` |
| Run 状态迁移 | `chunsun_run_status` | `chunsun run status` |
| 柔性约束提醒 | `chunsun_run_remind` | `chunsun run remind` |
| 上报 Step | `chunsun_step_add` | `chunsun step add` |
| 场景 upsert / 状态 | `chunsun_scenario_upsert` / `chunsun_scenario_status` | `chunsun scenario upsert/status` |
| 用例 list / upsert / 状态 | `chunsun_case_list` / `chunsun_case_upsert` / `chunsun_case_status` | `chunsun case list/upsert/status` |
| 工作记忆读写 | `chunsun_context_get` / `chunsun_context_put` | `chunsun context get/put` |
| 全量重置 | `chunsun_reset` | `chunsun reset` |
| 缺陷派生修复 | `chunsun_fix` | `chunsun fix` |
| 环境变量 | `chunsun_env_list` / `chunsun_env_get` | `chunsun env list/get` |
| 需求 | `chunsun_requirement_list/show/create/update` | `chunsun requirement *` |
| 连接状态 | `chunsun_status` | —（仅平台状态） |

### 情况 B：不存在 `chunsun_*` Agent 工具（其它 IDE / Cursor / Claude Code 等）

走 **`chunsun` CLI**，执行下文「CLI 参考」与 `references/commands.md` 中的命令；技能文件即本仓库 `skills/chunsun/`。

**Pre-flight 仅在情况 B 生效**：静默执行 `chunsun update --check`，非 0 则提示 `chunsun update` 并收尾。

## Pre-flight

（仅情况 B / CLI 宿主）

1. 静默执行 `chunsun update --check`：退出码 0 继续；非 0 提示 `chunsun update` 并结束会话。
2. `chunsun update` 会在当前仓库自动刷新技能模板（升级或本地模板落后时都会刷）；仅当首次接入仓库、或 `chunsun --help` 仍与本地命令不一致时，才需手动 `chunsun init`。

## 斜线命令（仅 2 个）

| 命令 | 用途 |
| --- | --- |
| `/chunsun <需求ID>` | 启动/继续/迭代自主交付 |
| `/chunsun-fix <缺陷ID>` | 派生唯一修复需求（origin=defect，缺陷 1:1）并进入自主交付 |

无 `/暂停` `/重来`：暂停 = 用户会话内抢话打断；重来 = 用户自然语言说明（"把登录那块重做"），由你判断意图后执行全量重置。

## 状态机

```
需求：pending → running → completed ──再 /chunsun──▶ running（新 Run）
轮次：running → completed（需求全绿）/ finished（正常收尾，预期下一轮）/ abandoned（放弃，不再推进）
```

- `Requirement.status` 是最新轮次的投影：需求状态 = 最新一次连续工作（Run）的状态。从未跑过 = pending。
- **轮次无「暂停」**：paused 已废弃——轮次不存在恢复（无 resume 命令，finished/abandoned 后不续跑原轮次）。`finished` 表示本轮正常收尾、预期下一轮，投影为需求 running；`abandoned` 表示放弃，需求不再推进（终态）。
- 每次 `/chunsun` 都开**新轮次**（finished 后不续跑原轮次），轮次是"一次连续工作"的时间切片。
- 缺陷 1:1 修复需求：修复需求 completed → 缺陷 resolved；复发 = 用户人工把缺陷拉回 open/processing，对**同一需求**再 `/chunsun` 迭代，不派生新需求。

## 自主交付协议（/chunsun 执行体）

```
/chunsun <需求ID>
  1. 拉取上下文：chunsun context get <ID> + chunsun scenario list <ID> --include-cases
     + 当前 Git 状态 + 环境变量（chunsun env list）
  2. 开新 Run：chunsun run start <ID>
     - 若报已有 Run 在跑（撞锁）：向用户展示最后活跃时间，用户确认后
       chunsun run takeover <ID>（僵尸 Run 人工接管），再 start
  3. 进入循环：
     a. 决策下一步 Step kind（think / code / test / verify / ask_user / info / reflect）
     b. 执行 Step；涉及验收变化时 upsert 场景/用例并回写状态
     c. 每个 Step 完成即上报：chunsun step add <ID> --run <runId> --kind ... --summary ...
     d. 柔性约束（规则提醒层，进下一轮 prompt 前调用 chunsun run remind <ID>）：
        - 尚无任何场景 → 至少 upsert 一个 happy path 场景
        - 有 failing 场景 → 修复后置 passing
        - 有 open decisions 堆积 → 优先向用户确认
        - 长轮次无 test/verify → 注意验收闭环
        - 有 code Step 且无 reflect → 关键环节做一次评审-反思-改进（见「RRI」节）
     e. 停点检查：
        - 所有场景 passing 或 waived 且无 open decisions → chunsun run status <ID> --status completed
        - ask_user 产生 open decision → chunsun run status <ID> --status finished --reason <问题>，停
        - 用户会话内抢话打断 → 当前 Step 收尾后置 finished（--reason 说明打断），停
        - 否则回 a
  4. 收尾/完成时输出：本轮 Step 摘要 + 验收状态 + 下一步建议
```

**停点只有三种**（验收全绿 / 需用户决策 / 用户打断）与 completed 平台硬条件见 `references/loop-rules.md`「停点」；平台拒绝时返回 COMPLETION_GATE_NOT_MET，不要绕过。

## Context（工作记忆）

每个需求一份，平台 SSOT，CLI 维护。**关系型数据（场景/用例）唯一真相在平台表，Context 不存镜像**——启动时实时拉取聚合。Context 只存：

- `requirementSnapshot`：已澄清边界（重来保留）
- `lastRunSummary`：上一轮决策链与结果
- `openDecisions`：待用户确认的点
- `codeLandmarks`：关键代码位置/摘要（不存代码本体）
- `envRefs`：引用的环境变量 key

**粒度严控**（整体 ~20k 字符内）：Context 是唯一进 prompt 的工作记忆，存太少断点不可续、存太多爆上下文，按「续跑必需」原则取舍。Step.detail 宽松存（不回喂 prompt，仅平台展示/审计）。

## 验收定义（passing 的标准）

见 `references/loop-rules.md`「验收定义」——该规则同时以常驻规则安装，始终生效：真实依赖跑通才算 passing、禁 stub / 替身冒充、不得伪造 passed。waived / reset 等操作见下文「自然语言动作」。

## RRI（评审-反思-改进）

关键环节要做一次评审-反思-改进，以上报一个 `reflect` Step 计：

```bash
chunsun step add <ID> --run <runId> --kind reflect --summary "评审了什么 / 发现什么 / 改进动作"
```

**四类关键环节**：

1. code Step 之后、进入 test/verify 之前
2. 场景 failing 修复之后（置 passing 前复盘根因）
3. 准备 completed 之前（整体回顾本轮 Run）
4. 收到用户反馈/纠偏之后

**定义不验证**：RRI 是柔性约束，不进 completed 硬门禁。CLI remind 只检测「有 code Step 且其后无 reflect」这一种可判定情形并提示；其余三类由你在循环中自觉执行。reflect 的 summary 建议三段式：评审了什么 → 发现什么 → 改进动作；无发现问题时明确写"无偏差，继续"即可，不要编造问题。

## 自然语言动作

- **豁免**：用户说"这个我认了 / 暂不修" → 场景置 waived + Step 留痕。
- **重来**：用户说"重做 / 重来" → `chunsun reset <ID>`（幂等：清 Context 工作记忆保留澄清边界 + 场景/用例重置 pending + 开新 Run），继续循环。
- **暂停/停**：用户直接打断 → 收尾置 finished（`--reason` 说明打断）；继续 = 再 `/chunsun` 开新 Run。

## 三层边界（编排归属）

见 `references/loop-rules.md`「边界」（Agent 管下一步 / 平台管状态合法性 / CLI 管事实搬运与提醒；不要臆造平台状态）。协议升级走 `chunsun update` 刷新本模板。

## CLI 参考

`chunsun --help` 查看全部命令；核心：

```
chunsun run start|takeover|status|list|remind <需求ID>
chunsun step add <需求ID> --run <runId> --kind <think|code|test|verify|ask_user|info|reflect> --summary <...>
chunsun scenario list|upsert|status
chunsun case list|upsert|status
chunsun context get|put
chunsun reset <需求ID>
chunsun fix <缺陷ID>
```

详细命令清单见 `references/commands.md`。