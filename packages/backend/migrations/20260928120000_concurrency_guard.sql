-- 并发写入保护（第一层 + 第二层）。
--
-- 两个独立问题合并在一个迁移里，因为都要在启动时一次性生效、且都不可逆：
--
-- 1) 知识库 / 记忆的丢失更新（lost update）
--    四个写入点都是「整篇覆盖」语义，谓词只有 `WHERE id = $n`，后写者无条件胜出。
--    harness 协议要求的恰恰是「拉取 → 本地改 → 全量覆盖」，天然是 read-modify-write 竞态。
--    加 `revision` 整数列作为乐观锁版本载体。
--
--    **为什么不用 updated_at 当版本号**（曾评估后退回）：
--    - `core/datetime.rs` 的 `format_millis` 是全后端下发时间戳的唯一出口，把微秒截断到
--      毫秒（对齐 JS `toISOString`）。库里是微秒 `timestamptz(6)`，客户端拿到的是毫秒，
--      回传即产生 123456µs vs 123000µs 的常数级不匹配，每次写入都会误报冲突。
--    - 时钟源不一致：知识文档/宪法用应用层 `Utc::now()`，两种记忆用数据库 `NOW()`。
--      多实例下应用层时钟不可靠，且毫秒截断让连续两次写入可能落在同一毫秒。
--    - 版本正确性会同时依赖应用时钟、format_millis 的精度选择、和 JS Date 的毫米限制。
--    `revision` 与时钟无关，比较语义无歧义，且不触碰 `updated_at` 的任何现有行为
--    （Prisma 兼容语义、空补丁不刷新 updated_at 的复刻、既有时间戳断言全部不受影响）。
--
-- 2) Run 锁只有应用层 check-then-act
--    `routes/harness.rs` 的 create_run_handler 是「先查 running 再插」，读/判定/写跨三个事务，
--    数据库层无兜底 —— 同刻并发可插出两条 running Run。
--
-- 迁移由 `sqlx::migrate!` 编译期嵌入二进制、启动时自动执行（main.rs），无需额外部署步骤。

-- ============================================================================
-- 一、乐观锁版本列（四张表）
-- ============================================================================
--
-- `NOT NULL DEFAULT 1`：存量行全部落到 revision = 1，零数据迁移。
-- 客户端用 `revision = 0` 表达「我认为这行还不存在」，首次写入（建宪法 / 建记忆）用它。
-- 新建行时应用层显式写 1。

ALTER TABLE "public"."project_knowledge_document"
    ADD COLUMN IF NOT EXISTS "revision" INTEGER NOT NULL DEFAULT 1;

ALTER TABLE "public"."project_policy"
    ADD COLUMN IF NOT EXISTS "revision" INTEGER NOT NULL DEFAULT 1;

ALTER TABLE "public"."project_memory"
    ADD COLUMN IF NOT EXISTS "revision" INTEGER NOT NULL DEFAULT 1;

ALTER TABLE "public"."requirement_memory"
    ADD COLUMN IF NOT EXISTS "revision" INTEGER NOT NULL DEFAULT 1;

-- ============================================================================
-- 二、Run 数据收敛（必须在建唯一索引之前）
-- ============================================================================
--
-- 建 `uq_run_one_running_per_requirement` 前必须先消除已有的同需求多 running，否则
-- CREATE UNIQUE INDEX 直接失败、迁移中止、后端起不来。
--
-- 这些脏数据来自 baseline 的旧语义：`RunStatus` 原为 ('running','paused','completed')，
-- 允许「pause 后开新 Run」，因此同一 requirement 可以积累多条 running。
-- `20260813120000_run_status_v2.sql` 删除了 paused 状态并把历史 paused 行刷成
-- finished/abandoned，但**没有处理「多条 running 并存」**这一情形。
--
-- 收敛规则：每个 requirement 只保留 `index` 最大的那条 running（最新一轮），
-- 其余置 finished 并写明原因，与该状态机的既有语义一致（finish = 本轮结束、可再开新轮）。

