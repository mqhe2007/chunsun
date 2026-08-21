# 自主交付核心规则（常驻生效）

以下规则在任何春笋自主交付工作中始终生效，优先级高于任务提示。

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

## RRI（评审-反思-改进）

- 四类关键环节要做一次 RRI 并上报 `reflect` Step：code 后进 test 前、failing 修复后、completed 前、用户反馈后。
- 柔性约束：不进 completed 硬门禁；CLI 只检测"有 code 无 reflect"并提醒，其余靠自觉。
- reflect 的 summary 三段式：评审了什么 → 发现什么 → 改进动作；无偏差写"无偏差，继续"，不得编造问题。
