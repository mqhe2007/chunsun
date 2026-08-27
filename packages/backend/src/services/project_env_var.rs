//! 项目环境变量业务服务（1:1 移植自 `packages/backend/src/routes/projectEnvVar.ts`）。
//!
//! ## 两条互不相同的权限路径（照搬旧实现，不要合并）
//!
//! **读**（list / count / by-key）走 `assertCanReadProject`：
//! SK 项目不匹配 → 403；否则按「可见性」判定，看不见就 **404 PROJECT_NOT_FOUND**。
//!
//! **写**（create / update / delete）走三段式：
//! SK 项目不匹配 → 403；`getProjectByIdOnly` 不存在 → 404；
//! `canProjectActionDb(envVar.write)` 不通过 → 403。
//!
//! 注意写路径用的是 **`getProjectByIdOnly`（不含可见性）**：一个局外人拿别人的项目 id
//! 去写，会拿到 403 而不是 404，等于可以探测「该项目是否存在」。这是旧实现的既有
//! 信息泄漏，移植阶段刻意保留以保证逐字节一致，收紧留到后续统一治理。
//!
//! ## 明文边界
//! 清单类接口（list / create / update 的返回体）**永不返回明文**：`value` 恒为 null，
//! 只给 `hasValue`。明文只有 `by-key` 一个出口，且仅 SK 通道可达。

use sqlx::PgPool;

use crate::api::AppError;
use crate::core::env_var_crypto::{open_env_var_value, seal_env_var_value};
use crate::core::permission_policy::ProjectAction;
use crate::repos::project;
use crate::repos::project_env_var::{
    self, CreateEnvVarInput, ProjectEnvVarRow, UpdateEnvVarPatch,
};
use crate::services::activity_log::{log_activity, ActivityAction, LogActivityOptions};
use crate::services::notification::{notify, NotifyRequest, project_member_user_ids};
use crate::services::project_access::can_project_action_db;

/// 环境变量键名规则：`/^[A-Z][A-Z0-9_]*$/`（大写字母开头，后接大写字母/数字/下划线）。
pub fn is_valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// 加解密所需的密钥来源。服务层不依赖 `AppState`，由路由层从 config 构造后传入。
#[derive(Clone, Copy)]
pub struct EnvCrypto<'a> {
    pub encryption_key: Option<&'a str>,
    pub jwt_secret: &'a str,
}

/// 调用方身份：`sk_project_id` 有值代表这是 CLI 的 Secret Key 通道。
#[derive(Clone, Copy)]
pub struct Caller<'a> {
    pub user_id: &'a str,
    pub is_admin: bool,
    pub sk_project_id: Option<&'a str>,
}

impl Caller<'_> {
    /// `assertSkProjectMatch`：SK 绑定了别的项目 → false（403）；非 SK 通道 → None（放行）。
    fn sk_mismatch(&self, project_id: &str) -> bool {
        matches!(self.sk_project_id, Some(bound) if bound != project_id)
    }
}

fn forbidden() -> AppError {
    AppError::forbidden("FORBIDDEN")
}

fn project_not_found() -> AppError {
    AppError::not_found("PROJECT_NOT_FOUND")
}

fn env_var_not_found() -> AppError {
    AppError::not_found("ENV_VAR_NOT_FOUND")
}

/// 读路径的项目门禁（`assertCanReadProject`）。
async fn assert_can_read_project(
    pool: &PgPool,
    caller: Caller<'_>,
    project_id: &str,
) -> Result<(), AppError> {
    if caller.sk_mismatch(project_id) {
        return Err(forbidden());
    }
    project::get_project_by_id(pool, project_id, caller.user_id, caller.is_admin)
        .await?
        .ok_or_else(project_not_found)?;
    Ok(())
}

/// 写路径的项目门禁：先判存在（不含可见性），再判 `envVar.write` 权限档。
async fn assert_can_write_env_var(
    pool: &PgPool,
    caller: Caller<'_>,
    project_id: &str,
) -> Result<(), AppError> {
    if caller.sk_mismatch(project_id) {
        return Err(forbidden());
    }
    project::get_project_by_id_only(pool, project_id)
        .await?
        .ok_or_else(project_not_found)?;

    let allowed = can_project_action_db(
        pool,
        ProjectAction::EnvVarWrite,
        project_id,
        caller.user_id,
        caller.is_admin,
    )
    .await?;
    if !allowed {
        return Err(forbidden());
    }
    Ok(())
}

// ── 读 ──────────────────────────────────────────────────────────────────

pub async fn list_env_vars(
    pool: &PgPool,
    caller: Caller<'_>,
    project_id: &str,
) -> Result<Vec<ProjectEnvVarRow>, AppError> {
    assert_can_read_project(pool, caller, project_id).await?;
    project_env_var::list_env_vars_by_project(pool, project_id).await
}

