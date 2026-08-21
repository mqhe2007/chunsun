-- 密码哈希：新账号/改密/重置使用 Argon2id（$argon2id$...）。
-- 存量 bcrypt（$2a$/$2b$）登录校验通过后由应用层就地升级为 Argon2，无需批量刷库。

COMMENT ON COLUMN "user".password IS
  'Argon2id hash ($argon2id$...) or legacy bcrypt ($2a$/$2b$), upgraded on successful login';
