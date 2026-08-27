-- 上下文命名重命名：消除项目级 context 与需求级 context 的混淆
-- 背景：项目级（project_context_document + project_policy 宪法）与需求级（context 工作记忆）
--       共用 context 一词，API/CLI/代码均易混淆。
-- 设计：需求级 context → requirement_memory；项目级 project_context_document → project_knowledge_document。
--       project_policy（宪法）保留原名——它不含 context 字样，且是独立的策略表。
-- 兼容：旧表名创建可更新视图，保留两个版本周期后移除。

-- 1) 需求级工作记忆：context → requirement_memory
ALTER TABLE "context" RENAME TO "requirement_memory";

-- 2) 项目级知识文档：project_context_document → project_knowledge_document
ALTER TABLE "project_context_document" RENAME TO "project_knowledge_document";

-- 3) 旧名兼容视图（单表视图在 PG 中默认可更新，支持 INSERT/UPDATE/DELETE）
--    移除时机：v0.7.0 或两个 minor 版本后
CREATE VIEW "context" AS SELECT * FROM "requirement_memory";
CREATE VIEW "project_context_document" AS SELECT * FROM "project_knowledge_document";