pub async fn count_env_vars(
    pool: &PgPool,
    caller: Caller<'_>,
    project_id: &str,
) -> Result<i64, AppError> {
    assert_can_read_project(pool, caller, project_id).await?;
    project_env_var::count_env_vars_by_project(pool, project_id).await
}

/// GET `/by-key/:key` —— 唯一的明文出口。
///
/// **SK 门禁在项目可见性判定之前**：Web JWT 即使访问一个根本不存在的项目，也会先拿到
/// 403 `ENV_VALUE_SK_ONLY` 而不是 404。顺序不能调换，否则 JWT 能借 404/403 差异探测
/// 项目存在性，且对拍会直接出 DIFF。
pub async fn get_env_var_value(
    pool: &PgPool,
    caller: Caller<'_>,
    crypto: EnvCrypto<'_>,
    project_id: &str,
    key: &str,
) -> Result<(ProjectEnvVarRow, String), AppError> {
    if caller.sk_project_id.is_none() {
        return Err(AppError::forbidden("ENV_VALUE_SK_ONLY"));
    }
    assert_can_read_project(pool, caller, project_id).await?;

    let row = project_env_var::get_env_var_by_key(pool, project_id, key)
        .await?
        .ok_or_else(env_var_not_found)?;

    let plain = open_env_var_value(&row.value, crypto.encryption_key, crypto.jwt_secret)?;
    Ok((row, plain))
}

// ── 写 ──────────────────────────────────────────────────────────────────

pub struct CreateEnvVarRequest<'a> {
    pub key: &'a str,
    pub value: &'a str,
    pub description: Option<&'a str>,
    pub is_secret: Option<bool>,
}

/// POST `/`
///
/// 旧实现把 `createEnvVar` 包在 `try/catch` 里，**任何**异常都收敛成 409
/// `ENV_VAR_KEY_EXISTS`（不只是唯一键冲突）。这里照搬这个「过宽」的 catch。
/// 注意 seal 失败（密钥缺失）发生在 try 之外，仍应冒泡成 500——不要一起吞掉。
pub async fn create_env_var(
    pool: &PgPool,
    caller: Caller<'_>,
    crypto: EnvCrypto<'_>,
    project_id: &str,
    req: CreateEnvVarRequest<'_>,
    public_origin: &str,
) -> Result<ProjectEnvVarRow, AppError> {
    assert_can_write_env_var(pool, caller, project_id).await?;

    let key = req.key.trim();
    if !is_valid_env_key(key) {
        return Err(AppError::bad_request("INVALID_ENV_KEY"));
    }

    let is_secret = req.is_secret.unwrap_or(true);
    let stored = seal_env_var_value(req.value, is_secret, crypto.encryption_key, crypto.jwt_secret)?;

    let row = project_env_var::create_env_var(
        pool,
        CreateEnvVarInput {
            project_id,
            key,
            value: &stored,
            description: req.description,
            is_secret,
        },
    )
    .await
    .map_err(|e| {
        tracing::warn!(error = ?e, "create env var failed, mapped to 409");
        AppError::conflict("ENV_VAR_KEY_EXISTS")
    })?;

    log_activity(
        pool,
        project_id,
        caller.user_id,
        ActivityAction::EnvVarCreated,
        LogActivityOptions {
            entity_type: Some("env_var"),
            entity_id: Some(&row.id),
            metadata: Some(audit_meta(&row)),
            description: Some(&format!("创建环境变量 {}", row.key)),
        },
    )
    .await?;

    notify_env_var_changed(pool, public_origin, project_id, caller.user_id, &row.key, "创建")
        .await?;

    Ok(row)
}

#[derive(Default)]
pub struct UpdateEnvVarRequest<'a> {
    pub key: Option<&'a str>,
    pub value: Option<&'a str>,
    /// 三态：`None` 不动、`Some(None)` 写 NULL、`Some(Some(s))` 写值。
    pub description: Option<Option<&'a str>>,
    pub is_secret: Option<bool>,
}

