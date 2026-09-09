-- 工作记忆 snapshot 从 JSONB 改为 TEXT（Markdown 自由文本）
-- 2026-09-09：工作记忆机制从 JSON 五字段结构改为 AI 编写的 Markdown 模板
-- 变更：snapshot JSONB NOT NULL → TEXT NULL（显式 null 表示清空记忆）
-- 数据：现有 JSON 保留为文本形式，由后续迁移脚本转为 Markdown

-- 20260827000000 创建的旧名兼容视图（注释计划 v0.7.0 移除）会阻塞 ALTER COLUMN TYPE，
-- 按序增量升级的库必须先删视图，否则报 cannot alter type of a column used by a view。
DROP VIEW IF EXISTS "context";
DROP VIEW IF EXISTS "project_context_document";

ALTER TABLE "requirement_memory" ALTER COLUMN "snapshot" TYPE TEXT USING "snapshot"::text;
ALTER TABLE "requirement_memory" ALTER COLUMN "snapshot" DROP NOT NULL;
