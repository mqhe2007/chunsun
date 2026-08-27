//! 通知服务：分类偏好 + 统一派发（站内信 / 邮件）。

use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::api::AppError;
use crate::repos::notification::{create_notification, NotificationInput};
use crate::repos::notification_preference::{
    clear_overrides, delete_override, list_overrides, upsert_override,
};
use crate::repos::user;
use crate::services::email;
use crate::services::settings;

/// 通知分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotifyCategory {
    Security,
    Membership,
    Delivery,
    Defect,
    Project,
}

impl NotifyCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Security => "security",
            Self::Membership => "membership",
            Self::Delivery => "delivery",
            Self::Defect => "defect",
            Self::Project => "project",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Security => "安全",
            Self::Membership => "项目成员",
            Self::Delivery => "需求与轮次",
            Self::Defect => "缺陷",
            Self::Project => "项目变更",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Security => "密码、邮箱验证、账号锁定、项目密钥等安全相关通知",
            Self::Membership => "项目邀请、角色变更、成员移出与离开",
            Self::Delivery => "需求负责人、轮次需决策与终态、需求重置",
            Self::Defect => "缺陷新建、状态变更、转需求与自动关闭",
            Self::Project => "项目信息变更、删除与环境变量变更",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "security" => Some(Self::Security),
            "membership" => Some(Self::Membership),
            "delivery" => Some(Self::Delivery),
            "defect" => Some(Self::Defect),
            "project" => Some(Self::Project),
            _ => None,
        }
    }

    pub const ALL: [Self; 5] = [
        Self::Security,
        Self::Membership,
        Self::Delivery,
        Self::Defect,
        Self::Project,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotifyChannel {
    InApp,
    Email,
}

impl NotifyChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InApp => "in_app",
            Self::Email => "email",
        }
    }
}

/// 事件类型 → 分类。
pub fn category_for_event(event: &str) -> Option<NotifyCategory> {
    match event {
        "password_changed"
        | "email_verified"
        | "account_locked"
        | "secret_key_regenerated"
        | "secret_key_revoked" => Some(NotifyCategory::Security),
        "project_invitation" | "member_role_changed" | "member_removed" | "member_left" => {
            Some(NotifyCategory::Membership)
        }
        "requirement_owner_changed"
        | "run_needs_decision"
        | "run_completed"
        | "run_finished"
        | "run_abandoned"
        | "requirement_reset" => Some(NotifyCategory::Delivery),
        "defect_created"
        | "defect_status_changed"
        | "defect_converted"
        | "defect_auto_resolved" => Some(NotifyCategory::Defect),
        "project_updated" | "project_deleted" | "env_var_changed" => Some(NotifyCategory::Project),
        _ => None,
    }
}

/// 默认矩阵（无 override 时）。
pub fn default_channel_enabled(category: NotifyCategory, channel: NotifyChannel) -> bool {
    match (category, channel) {
        (NotifyCategory::Security, _) => true,
        (NotifyCategory::Membership | NotifyCategory::Delivery | NotifyCategory::Defect, NotifyChannel::InApp) => {
            true
        }
        (NotifyCategory::Membership | NotifyCategory::Delivery | NotifyCategory::Defect, NotifyChannel::Email) => {
            false
        }
        (NotifyCategory::Project, _) => false,
    }
}

/// 安全类站内信不可关。
pub fn is_channel_locked(category: NotifyCategory, channel: NotifyChannel) -> bool {
    matches!(category, NotifyCategory::Security) && matches!(channel, NotifyChannel::InApp)
}

/// 永不走邮件的事件（即使安全类邮件开着）。
fn event_blocks_email(event: &str) -> bool {
    event == "email_verified"
}

/// 协作类跳过 actor==recipient；安全类仍发给本人。
fn should_skip_self(event: &str, actor_id: Option<&str>, recipient_id: &str) -> bool {
    let Some(actor) = actor_id else {
        return false;
    };
    if actor != recipient_id {
        return false;
    }
    !matches!(category_for_event(event), Some(NotifyCategory::Security))
}