/// PATCH `/:varId`
///
/// **isSecret 翻转要重写存储值**：只改标记不改值时，旧实现会把旧值解密后按新标记重新
/// 封套（true→加密、false→落回明文）。漏掉这步会出现「标记说加密、库里是明文」的
/// 不一致，CLI 之后取值就会解出乱码。
pub async fn update_env_var(
    pool: &PgPool,
    caller: Caller<'_>,
    crypto: EnvCrypto<'_>,
    project_id: &str,
    var_id: &str,
    req: UpdateEnvVarRequest<'_>,
    public_origin: &str,
) -> Result<ProjectEnvVarRow, AppError> {
    assert_can_write_env_var(pool, caller, project_id).await?;

    let existing = project_env_var::get_env_var_by_id(pool, project_id, var_id)
        .await?
        .ok_or_else(env_var_not_found)?;

    let next_key = match req.key {
        Some(k) => {
            let trimmed = k.trim();
            if !is_valid_env_key(trimmed) {
                return Err(AppError::bad_request("INVALID_ENV_KEY"));
            }
            Some(trimmed)
        }
        None => None,
    };

    let next_secret = req.is_secret.unwrap_or(existing.is_secret);
    let next_stored: Option<String> = if let Some(v) = req.value {
        Some(seal_env_var_value(v, next_secret, crypto.encryption_key, crypto.jwt_secret)?)
    } else if next_secret != existing.is_secret {
        let plain = open_env_var_value(&existing.value, crypto.encryption_key, crypto.jwt_secret)?;
        Some(seal_env_var_value(&plain, next_secret, crypto.encryption_key, crypto.jwt_secret)?)
    } else {
        None
    };

    let row = project_env_var::update_env_var_by_id(
        pool,
        project_id,
        var_id,
        UpdateEnvVarPatch {
            key: next_key,
            value: next_stored.as_deref(),
            description: req.description,
            is_secret: req.is_secret,
        },
    )
    .await
    .map_err(|e| {
        tracing::warn!(error = ?e, "update env var failed, mapped to 409");
        AppError::conflict("ENV_VAR_KEY_EXISTS")
    })?
    .ok_or_else(env_var_not_found)?;

    let mut metadata = audit_meta(&row);
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert(
            "previousKey".to_string(),
            serde_json::Value::String(existing.key.clone()),
        );
    }

    log_activity(
        pool,
        project_id,
        caller.user_id,
        ActivityAction::EnvVarUpdated,
        LogActivityOptions {
            entity_type: Some("env_var"),
            entity_id: Some(&row.id),
            metadata: Some(metadata),
            description: Some(&format!("更新环境变量 {}", row.key)),
        },
    )
    .await?;

    notify_env_var_changed(pool, public_origin, project_id, caller.user_id, &row.key, "更新")
        .await?;

    Ok(row)
}

pub async fn delete_env_var(
    pool: &PgPool,
    caller: Caller<'_>,
    project_id: &str,
    var_id: &str,
    public_origin: &str,
) -> Result<(), AppError> {
    assert_can_write_env_var(pool, caller, project_id).await?;

    let existing = project_env_var::get_env_var_by_id(pool, project_id, var_id)
        .await?
        .ok_or_else(env_var_not_found)?;

    project_env_var::delete_env_var_by_id(pool, project_id, var_id).await?;

    log_activity(
        pool,
        project_id,
        caller.user_id,
        ActivityAction::EnvVarDeleted,
        LogActivityOptions {
            entity_type: Some("env_var"),
            entity_id: Some(&existing.id),
            metadata: Some(audit_meta(&existing)),
            description: Some(&format!("删除环境变量 {}", existing.key)),
        },
    )
    .await?;

    notify_env_var_changed(
        pool,
        public_origin,
        project_id,
        caller.user_id,
        &existing.key,
        "删除",
    )
    .await?;

    Ok(())
}

async fn notify_env_var_changed(
    pool: &PgPool,
    public_origin: &str,
    project_id: &str,
    actor_user_id: &str,
    key: &str,
    action: &str,
) -> Result<(), AppError> {
    let recipients = project_member_user_ids(pool, project_id).await?;
    notify(
        pool,
        public_origin,
        NotifyRequest {
            event: "env_var_changed".into(),
            recipient_user_ids: recipients,
            actor_user_id: Some(actor_user_id.to_string()),
            title: format!("项目环境变量已{action}"),
            body: Some(format!("环境变量 {key} 已{action}。")),
            link: Some(format!("/projects/{project_id}/settings/env-vars")),
            email_link: None,
        },
    )
    .await
}

/// `auditMeta`：写进活动日志的元数据，**只记键名与标记，绝不记值**。
fn audit_meta(row: &ProjectEnvVarRow) -> serde_json::Value {
    serde_json::json!({
        "key": row.key,
        "isSecret": row.is_secret,
        "varId": row.id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_key_regex_matches_legacy() {
        for ok in ["A", "API_KEY", "K1", "A_", "Z9_Z"] {
            assert!(is_valid_env_key(ok), "{ok} should be valid");
        }
        for bad in [
            "", "a", "aB", "1A", "_A", "API-KEY", "API KEY", "api_key", "Api", "KEY!", "KÉY",
        ] {
            assert!(!is_valid_env_key(bad), "{bad} should be invalid");
        }
    }

    #[test]
    fn sk_mismatch_only_when_bound_to_other_project() {
        let jwt = Caller { user_id: "u1", is_admin: false, sk_project_id: None };
        assert!(!jwt.sk_mismatch("p1"));

        let sk_same = Caller { user_id: "u1", is_admin: false, sk_project_id: Some("p1") };
        assert!(!sk_same.sk_mismatch("p1"));

        let sk_other = Caller { user_id: "u1", is_admin: false, sk_project_id: Some("p2") };
        assert!(sk_other.sk_mismatch("p1"));
    }
}
