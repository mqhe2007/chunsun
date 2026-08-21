-- 春笋数据库 baseline（幂等）
--
-- 来源：2026-08-10 从当时的生产库结构导出（prisma migrate diff --from-empty
--       --to-config-datasource），作为 Rust/sqlx 迁移体系的起点。此前 Prisma
--       时代的 33 个历史 migration 已随旧 Bun 后端一并弃用，不再保留。
--
-- 幂等保证：CREATE TABLE/INDEX 使用 IF NOT EXISTS；CREATE TYPE 与外键约束用
--          DO block 吞掉重复对象异常。因此本文件在空库上完整建库，在已有库
--          上执行为 no-op —— 存量环境无需手工 baseline 标记。
--
-- 后续变更请勿修改本文件，一律用 `cargo sqlx migrate add <name>` 新增。

-- CreateSchema
CREATE SCHEMA IF NOT EXISTS "public";


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."DefectSeverity" AS ENUM ('critical', 'major', 'minor', 'trivial');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."DefectStatus" AS ENUM ('open', 'processing', 'resolved', 'closed');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."ProjectMemberRole" AS ENUM ('OWNER', 'ADMIN', 'MEMBER');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."ProjectStatus" AS ENUM ('INITIALIZING', 'ACTIVE', 'ARCHIVED', 'FAILED');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."RequirementCoverage" AS ENUM ('none', 'partial', 'full');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."RequirementOrigin" AS ENUM ('manual', 'defect');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."RequirementStatus" AS ENUM ('pending', 'running', 'paused', 'completed');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."RunStatus" AS ENUM ('running', 'paused', 'completed');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."ScenarioStatus" AS ENUM ('pending', 'passing', 'failing', 'blocked', 'waived');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."StepKind" AS ENUM ('think', 'code', 'test', 'verify', 'ask_user', 'info', 'reflect');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."TestCaseExecutedBy" AS ENUM ('agent', 'manual');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."TestCaseExecutionPlan" AS ENUM ('auto', 'manual');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."TestCaseKind" AS ENUM ('unit', 'integration', 'e2e');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."TestCaseStatus" AS ENUM ('pending', 'passed', 'failed', 'blocked', 'skipped');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."UserRole" AS ENUM ('ADMIN', 'USER');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateEnum
DO $baseline$ BEGIN
    CREATE TYPE "public"."UserStatus" AS ENUM ('ACTIVE', 'INACTIVE', 'LOCKED');
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."audit_log" (
    "id" TEXT NOT NULL,
    "user_id" TEXT,
    "action" VARCHAR(50) NOT NULL,
    "resource_type" VARCHAR(50),
    "resource_id" VARCHAR(100),
    "metadata" JSONB,
    "ip" VARCHAR(64),
    "user_agent" VARCHAR(500),
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "audit_log_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."context" (
    "id" TEXT NOT NULL,
    "requirement_id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "snapshot" JSONB NOT NULL,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "context_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."defect" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "title" VARCHAR(200) NOT NULL,
    "description" TEXT,
    "status" "public"."DefectStatus" NOT NULL DEFAULT 'open',
    "severity" "public"."DefectSeverity" NOT NULL DEFAULT 'minor',
    "requirement_id" TEXT,
    "module" VARCHAR(100),
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,
    "application" VARCHAR(100),

    CONSTRAINT "defect_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."email_log" (
    "id" TEXT NOT NULL,
    "to" VARCHAR(200) NOT NULL,
    "subject" VARCHAR(255) NOT NULL,
    "template" VARCHAR(50) NOT NULL,
    "status" VARCHAR(20) NOT NULL,
    "error" TEXT,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "email_log_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."email_verification_token" (
    "id" TEXT NOT NULL,
    "user_id" TEXT NOT NULL,
    "token" VARCHAR(255) NOT NULL,
    "expires_at" TIMESTAMPTZ(6) NOT NULL,
    "used_at" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "email_verification_token_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."invitation_code" (
    "id" TEXT NOT NULL,
    "code" VARCHAR(64) NOT NULL,
    "inviter_id" TEXT NOT NULL,
    "role" "public"."UserRole" NOT NULL DEFAULT 'USER',
    "max_uses" INTEGER NOT NULL DEFAULT 1,
    "used_count" INTEGER NOT NULL DEFAULT 0,
    "expires_at" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "invitation_code_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."login_attempt" (
    "id" TEXT NOT NULL,
    "identifier" VARCHAR(255) NOT NULL,
    "user_id" TEXT,
    "attempts" INTEGER NOT NULL DEFAULT 1,
    "last_attempt_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "locked_until" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "login_attempt_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."main_spec" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "domain" VARCHAR(120) NOT NULL,
    "title" VARCHAR(300) NOT NULL,
    "content" TEXT NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "main_spec_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."notification" (
    "id" TEXT NOT NULL,
    "user_id" TEXT NOT NULL,
    "type" VARCHAR(50) NOT NULL,
    "title" VARCHAR(200) NOT NULL,
    "body" TEXT,
    "link" VARCHAR(500),
    "is_read" BOOLEAN NOT NULL DEFAULT false,
    "read_at" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "notification_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."password_reset_token" (
    "id" TEXT NOT NULL,
    "user_id" TEXT NOT NULL,
    "token" VARCHAR(255) NOT NULL,
    "expires_at" TIMESTAMPTZ(6) NOT NULL,
    "used_at" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "password_reset_token_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."project" (
    "id" TEXT NOT NULL,
    "user_id" TEXT NOT NULL,
    "name" VARCHAR(100) NOT NULL,
    "description" TEXT,
    "status" "public"."ProjectStatus" NOT NULL DEFAULT 'INITIALIZING',
    "error_message" TEXT,
    "secret_key" VARCHAR(255),
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "project_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."project_activity" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "user_id" TEXT NOT NULL,
    "action" VARCHAR(50) NOT NULL,
    "entity_type" VARCHAR(50),
    "entity_id" TEXT,
    "description" TEXT NOT NULL,
    "metadata" JSONB,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "project_activity_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."project_context_document" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "title" VARCHAR(200) NOT NULL,
    "content" TEXT NOT NULL DEFAULT '',
    "sort_order" INTEGER NOT NULL DEFAULT 0,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "project_context_document_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."project_env_var" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "key" VARCHAR(128) NOT NULL,
    "value" TEXT NOT NULL,
    "description" VARCHAR(500),
    "is_secret" BOOLEAN NOT NULL DEFAULT true,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "project_env_var_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."project_member" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "user_id" TEXT NOT NULL,
    "role" "public"."ProjectMemberRole" NOT NULL DEFAULT 'MEMBER',
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "project_member_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."project_policy" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,
    "constitution_md" TEXT NOT NULL DEFAULT '',

    CONSTRAINT "project_policy_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."project_prompt" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "system_prompt" TEXT NOT NULL,
    "user_prompt_template" TEXT NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "project_prompt_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."repository" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "name" VARCHAR(100) NOT NULL,
    "slug" VARCHAR(100) NOT NULL,
    "root_hint" VARCHAR(500),
    "is_default" BOOLEAN NOT NULL DEFAULT false,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "repository_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."requirement" (
    "id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "repository_id" TEXT,
    "description" TEXT NOT NULL,
    "source_text" TEXT,
    "client_notes" TEXT,
    "status" "public"."RequirementStatus" NOT NULL DEFAULT 'pending',
    "coverage" "public"."RequirementCoverage" NOT NULL DEFAULT 'none',
    "released_at" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,
    "origin" "public"."RequirementOrigin" NOT NULL DEFAULT 'manual',
    "owner_id" TEXT,

    CONSTRAINT "requirement_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."run" (
    "id" TEXT NOT NULL,
    "requirement_id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "index" INTEGER NOT NULL,
    "status" "public"."RunStatus" NOT NULL,
    "pause_reason" TEXT,
    "started_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "paused_at" TIMESTAMPTZ(6),
    "ended_at" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "run_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."scenario" (
    "id" TEXT NOT NULL,
    "requirement_id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "key" VARCHAR(120) NOT NULL,
    "title" VARCHAR(300) NOT NULL,
    "description" TEXT,
    "status" "public"."ScenarioStatus" NOT NULL DEFAULT 'pending',
    "sort_order" INTEGER NOT NULL DEFAULT 0,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "scenario_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."step" (
    "id" TEXT NOT NULL,
    "run_id" TEXT NOT NULL,
    "seq" INTEGER NOT NULL,
    "kind" "public"."StepKind" NOT NULL,
    "summary" VARCHAR(500) NOT NULL,
    "detail" TEXT,
    "artifacts" JSONB,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "step_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."system_setting" (
    "key" VARCHAR(100) NOT NULL,
    "value" TEXT NOT NULL,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "system_setting_pkey" PRIMARY KEY ("key")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."test_case" (
    "id" TEXT NOT NULL,
    "requirement_id" TEXT NOT NULL,
    "project_id" TEXT NOT NULL,
    "scenario_id" TEXT NOT NULL,
    "title" VARCHAR(300) NOT NULL,
    "kind" "public"."TestCaseKind" NOT NULL DEFAULT 'e2e',
    "steps" TEXT,
    "expected" TEXT,
    "local_path" VARCHAR(500),
    "execution_plan" "public"."TestCaseExecutionPlan" NOT NULL DEFAULT 'auto',
    "status" "public"."TestCaseStatus" NOT NULL DEFAULT 'pending',
    "actual_result" TEXT,
    "executed_at" TIMESTAMPTZ(6),
    "executed_by" "public"."TestCaseExecutedBy",
    "sort_order" INTEGER NOT NULL DEFAULT 0,
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,

    CONSTRAINT "test_case_pkey" PRIMARY KEY ("id")
);


-- CreateTable
CREATE TABLE IF NOT EXISTS "public"."user" (
    "id" TEXT NOT NULL,
    "username" VARCHAR(50) NOT NULL,
    "email" VARCHAR(100) NOT NULL,
    "password" VARCHAR(255) NOT NULL,
    "nickname" VARCHAR(50),
    "role" "public"."UserRole" NOT NULL DEFAULT 'USER',
    "status" "public"."UserStatus" NOT NULL DEFAULT 'ACTIVE',
    "created_at" TIMESTAMPTZ(6) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,
    "qq" VARCHAR(20),
    "email_verified" BOOLEAN NOT NULL DEFAULT false,

    CONSTRAINT "user_pkey" PRIMARY KEY ("id")
);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_audit_action_created" ON "public"."audit_log"("action" ASC, "created_at" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_audit_user_created" ON "public"."audit_log"("user_id" ASC, "created_at" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "context_requirement_id_key" ON "public"."context"("requirement_id" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_context_project" ON "public"."context"("project_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "defect_requirement_id_key" ON "public"."defect"("requirement_id" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_defect_project_id_severity" ON "public"."defect"("project_id" ASC, "severity" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_defect_project_id_status" ON "public"."defect"("project_id" ASC, "status" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_email_log_to_created" ON "public"."email_log"("to" ASC, "created_at" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "email_verification_token_token_key" ON "public"."email_verification_token"("token" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_email_verification_user_id" ON "public"."email_verification_token"("user_id" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_invitation_code" ON "public"."invitation_code"("code" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_invitation_inviter_id" ON "public"."invitation_code"("inviter_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "invitation_code_code_key" ON "public"."invitation_code"("code" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_login_attempt_locked" ON "public"."login_attempt"("locked_until" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_login_attempt_user_id" ON "public"."login_attempt"("user_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "login_attempt_identifier_key" ON "public"."login_attempt"("identifier" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_main_spec_project_id" ON "public"."main_spec"("project_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "uq_main_spec_project_id_domain" ON "public"."main_spec"("project_id" ASC, "domain" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_notification_created_at" ON "public"."notification"("created_at" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_notification_user_read" ON "public"."notification"("user_id" ASC, "is_read" ASC, "created_at" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_password_reset_user_id" ON "public"."password_reset_token"("user_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "password_reset_token_token_key" ON "public"."password_reset_token"("token" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_project_created_at" ON "public"."project"("created_at" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_project_status" ON "public"."project"("status" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_project_user_id" ON "public"."project"("user_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "project_secret_key_key" ON "public"."project"("secret_key" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_project_activity_created_at" ON "public"."project_activity"("created_at" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_project_activity_project_id" ON "public"."project_activity"("project_id" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_project_activity_user_id" ON "public"."project_activity"("user_id" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_project_context_doc_project_sort" ON "public"."project_context_document"("project_id" ASC, "sort_order" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_project_env_var_project_id" ON "public"."project_env_var"("project_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "project_env_var_project_id_key_key" ON "public"."project_env_var"("project_id" ASC, "key" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_project_member_project_id" ON "public"."project_member"("project_id" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_project_member_user_id" ON "public"."project_member"("user_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "project_member_project_id_user_id_key" ON "public"."project_member"("project_id" ASC, "user_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "project_policy_project_id_key" ON "public"."project_policy"("project_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "project_prompt_project_id_key" ON "public"."project_prompt"("project_id" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_repository_project_id_created_at" ON "public"."repository"("project_id" ASC, "created_at" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_repository_project_id_is_default" ON "public"."repository"("project_id" ASC, "is_default" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "uq_repository_project_id_slug" ON "public"."repository"("project_id" ASC, "slug" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_requirement_project_id_status" ON "public"."requirement"("project_id" ASC, "status" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_run_project_started" ON "public"."run"("project_id" ASC, "started_at" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_run_req_index" ON "public"."run"("requirement_id" ASC, "index" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_scenario_project_requirement" ON "public"."scenario"("project_id" ASC, "requirement_id" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "uq_scenario_requirement_id_key" ON "public"."scenario"("requirement_id" ASC, "key" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "uq_step_run_seq" ON "public"."step"("run_id" ASC, "seq" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_case_project_requirement" ON "public"."test_case"("project_id" ASC, "requirement_id" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_case_requirement_kind" ON "public"."test_case"("requirement_id" ASC, "kind" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_case_requirement_status" ON "public"."test_case"("requirement_id" ASC, "status" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_case_scenario_id" ON "public"."test_case"("scenario_id" ASC);


-- CreateIndex
CREATE INDEX IF NOT EXISTS "idx_user_status" ON "public"."user"("status" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "user_email_key" ON "public"."user"("email" ASC);


-- CreateIndex
CREATE UNIQUE INDEX IF NOT EXISTS "user_username_key" ON "public"."user"("username" ASC);


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."audit_log" ADD CONSTRAINT "audit_log_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE SET NULL ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."context" ADD CONSTRAINT "context_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."context" ADD CONSTRAINT "context_requirement_id_fkey" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."defect" ADD CONSTRAINT "defect_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."defect" ADD CONSTRAINT "defect_requirement_id_fkey" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE SET NULL ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."email_verification_token" ADD CONSTRAINT "email_verification_token_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."invitation_code" ADD CONSTRAINT "invitation_code_inviter_id_fkey" FOREIGN KEY ("inviter_id") REFERENCES "public"."user"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."login_attempt" ADD CONSTRAINT "login_attempt_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE SET NULL ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."main_spec" ADD CONSTRAINT "main_spec_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."notification" ADD CONSTRAINT "notification_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."password_reset_token" ADD CONSTRAINT "password_reset_token_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."project" ADD CONSTRAINT "project_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."project_activity" ADD CONSTRAINT "project_activity_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."project_activity" ADD CONSTRAINT "project_activity_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."project_context_document" ADD CONSTRAINT "project_context_document_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."project_env_var" ADD CONSTRAINT "project_env_var_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."project_member" ADD CONSTRAINT "project_member_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."project_member" ADD CONSTRAINT "project_member_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "public"."user"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."project_policy" ADD CONSTRAINT "project_policy_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."project_prompt" ADD CONSTRAINT "project_prompt_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."repository" ADD CONSTRAINT "repository_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."requirement" ADD CONSTRAINT "requirement_owner_id_fkey" FOREIGN KEY ("owner_id") REFERENCES "public"."user"("id") ON DELETE SET NULL ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."requirement" ADD CONSTRAINT "requirement_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."requirement" ADD CONSTRAINT "requirement_repository_id_fkey" FOREIGN KEY ("repository_id") REFERENCES "public"."repository"("id") ON DELETE SET NULL ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."run" ADD CONSTRAINT "run_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."run" ADD CONSTRAINT "run_requirement_id_fkey" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."scenario" ADD CONSTRAINT "scenario_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."scenario" ADD CONSTRAINT "scenario_requirement_id_fkey" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."step" ADD CONSTRAINT "step_run_id_fkey" FOREIGN KEY ("run_id") REFERENCES "public"."run"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."test_case" ADD CONSTRAINT "test_case_project_id_fkey" FOREIGN KEY ("project_id") REFERENCES "public"."project"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."test_case" ADD CONSTRAINT "test_case_requirement_id_fkey" FOREIGN KEY ("requirement_id") REFERENCES "public"."requirement"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;


-- AddForeignKey
DO $baseline$ BEGIN
    ALTER TABLE "public"."test_case" ADD CONSTRAINT "test_case_scenario_id_fkey" FOREIGN KEY ("scenario_id") REFERENCES "public"."scenario"("id") ON DELETE CASCADE ON UPDATE CASCADE;
EXCEPTION WHEN duplicate_object OR duplicate_table THEN NULL;
END $baseline$;
