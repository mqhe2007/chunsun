-- 缺陷模型已移除 title 字段（代码层），此处对齐删除孤儿列
ALTER TABLE "public"."defect" DROP COLUMN IF EXISTS "title";
