-- 项目级记忆：跨需求复用的填坑、经验、沉淀。
--
-- 设计（对齐需求级 requirement_memory 与宪法 project_policy 的既有模式）：
-- - 1:1 每项目一份（project_id 唯一），snapshot 为自由 Markdown 文本（TEXT，可 NULL = 清空）。
-- - 「属于项目知识库之一」：知识索引 / 知识概览固定展示 key=memory 的 system 条目（同宪法）。
-- - 「仅可编辑不可删除」：不提供 DELETE 端点（同宪法），删除天然不可达。
-- - 容量上限与需求工作记忆统一（MEMORY_SNAPSHOT_MAX_CHARS = 10_000，路由层校验）。

CREATE TABLE IF NOT EXISTS "public"."project_memory" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "snapshot" TEXT,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,
    CONSTRAINT "project_memory_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX IF NOT EXISTS "project_memory_project_id_key" ON "public"."project_memory"("project_id" ASC);

DO $project_memory$ BEGIN
    ALTER TABLE "public"."project_memory" ADD CONSTRAINT "project_memory_project_id_fkey"
    FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $project_memory$;
