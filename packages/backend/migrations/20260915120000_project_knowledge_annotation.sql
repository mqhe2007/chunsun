-- 知识库文档批注（需求 u-WPvdvYh4Fw，三步一体：文档级 / 内联 / LLM 结合批注优化）。
--
-- 设计要点：
-- - 批注挂「文档引用」而非物理外键：doc_kind ∈ constitution|memory|document，
--   document_id 仅 doc_kind='document' 时有值（宪法 / 项目记忆是项目级单例，无文档行）。
--   因此**不能**给 document_id 加外键——删文档时批注要显式随之处置（见 repos 层）。
-- - anchor_text 为 NULL = 整篇批注；非 NULL = 内联批注，存**纯文本**锚点
--   （anchor_text + anchor_prefix + anchor_suffix），不存 DOM 路径 —— 渲染器变更后
--   仍可用前后文片段重定位。
-- - status 与 outcome 拆开：status ∈ open|resolved|stale，outcome ∈ addressed|dismissed
--   （仅 resolved 时有值）。stale 只由锚点漂移置位，不参与结案。
-- - resolved_note = AI / 人结案时给出的依据（选项 B 的「可核查兜底」硬前提）。
-- - 批注**永不自动删除**：resolved / stale 留作文档变更因果史，清理只靠人显式删，
--   不引入自动 GC（无 deleted_at / 无 TTL）。

CREATE TABLE IF NOT EXISTS "public"."project_knowledge_annotation" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "doc_kind" TEXT NOT NULL,
    "document_id" TEXT,
    "anchor_text" TEXT,
    "anchor_prefix" TEXT,
    "anchor_suffix" TEXT,
    "body" TEXT NOT NULL,
    "status" TEXT NOT NULL DEFAULT 'open',
    "outcome" TEXT,
    "resolved_note" TEXT,
    "resolved_by" TEXT,
    "resolved_at" TIMESTAMPTZ(6),
    "created_by" TEXT NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,
    CONSTRAINT "project_knowledge_annotation_pkey" PRIMARY KEY ("id")
);

-- 面板按文档取批注（阅读页 / Agent 注入都走这条）
CREATE INDEX IF NOT EXISTS "project_knowledge_annotation_doc_idx"
    ON "public"."project_knowledge_annotation"("project_id" ASC, "doc_kind" ASC, "document_id" ASC);

-- 候选文档 id 跨项目唯一，此索引仅供批注反查（如「该文档还有多少批注」）
CREATE INDEX IF NOT EXISTS "project_knowledge_annotation_document_id_idx"
    ON "public"."project_knowledge_annotation"("document_id" ASC);

DO $pka_project$ BEGIN
    ALTER TABLE "public"."project_knowledge_annotation" ADD CONSTRAINT "project_knowledge_annotation_project_id_fkey"
    FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $pka_project$;
