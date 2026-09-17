-- 知识库文档父子关联（主文档 ↔ 分册，需求 AOzsC2VvzMHL）。
-- 可空自引用外键：旧文档 parent_id = NULL 自动成为根，零数据迁移。
-- 同级排序复用 sort_order（列表仍按 sort_order ASC, created_at DESC 取行，应用层组树）。
-- 删除语义在应用层：有子文档默认拒绝；显式 withChildren=true 由应用层递归删除。
-- FK 仍加 ON DELETE SET NULL 兜底：任何绕过应用层的删除只会把子文档提升为根，不产生孤儿。

ALTER TABLE "public"."project_knowledge_document"
    ADD COLUMN IF NOT EXISTS "parent_id" TEXT;

DO $pkd_parent$ BEGIN
    ALTER TABLE "public"."project_knowledge_document"
    ADD CONSTRAINT "project_knowledge_document_parent_id_fkey"
    FOREIGN KEY ("parent_id") REFERENCES "public"."project_knowledge_document"("id")
    ON DELETE SET NULL ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $pkd_parent$;

CREATE INDEX IF NOT EXISTS "idx_project_knowledge_doc_project_parent_sort"
    ON "public"."project_knowledge_document"("project_id" ASC, "parent_id" ASC, "sort_order" ASC);
