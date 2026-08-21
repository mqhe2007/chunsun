-- 缺陷表单删除"应用"、"模块"字段：对应列不再使用。
-- 背景：需求「缺陷表单删除'关联需求'、'应用'、'模块'字段」。
-- 说明：application 和 module 列仅用于前端展示，无业务逻辑依赖，
-- 直接 DROP COLUMN；requirement_id 保留（/chunsun-fix 联动依赖）。
-- 前端留存数据通过 API 响应中已有的 id/description 等字段回溯即可。

ALTER TABLE "public"."defect" DROP COLUMN IF EXISTS "application";
ALTER TABLE "public"."defect" DROP COLUMN IF EXISTS "module";