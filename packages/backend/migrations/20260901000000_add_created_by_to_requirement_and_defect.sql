-- 需求详情 / 缺陷详情显示创建人：requirement 与 defect 增加 created_by 列。
-- 背景：需求「需求详情，缺陷详情中要显示创建人」。需求与缺陷在创建时记录操作者，
--       详情页展示创建人（昵称/邮箱/头像）。
-- 设计：created_by 存用户 id，可空——历史数据创建人未知，置 NULL；
--       新建行由服务层写入当前登录用户 id（同 owner_id 语义）。
--       外键 ON DELETE SET NULL：用户删除后不级联删除需求/缺陷，仅清空引用。
--       不建索引：当前仅详情展示用途，不做按创建人筛选（与 owner_id 无索引保持一致）。
ALTER TABLE "public"."requirement" ADD COLUMN "created_by" TEXT;
ALTER TABLE "public"."defect" ADD COLUMN "created_by" TEXT;

ALTER TABLE "public"."requirement" ADD CONSTRAINT "requirement_created_by_fkey"
    FOREIGN KEY ("created_by") REFERENCES "public"."user"("id")
    ON DELETE SET NULL ON UPDATE CASCADE;

ALTER TABLE "public"."defect" ADD CONSTRAINT "defect_created_by_fkey"
    FOREIGN KEY ("created_by") REFERENCES "public"."user"("id")
    ON DELETE SET NULL ON UPDATE CASCADE;
