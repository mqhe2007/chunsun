-- 20260827000000 创建的旧名兼容视图（注释计划 v0.7.0 移除）会阻塞
-- 20260909104500 对 requirement_memory.snapshot 的 ALTER COLUMN TYPE。
-- 该视图在按序增量升级的库中必然存在，须在改列类型之前先删除，
-- 否则报 cannot alter type of a column used by a view。
-- 单独成迁移而非修改 20260909104500：sqlx 校验已应用迁移的哈希，就地改会拒绝启动。
DROP VIEW IF EXISTS "context";
DROP VIEW IF EXISTS "project_context_document";
