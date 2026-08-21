-- 移除 user.username：邮箱作为唯一登录标识，nickname 作为展示名。
-- 背景：登录只认 email；username 既不参与认证，也不作为 URL/@ 提及的公开 handle，
--       实际仅作 nickname 的回退展示。按现代设计收敛为「email（唯一登录）+ nickname（展示名）」。
-- 步骤：1) 防御性把 username 兜底并入空 nickname（当前生产 7 行 nickname 均非空，此步 no-op）；
--       2) 删唯一索引；3) 删列。

-- 1) 兜底合并：仅当 nickname 为空时才用 username 回填，保留用户已设置的展示名
UPDATE "user" SET nickname = username WHERE nickname IS NULL OR nickname = '';

-- 2) 删除 username 唯一索引（独立 CREATE UNIQUE INDEX，需先于删列处理）
DROP INDEX IF EXISTS "user_username_key";

-- 3) 删除 username 列
ALTER TABLE "user" DROP COLUMN IF EXISTS "username";
