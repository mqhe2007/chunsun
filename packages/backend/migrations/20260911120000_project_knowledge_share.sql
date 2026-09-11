-- 知识文档只读公开分享（仅自定义文档；宪法/记忆等系统项不建行、API 拒绝）。
-- token 仅存 SHA-256 hex；明文只在创建/轮换响应中返回一次。

CREATE TABLE IF NOT EXISTS "public"."project_knowledge_share" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "document_id" TEXT NOT NULL,
    "token_hash" TEXT NOT NULL,
    "enabled" BOOLEAN NOT NULL DEFAULT TRUE,
    "expires_at" TIMESTAMPTZ(6),
    "created_by" TEXT NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,
    CONSTRAINT "project_knowledge_share_pkey" PRIMARY KEY ("id")
);

CREATE UNIQUE INDEX IF NOT EXISTS "project_knowledge_share_token_hash_key"
    ON "public"."project_knowledge_share"("token_hash" ASC);

CREATE UNIQUE INDEX IF NOT EXISTS "project_knowledge_share_project_document_key"
    ON "public"."project_knowledge_share"("project_id" ASC, "document_id" ASC);

DO $pks_project$ BEGIN
    ALTER TABLE "public"."project_knowledge_share" ADD CONSTRAINT "project_knowledge_share_project_id_fkey"
    FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $pks_project$;

DO $pks_doc$ BEGIN
    ALTER TABLE "public"."project_knowledge_share" ADD CONSTRAINT "project_knowledge_share_document_id_fkey"
    FOREIGN KEY ("document_id") REFERENCES "public"."project_knowledge_document"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $pks_doc$;
