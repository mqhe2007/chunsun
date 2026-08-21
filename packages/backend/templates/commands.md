# chunsun CLI 命令参考

这些命令是春笋自主交付 Skill 调用的底层动作。AI 助手会自动执行；人工愿意接管时也可以手动运行。

**宿主说明**：若当前 Agent 已提供 `chunsun_*` 工具（技能「宿主选择」情况 A），本文件仅作语义对照——勿执行 shell，改调对应工具（参数见工具 schema）。情况 B 才执行下文 CLI。

## chunsun run（交付轮次）

```bash
chunsun run list <需求ID>                                        # 列出需求的全部 Run
chunsun run start <需求ID>                                       # 开新 Run（撞锁时提示 lastActiveAt 并建议 takeover）
chunsun run takeover <需求ID>                                    # 撞锁接管：旧 running Run 置 finished（end_reason=被接管），再开新 Run（僵尸 Run 人工接管）
chunsun run status <需求ID> --status completed|finished|abandoned [--reason "..."]   # Run 状态迁移（completed 过平台硬条件；finished/abandoned 记结束原因）
chunsun run remind <需求ID>                                      # 规则提醒层：输出当前未满足的柔性约束
```

- 需求状态 = 最新轮次的投影；每次 `/chunsun` 都开新轮次，finished/abandoned 后不续跑（轮次无 paused、无 resume）。
- `completed` 是平台硬条件：场景须全部 passing 或 waived 且无 open decisions，否则返回 `COMPLETION_GATE_NOT_MET`。

## chunsun step（Step 上报）

```bash
chunsun step add <需求ID> --run <runId> --kind <think|code|test|verify|ask_user|info|reflect> --summary "..." [--detail "..." ] [--artifacts '{"ref":"..."}']
chunsun step list <需求ID> --run <runId>                         # 列出某 Run 的 Steps
```

每个 Step 完成后必须上报（seq 自动递增）。detail 宽松存，不进 prompt。

## chunsun scenario / case（验收）

```bash
chunsun scenario list <需求ID> [--include-cases]                 # 列出场景（含状态）
chunsun scenario upsert <需求ID> --key login-happy --title "登录成功" [--status passing]
chunsun scenario status <需求ID> <场景ID> <pending|passing|failing|blocked|waived>
chunsun case list <需求ID>
chunsun case upsert <需求ID> --scenario <场景ID|key> --title "..." [--kind e2e] [--plan auto]
chunsun case status <需求ID> <用例ID> <passed|failed|blocked|skipped> [--result "..."]
```

场景/用例唯一真相在平台表；`waived` 只能由用户自然语言豁免触发并留痕。

## chunsun context / reset / fix

```bash
chunsun context                                                  # 项目整体开发上下文摘要（区别于需求 Context）
chunsun context get <需求ID>                                     # 拉取工作记忆
chunsun context put <需求ID> --snapshot '{"lastRunSummary":{...}}'  # 增量写回（顶层 key 合并）
chunsun reset <需求ID>                                           # 全量重置：清工作记忆（保留澄清边界）+ 场景/用例重置 pending + 开新 Run
chunsun fix <缺陷ID>                                             # 派生唯一修复需求（缺陷 1:1）并启动自主交付
```

`put` 会先读已有 snapshot，再按顶层 key 合并后写入；缺省 Context 时从空对象开始。
`get` 在尚无 Context 时友好提示并退出 0（`--json` 返回 `{"exists":false}`）。

## chunsun requirement / defect / env（基础管理）

```bash
chunsun requirement list [--status <状态>]                        # 列出需求
chunsun requirement create --description "..."                   # 创建需求
chunsun requirement show <id>
chunsun requirement update <id> --status <状态>
chunsun defect list [--status <状态>] [--severity <级别>]          # 列出缺陷
chunsun defect create --title "..." [--severity <级别>]           # 登记缺陷
chunsun defect show <id>
chunsun defect update <id> --status <状态>
chunsun env list / env get <key>                                 # 环境变量（本地 .env 同名优先，实时拉取不落盘）
```

可用需求状态：`pending` `running` `completed` `abandoned`（无 paused；轮次四态 `running` `completed` `finished` `abandoned`，`finished` 投影为需求 running）。缺陷状态：`open` `processing` `resolved` `closed`。

## 基础命令

```bash
chunsun init                # 安装/刷新技能文件并绑定仓库
chunsun update              # 更新 CLI 并刷新模板
chunsun main-spec list      # 主规格库（按域分篇，与需求循环正交）
chunsun context             # 项目整体开发上下文摘要（区别于需求 Context）
```

## 规则提醒层（柔性约束）

`chunsun run remind <需求ID>` 返回当前未满足的柔性约束，进下一轮 prompt 前调用：

- 无任何验收场景 → 提示 upsert 至少一个 happy path
- 存在 failing 场景 → 提示修复
- 存在未决 open decisions → 提示优先确认
- 长轮次无 test/verify → 提示验收闭环
- 有 code Step 且其后无 reflect → 提示做一次评审-反思-改进（RRI，见 skill「RRI」节）

机制可泛化：任何"违反时才提示"的规则都可挂入。