pub fn is_smtp_delivery_available(config: &settings::SmtpConfig) -> bool {
    !config.host.is_empty() && !config.user.is_empty() && !config.from_address.is_empty()
}

#[derive(Debug, Clone)]
pub struct ChannelState {
    pub enabled: bool,
    pub locked: bool,
}

#[derive(Debug, Clone)]
pub struct CategoryEffective {
    pub category: NotifyCategory,
    pub in_app: ChannelState,
    pub email: ChannelState,
}

fn merge_effective(
    category: NotifyCategory,
    overrides: &HashMap<(String, String), bool>,
) -> CategoryEffective {
    let in_app_default = default_channel_enabled(category, NotifyChannel::InApp);
    let email_default = default_channel_enabled(category, NotifyChannel::Email);
    let in_app_locked = is_channel_locked(category, NotifyChannel::InApp);
    let email_locked = is_channel_locked(category, NotifyChannel::Email);

    let mut in_app = overrides
        .get(&(category.as_str().to_string(), NotifyChannel::InApp.as_str().to_string()))
        .copied()
        .unwrap_or(in_app_default);
    if in_app_locked {
        in_app = true;
    }
    let email = overrides
        .get(&(category.as_str().to_string(), NotifyChannel::Email.as_str().to_string()))
        .copied()
        .unwrap_or(email_default);

    CategoryEffective {
        category,
        in_app: ChannelState {
            enabled: in_app,
            locked: in_app_locked,
        },
        email: ChannelState {
            enabled: email,
            locked: email_locked,
        },
    }
}

async fn load_override_map(
    pool: &PgPool,
    user_id: &str,
) -> Result<HashMap<(String, String), bool>, AppError> {
    let rows = list_overrides(pool, user_id).await?;
    Ok(rows
        .into_iter()
        .map(|r| ((r.category, r.channel), r.enabled))
        .collect())
}

