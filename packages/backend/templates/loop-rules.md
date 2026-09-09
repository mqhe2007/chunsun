# 自主交付核心规则（技能激活期间恒生效）

以下规则在春笋技能激活期间始终生效，优先级高于任务提示；技能是唯一 harness 载体，规则不再单独安装为常驻规则文件。

## 验收定义（passing 的标准）

- **真实依赖跑通才算 passing**：用 stub / 假数据 / in-memory 替身冒充安全存储、Keychain、凭据库、系统权限等真实依赖，等同未验收。
- **e2e 走真实用户路径**：禁止用替身冒充 e2e 标 passing。
- auto 用例应有 localPath 并跑通；尚未执行的用例保持 pending，不得伪造 passed。

## 停点

- 自主交付只有三种停点：**验收全绿**（所有场景 passing 或 waived 且无 open decisions）、**需要用户决策**、**用户打断**。
- completed 是平台硬条件；不满足时平台拒绝，不要绕过。

## 状态

- 需求状态 = 最新轮次的投影；轮次四态 `running` / `completed` / `finished` / `abandoned`（**无 paused**）。
- 轮次不存在「暂停」：无 resume 命令，finished/abandoned 后不续跑原轮次，继续 = 开新 Run。
- `finished` 投影为需求 running（本轮正常收尾、需求仍在推进）；`abandoned` 投影为需求 abandoned（放弃，不再推进）。
- 场景 `waived` 只能由用户自然语言豁免触发（"这个我认了"），须留痕。

## 边界

- Agent 管"下一步做什么"；平台管"状态合不合法"；CLI / Agent 工具管"事实搬运与提醒"。
- 不要臆造平台状态：以平台经 `chunsun` CLI 或 `chunsun_*` Agent 工具的真实返回为准。

## Memory（工作记忆）

- 工作记忆是**自由 Markdown 文本**，按 skill.md 中的模板结构编写（需求边界 / 待决策 / 代码标记 / 环境变量 / 本轮总结等章节）。
- 写入走「拉取-修改-保存」全流程：`memory get` → 本地修改 → `memory put --snapshot '<完整 Markdown>'` 全量覆盖。
- 「声称即写入」：说「已写入工作记忆」必须真的调用过 `memory put`，只在回复里声称等同没写。
- 每轮 Run 收尾（completed / finished）前必须写「## 本轮总结」；`chunsun run remind` 会检查本轮是否写过记忆并提醒。
- snapshot 整体 ≤ 20k 字符，超限平台拒绝（`MEMORY_TOO_LARGE`）；PUT 缺 `snapshot` 字段平台拒绝（`SNAPSHOT_REQUIRED`）。接近上限时主动精简旧内容。

## RRI（评审-反思-改进）

- 四类关键环节要做一次 RRI 并上报 `reflect` Step：code 后进 test 前、failing 修复后、completed 前、用户反馈后。
- 柔性约束：不进 completed 硬门禁；CLI 只检测"有 code 无 reflect"并提醒，其余靠自觉。
- reflect 的 summary 三段式：评审了什么 → 发现什么 → 改进动作；无偏差写"无偏差，继续"，不得编造问题。
