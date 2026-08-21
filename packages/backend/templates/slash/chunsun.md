---
name: chunsun
id: chunsun
category: Workflow
description: "启动/继续/迭代需求自主交付：一次工作到验收绿 / 需用户决策 / 被打断才停"
---

# /chunsun <需求ID>

启动（或继续/迭代）需求的自主交付。按已安装的 chunsun 技能（所选 IDE 的 `skills/chunsun/SKILL.md`）的「自主交付协议」执行；**宿主按技能「宿主选择」鉴别（Agent 工具或 CLI，二选一）**，下文 CLI 命令仅作语义参考——情况 A 改调对应 `chunsun_*` 工具。

1. **拉上下文**：`chunsun context get <ID>` + `chunsun scenario list <ID> --include-cases` + Git 状态 + `chunsun env list`
2. **开新 Run**：`chunsun run start <ID>`（撞锁时向用户确认后 `chunsun run takeover <ID>`）
3. **自主交付**：决策步骤 kind → 执行 → `chunsun step add` 上报 → 验收变化时 upsert 场景/用例 → 每轮前 `chunsun run remind <ID>` 看柔性约束
4. **停点**：全场景 passing/waived 且无 open decisions → `chunsun run status <ID> --status completed`；需要用户决策 → 置 finished 并 `--reason` 说明问题；用户打断 → 收尾置 finished

关键规则：

- 无场景时先 upsert 至少一个 happy path 场景（柔性约束，提醒层监督）
- 用户说"这个我认了"→ 场景置 waived 留痕；用户说"重做"→ `chunsun reset <ID>` 全量重置
- completed 是平台硬条件（场景全 passing/waived 且无 open decisions），不满足平台会拒绝
- 暂停/重来没有专用命令：用户直接说话即可（停下 = finished，继续 = 再 `/chunsun` 开新 Run）

收尾/完成时输出：本轮 Step 摘要 + 验收状态 + 下一步建议。
