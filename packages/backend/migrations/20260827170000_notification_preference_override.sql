-- 用户通知偏好覆盖表：仅存相对默认策略的差异（无行 = 走代码默认）
CREATE TABLE IF NOT EXISTS "public"."notification_preference_override" (
    "user_id" TEXT NOT NULL,
    "category" VARCHAR(32) NOT NULL,
    "channel" VARCHAR(16) NOT NULL,
    "enabled" BOOLEAN NOT NULL,
    "updated_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "notification_preference_override_pkey" PRIMARY KEY ("user_id", "category", "channel"),
    CONSTRAINT "notification_preference_override_category_check"
        CHECK ("category" IN ('security', 'membership', 'delivery', 'defect', 'project')),
    CONSTRAINT "notification_preference_override_channel_check"
        CHECK ("channel" IN ('in_app', 'email')),
    CONSTRAINT "notification_preference_override_user_id_fkey"
        FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE INDEX IF NOT EXISTS "idx_notification_preference_override_user"
    ON "public"."notification_preference_override" ("user_id");