pub async fn effective_for_user(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<CategoryEffective>, AppError> {
    let map = load_override_map(pool, user_id).await?;
    Ok(NotifyCategory::ALL
        .into_iter()
        .map(|c| merge_effective(c, &map))
        .collect())
}

async fn channels_for_event(
    pool: &PgPool,
    user_id: &str,
    event: &str,
) -> Result<(bool, bool), AppError> {
    let Some(category) = category_for_event(event) else {
        // 未知事件：保守只发站内信
        return Ok((true, false));
    };
    let map = load_override_map(pool, user_id).await?;
    let eff = merge_effective(category, &map);
    let email = eff.email.enabled && !event_blocks_email(event);
    Ok((eff.in_app.enabled, email))
}

#[derive(Debug, Clone)]
pub struct NotifyRequest {
    pub event: String,
    pub recipient_user_ids: Vec<String>,
    pub actor_user_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    /// 站内信相对路径
    pub link: Option<String>,
    /// 邮件绝对 URL；缺省时用 public_origin + /console + link
    pub email_link: Option<String>,
}

/// 统一派发：查偏好 → 写站内信 / 发邮件。失败不阻断主流程（站内信写入失败仍上抛）。
pub async fn notify(
    pool: &PgPool,
    public_origin: &str,
    req: NotifyRequest,
) -> Result<(), AppError> {
    let mut seen = HashSet::new();
    for recipient_id in &req.recipient_user_ids {
        if recipient_id.is_empty() || !seen.insert(recipient_id.clone()) {
            continue;
        }
        if should_skip_self(&req.event, req.actor_user_id.as_deref(), recipient_id) {
            continue;
        }

        let (in_app, want_email) = channels_for_event(pool, recipient_id, &req.event).await?;

        if in_app {
            create_notification(
                pool,
                NotificationInput {
                    user_id: recipient_id.clone(),
                    ty: req.event.clone(),
                    title: req.title.clone(),
                    body: req.body.clone(),
                    link: req.link.clone(),
                },
            )
            .await?;
        }

        if want_email {
            let Some(u) = user::get_user_by_id(pool, recipient_id).await? else {
                continue;
            };
            let link = req.email_link.clone().or_else(|| {
                req.link.as_ref().map(|rel| {
                    let path = if rel.starts_with('/') {
                        rel.clone()
                    } else {
                        format!("/{rel}")
                    };
                    format!("{public_origin}/console{path}")
                })
            });
            let email_body = req.body.clone().unwrap_or_default();
            let link_str = link.unwrap_or_else(|| public_origin.to_string());
            email::send_notification_email(
                pool,
                &u.email,
                &req.title,
                &email_body,
                &link_str,
            )
            .await;
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryPatch {
    pub in_app: Option<bool>,
    pub email: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesPatch {
    pub categories: HashMap<String, CategoryPatch>,
}

pub async fn preferences_dto(pool: &PgPool, user_id: &str) -> Result<Value, AppError> {
    let effective = effective_for_user(pool, user_id).await?;
    let smtp = settings::get_smtp_config(pool).await?;
    let email_available = is_smtp_delivery_available(&smtp);
    let categories: Vec<Value> = effective
        .into_iter()
        .map(|c| {
            json!({
                "key": c.category.as_str(),
                "label": c.category.label(),
                "description": c.category.description(),
                "inApp": {
                    "enabled": c.in_app.enabled,
                    "locked": c.in_app.locked,
                },
                "email": {
                    "enabled": c.email.enabled,
                    "locked": c.email.locked || !email_available,
                },
            })
        })
        .collect();
    Ok(json!({
        "categories": categories,
        "emailDeliveryAvailable": email_available,
    }))
}

pub async fn patch_preferences(
    pool: &PgPool,
    user_id: &str,
    patch: PreferencesPatch,
) -> Result<Value, AppError> {
    for (key, cat_patch) in &patch.categories {
        let Some(category) = NotifyCategory::parse(key) else {
            return Err(AppError::unprocessable("VALIDATION_ERROR")
                .with_message(format!("未知通知分类: {key}")));
        };
        if let Some(enabled) = cat_patch.in_app {
            if is_channel_locked(category, NotifyChannel::InApp) {
                if !enabled {
                    return Err(AppError::bad_request("SECURITY_IN_APP_LOCKED")
                        .with_message("安全类站内信不可关闭"));
                }
                // 强制开：删除可能存在的错误覆盖
                delete_override(pool, user_id, category.as_str(), NotifyChannel::InApp.as_str())
                    .await?;
            } else {
                apply_channel_override(
                    pool,
                    user_id,
                    category,
                    NotifyChannel::InApp,
                    enabled,
                )
                .await?;
            }
        }
        if let Some(enabled) = cat_patch.email {
            apply_channel_override(pool, user_id, category, NotifyChannel::Email, enabled).await?;
        }
    }
    preferences_dto(pool, user_id).await
}

async fn apply_channel_override(
    pool: &PgPool,
    user_id: &str,
    category: NotifyCategory,
    channel: NotifyChannel,
    enabled: bool,
) -> Result<(), AppError> {
    let default = default_channel_enabled(category, channel);
    if enabled == default {
        delete_override(pool, user_id, category.as_str(), channel.as_str()).await?;
    } else {
        upsert_override(pool, user_id, category.as_str(), channel.as_str(), enabled).await?;
    }
    Ok(())
}

pub async fn reset_preferences(pool: &PgPool, user_id: &str) -> Result<Value, AppError> {
    clear_overrides(pool, user_id).await?;
    preferences_dto(pool, user_id).await
}

/// snapshot 中 openDecisions 数组长度。
pub fn open_decisions_len(snapshot: &Value) -> usize {
    snapshot
        .get("openDecisions")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

/// 需求通知收件人：owner，否则项目创建者。
pub async fn delivery_recipient(
    pool: &PgPool,
    requirement_id: &str,
    project_id: &str,
) -> Result<Option<String>, AppError> {
    use crate::repos::{project, requirement};
    if let Some(req) = requirement::get_requirement_by_id(pool, requirement_id, project_id).await? {
        if let Some(owner) = req.owner_id {
            return Ok(Some(owner));
        }
    }
    Ok(project::get_project_by_id_only(pool, project_id)
        .await?
        .map(|p| p.user_id))
}

/// 缺陷通知收件人：关联需求 owner → 项目创建者。
pub async fn defect_recipient(
    pool: &PgPool,
    project_id: &str,
    requirement_id: Option<&str>,
) -> Result<Option<String>, AppError> {
    if let Some(rid) = requirement_id {
        if let Some(uid) = delivery_recipient(pool, rid, project_id).await? {
            return Ok(Some(uid));
        }
    }
    use crate::repos::project;
    Ok(project::get_project_by_id_only(pool, project_id)
        .await?
        .map(|p| p.user_id))
}

/// 项目全体成员 user_id（含 OWNER 行）。
pub async fn project_member_user_ids(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<String>, AppError> {
    use crate::repos::project_member;
    let members = project_member::list_project_members(pool, project_id).await?;
    Ok(members.into_iter().map(|m| m.user_id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_agreed_matrix() {
        assert!(default_channel_enabled(NotifyCategory::Security, NotifyChannel::InApp));
        assert!(default_channel_enabled(NotifyCategory::Security, NotifyChannel::Email));
        assert!(default_channel_enabled(NotifyCategory::Membership, NotifyChannel::InApp));
        assert!(!default_channel_enabled(NotifyCategory::Membership, NotifyChannel::Email));
        assert!(!default_channel_enabled(NotifyCategory::Project, NotifyChannel::InApp));
        assert!(!default_channel_enabled(NotifyCategory::Project, NotifyChannel::Email));
    }

    #[test]
    fn security_in_app_locked() {
        assert!(is_channel_locked(NotifyCategory::Security, NotifyChannel::InApp));
        assert!(!is_channel_locked(NotifyCategory::Security, NotifyChannel::Email));
        assert!(!is_channel_locked(NotifyCategory::Delivery, NotifyChannel::InApp));
    }

    #[test]
    fn email_verified_blocks_email() {
        assert!(event_blocks_email("email_verified"));
        assert!(!event_blocks_email("password_changed"));
    }

    #[test]
    fn skip_self_only_for_collab() {
        assert!(should_skip_self("defect_created", Some("u1"), "u1"));
        assert!(!should_skip_self("password_changed", Some("u1"), "u1"));
        assert!(!should_skip_self("defect_created", Some("u1"), "u2"));
    }

    #[test]
    fn category_mapping_covers_catalog() {
        for e in [
            "password_changed",
            "email_verified",
            "account_locked",
            "secret_key_regenerated",
            "secret_key_revoked",
            "project_invitation",
            "member_role_changed",
            "member_removed",
            "member_left",
            "requirement_owner_changed",
            "run_needs_decision",
            "run_completed",
            "run_finished",
            "run_abandoned",
            "requirement_reset",
            "defect_created",
            "defect_status_changed",
            "defect_converted",
            "defect_auto_resolved",
            "project_updated",
            "project_deleted",
            "env_var_changed",
        ] {
            assert!(category_for_event(e).is_some(), "missing {e}");
        }
    }

    #[test]
    fn open_decisions_len_reads_array() {
        assert_eq!(open_decisions_len(&json!({})), 0);
        assert_eq!(
            open_decisions_len(&json!({ "openDecisions": [{}, {}] })),
            2
        );
    }

    #[test]
    fn merge_forces_security_in_app() {
        let mut map = HashMap::new();
        map.insert(("security".into(), "in_app".into()), false);
        let eff = merge_effective(NotifyCategory::Security, &map);
        assert!(eff.in_app.enabled);
        assert!(eff.in_app.locked);
    }
}