DO $run_converge$ BEGIN
    UPDATE "public"."run" AS r
       SET "status" = 'finished'::"public"."RunStatus",
           "ended_at" = COALESCE(r."ended_at", NOW()),
           "end_reason" = COALESCE(r."end_reason", '并发保护迁移：同需求存在多条 running，保留最新一轮'),
           "updated_at" = NOW()
     WHERE r."status" = 'running'::"public"."RunStatus"
       AND EXISTS (
           SELECT 1 FROM "public"."run" AS newer
            WHERE newer."requirement_id" = r."requirement_id"
              AND newer."status" = 'running'::"public"."RunStatus"
              AND (newer."index" > r."index"
                   OR (newer."index" = r."index" AND newer."id" > r."id"))
       );
END $run_converge$;

-- ============================================================================
-- 三、Run 唯一约束
-- ============================================================================
--
-- 部分唯一索引：同一 requirement 至多一条 running。
-- 谓词里的 enum 字面量需显式 cast，且 cast 的是 20260813120000 重建后的**新** RunStatus。
--
-- 这是「多 Agent 抢同一需求」的最终防线。应用层的预检查保留（能给出更友好的
-- lastActiveAt），但不再是唯一防线 —— 竞态下由本索引把重复插入变成 23505，
-- 由 handler 映射为 409 RUN_ALREADY_RUNNING。

CREATE UNIQUE INDEX IF NOT EXISTS "uq_run_one_running_per_requirement"
    ON "public"."run" ("requirement_id" ASC)
    WHERE "status" = 'running'::"public"."RunStatus";

