-- 轮次状态机 v2：删除 paused，引入 finished（正常收尾）/ abandoned（放弃）
-- 背景：协议「paused 后不续跑」+ CLI 无 resume 命令 → 轮次不存在真暂停，
--       paused 实为「非 completed 终态」的误名（放弃/打断/等决策/接管/里程碑收尾共用）。
-- 设计：RunStatus 四态 running/completed/finished/abandoned；
--       RequirementStatus 去 paused（pending/running/completed/abandoned），
--       需求级投影：finished → running（需求仍在推进）、abandoned → abandoned。
-- 注意：sqlx migrate 每迁移一个事务，故用「重建 enum」方案而非 ALTER TYPE ADD VALUE。

-- 1) RunStatus 重建：paused 一律先落 finished，第 2 步按 end_reason 关键词刷 abandoned
ALTER TYPE "RunStatus" RENAME TO "RunStatus_legacy";
CREATE TYPE "RunStatus" AS ENUM ('running', 'completed', 'finished', 'abandoned');
ALTER TABLE run ALTER COLUMN status TYPE "RunStatus"
  USING (CASE status::text
    WHEN 'running'   THEN 'running'::"RunStatus"
    WHEN 'completed' THEN 'completed'::"RunStatus"
    ELSE 'finished'::"RunStatus"
  END);
DROP TYPE "RunStatus_legacy";

-- 2) 放弃语义的暂停（pause_reason 含放弃关键词）→ abandoned
UPDATE run SET status = 'abandoned'::"RunStatus"
WHERE status = 'finished'::"RunStatus"
  AND pause_reason IS NOT NULL
  AND (pause_reason LIKE '%放弃%' OR pause_reason LIKE '%不做了%' OR pause_reason LIKE '%不再%');

-- 3) run 表字段语义化：pause_reason → end_reason；paused_at 并入 ended_at 后删列
ALTER TABLE run RENAME COLUMN pause_reason TO end_reason;
UPDATE run SET ended_at = COALESCE(ended_at, paused_at) WHERE paused_at IS NOT NULL;
ALTER TABLE run DROP COLUMN paused_at;

-- 4) RequirementStatus 重建：paused → running（默认），第 5 步按最新 Run 刷 abandoned。
--    requirement.status 有 DEFAULT 'pending'，改类型前须 DROP DEFAULT（PG 无法自动 cast 旧默认值），改完恢复。
ALTER TYPE "RequirementStatus" RENAME TO "RequirementStatus_legacy";
CREATE TYPE "RequirementStatus" AS ENUM ('pending', 'running', 'completed', 'abandoned');
ALTER TABLE requirement ALTER COLUMN status DROP DEFAULT;
ALTER TABLE requirement ALTER COLUMN status TYPE "RequirementStatus"
  USING (CASE status::text
    WHEN 'pending'   THEN 'pending'::"RequirementStatus"
    WHEN 'running'   THEN 'running'::"RequirementStatus"
    WHEN 'completed' THEN 'completed'::"RequirementStatus"
    ELSE 'running'::"RequirementStatus"
  END);
ALTER TABLE requirement ALTER COLUMN status SET DEFAULT 'pending';
DROP TYPE "RequirementStatus_legacy";

-- 5) 需求级投影一致性：最新 Run 为 abandoned 的需求 → abandoned
UPDATE requirement r SET status = 'abandoned'::"RequirementStatus"
WHERE EXISTS (
  SELECT 1 FROM run rn
  WHERE rn.requirement_id = r.id
    AND rn.status = 'abandoned'::"RunStatus"
    AND rn.index = (SELECT MAX(index) FROM run WHERE requirement_id = r.id)
);
