---
name: chunsun-fix
id: chunsun-fix
category: Workflow
description: "派生唯一修复需求（缺陷 1:1）并进入自主交付修复，完成后缺陷自动 resolved"
---

# /chunsun-fix <缺陷ID>

把缺陷转成唯一修复需求并进入自主交付：

1. **派生**：`chunsun fix <缺陷ID>`——平台派生修复需求（origin=defect，与缺陷 1:1，缺陷置 processing），并自动启动 Run
2. **自主交付**：按已安装的 chunsun 技能（所选 IDE 的 `skills/chunsun/SKILL.md`）自主交付协议执行（同 /chunsun）；**宿主按技能「宿主选择」鉴别**，下文 CLI 仅作语义参考
   - 先明确缺陷根因（think Step）
   - 修复代码 → 写/跑验证 → upsert 场景/用例回写状态
3. **完成**：场景全 passing/waived 且无 open decisions → `chunsun run status <ID> --status completed`，平台自动把缺陷置 resolved

规则：

- 一个缺陷只对应一个修复需求；缺陷复发 = 用户人工把缺陷拉回 open，对**同一需求**再 `/chunsun` 迭代，不要重复派生
- 若缺陷其实是新需求（范围变大），改为创建普通需求，不走修复线