-- `index` 的分配是 `SELECT COALESCE(MAX(index),0) + 1`（repos/harness.rs 的 create_run），
-- 该读写不在同一把锁下，并发时会算出同一个 index。
-- 加唯一约束后由数据库拒绝，应用层捕获 23505 后重试一次即可；
-- 没有它则是两条 Run 索引相同、静默错乱。
--
-- 同样需要先收敛：`idx_run_req_index`（baseline）是普通非唯一索引，历史上可能已有重复。
--
-- **不能用「相关子查询算 MAX + 1」那种写法**（本迁移的初版就是这么写的，在真库上必挂）：
-- 同一分区内 index 全相同（比如三条都是 1）时，UPDATE 的 SET 子查询在同一条语句的
-- 同一快照下对每行求值，且彼此看不见对方的新值 —— 三条里「比自己小的」都只有 b1，
-- 于是 b2、b3 各自算出同一个 `MAX=1 → 2`，收敛语句**自己造出新的重复**，
-- 紧接着的建唯一约束必然失败：
--     could not create unique index "run_requirement_id_index_key"
-- 正确做法是把「重排」交给窗口函数。但**只重排重复的行是不够的**（第二版踩的坑，
-- 见下），必须整个分区一起重排，且新值要绕开任何行已占用的编号。
--
-- **第二版为什么也不行**：那一版只动 `same_index > 1` 的行，给它们编 1..n。
-- 但分区的最大值可能远大于 n —— 于是重排出来的小号会撞上**没被重排**的行：
--     (c1,1) (c2,1) (c3,2)   →  c1=1 不动、c2 重排成 2  →  与 c3 的 2 撞车
-- 只重排一部分，等于把重复从「组内」搬到了「组与组之间」，建唯一约束照样失败。
--
-- 正确规则：**每个分区内，按 ("index", "id") 保序整体重编号**，
-- 每个 index 组占一段，段的起点是 `已出现过的行数`（即前序组的大小之和 + 1），
-- 组内按 id 递增。这样既保持「同一 group 内的相对顺序」（同 index 的按 id 有序），
-- 又让新编号**严格递增且无重复**，必然全部落在 {1..分区行数} 内。
-- 例：(c1,1)(c2,1)(c3,2) → c1=1, c2=2, c3=3。
--     (d1,1)(d2,2)(d3,5) → 1,2,3（原为空档，但既然要重排就压紧）。
--
-- 只处理**确有重复的分区**（`dup > 0`）：无重复的分区即使 index 留有空档
-- （如 1,2,5）也原样保留 —— 空档是「放弃过某轮」的历史痕迹，`index` 是对用户
-- 展示的轮次编号，没坏就不要重编号。跨需求互不影响。
-- **第三版踩的坑（只有真实数据能触发，临时表样例漏掉了）**：`dirty` 必须是
-- **requirement 级**标记，不能是「重复的那些组」。第三版写成
-- `SELECT DISTINCT requirement_id FROM groups WHERE grp_size > 1`，但那个 CTE 返回的
-- 是**重复的 index 值**、不是重复的 requirement —— 后续 `WHERE k.requirement_id IN (...)`
-- 与 `k.index` 无关，于是把别的组也一起重排了，可 `picked` 的行是完整的……
-- 等等，实际后果看这里：真数据 `(1,1), (2,1), (3,1)` 这组，
-- 重复的只有 index 1，重排后 index 1 的两行变成 1、2，而 index 2、3 的行**没进**
-- `picked`（它们所在组的 grp_size = 1），保留原值 2、3 —— 新值 2 与旧值 2 撞车，
-- 建唯一约束失败：`could not create unique index "run_requirement_id_index_key"`。
-- 修正：用 `bool_or(grp_size > 1) OVER (PARTITION BY requirement_id)` 把「该需求有重复」
-- 传播到该需求的**每一组**，这样 `picked` 覆盖整个分区，重排是全分区一致的。
-- 例：(1,1)(2,1)(3,1) 这条需求（index 1 重复、2 和 3 各一行）→ 1,2,3,4。
--
-- 实现分三步，**不能**像初版那样在窗口里嵌 `count(*)` 又同时选 `id`
-- （PG 报 `column "run.id" must appear in the GROUP BY clause`）：
--   groups  按 (requirement_id, index) 聚合得到组大小，并用窗口 `bool_or` 标出整个分区是否脏
--   slots   在组级别开窗，算出每段可用的起始编号 base
--   picked  展开回具体行，组内按 id 排序分配 base .. base+size-1
WITH groups AS (
    SELECT "requirement_id", "index", count(*) AS grp_size,
           bool_or(count(*) > 1) OVER (PARTITION BY "requirement_id") AS req_dirty
      FROM "public"."run"
     GROUP BY "requirement_id", "index"
),
slots AS (
    SELECT "requirement_id", "index",
           sum("grp_size") OVER (
               PARTITION BY "requirement_id"
               ORDER BY "index"
               ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
           ) - "grp_size" + 1 AS base
      FROM groups
),
picked AS (
    SELECT s."requirement_id",
           r."id",
           s."base" + row_number() OVER (
               PARTITION BY s."requirement_id", s."index" ORDER BY r."id"
           ) - 1 AS new_index
      FROM slots AS s
      JOIN "public"."run" AS r
        ON r."requirement_id" = s."requirement_id"
       AND r."index" = s."index"
),
dirty AS (
    -- requirement 级：只要该需求有任一 index 组 >1 行，整条需求都要重排。
    SELECT DISTINCT "requirement_id" FROM groups WHERE "req_dirty"
)
UPDATE "public"."run" AS r
   SET "index" = k."new_index"
  FROM picked AS k
 WHERE r."id" = k."id"
   AND k."requirement_id" IN (SELECT "requirement_id" FROM dirty)
   AND k."new_index" <> r."index";

DO $run_index_unique$ BEGIN
    ALTER TABLE "public"."run"
    ADD CONSTRAINT "run_requirement_id_index_key" UNIQUE ("requirement_id", "index");
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $run_index_unique$;
