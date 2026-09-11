-- 成员侧可再次查看分享链接：存明文 token（仅项目成员可读；公开校验仍走 token_hash）。
ALTER TABLE "public"."project_knowledge_share"
  ADD COLUMN IF NOT EXISTS "token" TEXT;

-- 新写入必填；历史行可为空，需重新生成一次链接。
