-- 需求/缺陷依赖关系（Blocking / Blocked By），底层为 DAG（有向无环图）。
--
-- 设计：
-- - 单一 `dependency` 边表，用 `source_type` / `target_type` 区分节点是需求还是缺陷，
--   避免「需求依赖表 / 缺陷依赖表 / 跨类型表」三张表重复。边方向语义为
--   `source blocks target`（source 不完成，target 无法开始），即有向边 source → target。
-- - `source_id` / `target_id` 是 nanoid(12) 字符串，分别引用 requirement.id 或 defect.id，
--   由 `source_type`/`target_type` 决定归属（不建 FK，因为要跨两张表多态引用，且历史数据
--   删除时节点可能已不存在——依赖边应随节点删除级联清理，见下方触发器式约定之外，这里用
--   应用层在删除需求/缺陷时级联删除相关边）。
-- - `UNIQUE (source_type, source_id, target_type, target_id)` 防止重复建边。
-- - `(project_id, source_type, source_id)` 与 `(project_id, target_type, target_id)` 两个索引
--   支持「查某节点全部上下游」的高效查询。
-- - 循环依赖检测在应用层（Rust DFS）做，不依赖 DB 递归 CTE / 触发器（本仓库无此先例）。

CREATE TABLE IF NOT EXISTS "public"."dependency" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "source_type" TEXT NOT NULL,
    "source_id" TEXT NOT NULL,
    "target_type" TEXT NOT NULL,
    "target_id" TEXT NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "dependency_pkey" PRIMARY KEY ("id"),
    CONSTRAINT "dependency_source_target_uniq"
        UNIQUE ("source_type", "source_id", "target_type", "target_id")
);

-- 项目删除级联清理依赖边
ALTER TABLE "public"."dependency" ADD CONSTRAINT "dependency_project_fkey"
    FOREIGN KEY ("project_id") REFERENCES "public"."project"("id")
    ON DELETE CASCADE ON UPDATE CASCADE;

CREATE INDEX IF NOT EXISTS "idx_dependency_source"
    ON "public"."dependency" ("project_id", "source_type", "source_id");

CREATE INDEX IF NOT EXISTS "idx_dependency_target"
    ON "public"."dependency" ("project_id", "target_type", "target_id");
