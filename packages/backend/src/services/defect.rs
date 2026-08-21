//! 缺陷域业务服务（1:1 移植自 `packages/backend/src/routes/defect.ts` 的 handler 逻辑）。
//!
//! 权限档与 requirement 一致：**六条端点都只判项目可见性**，没有细粒度动作校验、
//! 没有 owner 成员校验、没有 SK-only 门禁。能看见项目就能读写它的缺陷。
//!
//! 两个值得记下的语义点：
//! 1. `requirementId` 列表过滤是精确匹配且**未 trim**（旧 `filters?.xxx ? {xxx} : {}`），
//!    这个不对称照搬。
//! 2. `convert-to-requirement` 是个事务端点：原子地「建 requirement（origin=defect）+ 回链缺陷
//!    （status=processing）」。已派生（requirementId 非空且需求仍在）则幂等返回既有需求；
//!    状态非 `open` 返回 409 `DEFECT_NOT_CONVERTIBLE`。注意 convert **不写 requirement 的 activity**
//!    （旧实现走 repository 直写，只记 `DEFECT_CONVERTED`），照搬。

use sqlx::PgPool;

use crate::api::AppError;
use crate::repos::defect::{
    self, CreateDefectInput, DefectListFilters, DefectRow, UpdateDefectPatch,
};
use crate::repos::requirement::{self, CreateRequirementInput, RequirementRow};
use crate::services::activity_log::{log_activity, ActivityAction, LogActivityOptions};
use crate::services::project_access::visible_project_id;

/// 缺陷域的失败分支。
#[derive(Debug, Clone, Copy)]
pub enum DefectFailure {
    /// 缺陷不存在（或不属于该项目）→ 404
    DefectNotFound,
    /// 已 processing / resolved / closed，不可再转需求 → 409
    #[allow(dead_code)]
    DefectNotConvertible,
}

impl From<DefectFailure> for AppError {
    fn from(f: DefectFailure) -> Self {
        match f {
            DefectFailure::DefectNotFound => AppError::not_found("DEFECT_NOT_FOUND"),
            DefectFailure::DefectNotConvertible => {
                AppError::conflict("DEFECT_NOT_CONVERTIBLE")
            }
        }
    }
}

/// 严重级别 → 中文标签（用于 convert 生成的需求标题/详情，对齐 `SEVERITY_LABEL`）。
fn severity_label(sev: &str) -> &str {
    match sev {
        "critical" => "致命",
        "major" => "严重",
        "minor" => "一般",
        "trivial" => "轻微",
        _ => sev,
    }
}

/// 从缺陷的 description 生成简短显示标签（用于活动日志等场景）。
/// 优先取 description 的前 50 个字符，无 description 时回退到 ID。
fn defect_display_label(d: &DefectRow) -> String {
    d.description
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            let trimmed = s.trim();
            if trimmed.chars().count() > 50 {
                format!("{}…", trimmed.chars().take(50).collect::<String>())
            } else {
                trimmed.to_string()
            }
        })
        .unwrap_or_else(|| d.id.clone())
}

pub struct ListDefectsQuery<'a> {
    pub status: Option<Vec<&'a str>>,
    pub severity: Option<&'a str>,
    pub requirement_id: Option<&'a str>,
    pub q: Option<&'a str>,
}

/// GET `/projects/:projectId/defects`
pub async fn list_defects(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    query: ListDefectsQuery<'_>,
) -> Result<Vec<DefectRow>, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    defect::list_defects_by_project(
        pool,
        &project_id,
        DefectListFilters {
            status: query.status,
            severity: query.severity,
            requirement_id: query.requirement_id,
            q: query.q,
        },
    )
    .await
}

pub struct CreateDefectArgs<'a> {
    pub description: Option<&'a str>,
    pub status: Option<&'a str>,
    pub severity: Option<&'a str>,
    pub requirement_id: Option<&'a str>,
}

/// POST `/projects/:projectId/defects` → 201
pub async fn create_defect(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    args: CreateDefectArgs<'_>,
) -> Result<DefectRow, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;

    let row = defect::create_defect(
        pool,
        CreateDefectInput {
            project_id: &project_id,
            description: args.description,
            status: args.status,
            severity: args.severity,
            requirement_id: args.requirement_id,
        },
    )
    .await?;

    let desc = defect_display_label(&row);
    log_activity(
        pool,
        &project_id,
        user_id,
        ActivityAction::DefectCreated,
        LogActivityOptions {
            entity_type: Some("defect"),
            entity_id: Some(&row.id),
            description: Some(&format!("创建缺陷 {desc}")),
            ..Default::default()
        },
    )
    .await?;

    Ok(row)
}

/// GET `/projects/:projectId/defects/:defectId`
pub async fn get_defect(
    pool: &PgPool,
    project_id: &str,
    defect_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<DefectRow, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    defect::get_defect_by_id(pool, defect_id, &project_id)
        .await?
        .ok_or_else(|| DefectFailure::DefectNotFound.into())
}

pub struct UpdateDefectArgs<'a> {
    pub description: Option<&'a str>,
    pub status: Option<&'a str>,
    pub severity: Option<&'a str>,
    /// 三态：不传 / 显式 null（清空） / 有值。空串在路由层已摊平成 `None`（对齐 JS `|| null`）。
    pub requirement_id: Option<Option<&'a str>>,
}

/// PATCH `/projects/:projectId/defects/:defectId`
pub async fn update_defect(
    pool: &PgPool,
    project_id: &str,
    defect_id: &str,
    user_id: &str,
    is_admin: bool,
    args: UpdateDefectArgs<'_>,
) -> Result<DefectRow, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;

    let row = defect::update_defect_by_id(
        pool,
        defect_id,
        &project_id,
        UpdateDefectPatch {
            description: args.description,
            status: args.status,
            severity: args.severity,
            requirement_id: args.requirement_id,
        },
    )
    .await?
    .ok_or::<AppError>(DefectFailure::DefectNotFound.into())?;

    let desc = defect_display_label(&row);
    log_activity(
        pool,
        &project_id,
        user_id,
        ActivityAction::DefectUpdated,
        LogActivityOptions {
            entity_type: Some("defect"),
            entity_id: Some(&row.id),
            description: Some(&format!("更新缺陷 {desc}")),
            ..Default::default()
        },
    )
    .await?;

    Ok(row)
}

/// DELETE `/projects/:projectId/defects/:defectId` → `{id}`
pub async fn delete_defect(
    pool: &PgPool,
    project_id: &str,
    defect_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<String, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    let row = defect::delete_defect_by_id(pool, defect_id, &project_id)
        .await?
        .ok_or::<AppError>(DefectFailure::DefectNotFound.into())?;

    log_activity(
        pool,
        &project_id,
        user_id,
        ActivityAction::DefectDeleted,
        LogActivityOptions {
            entity_type: Some("defect"),
            entity_id: Some(&row.id),
            description: Some(&format!("删除缺陷 {}", defect_display_label(&row))),
            ..Default::default()
        },
    )
    .await?;

    Ok(row.id)
}

/// `convert-to-requirement` 的结果分支（映射到状态码）。
pub enum ConvertDefectResult {
    NotFound,
    NotConvertible,
    Ok { requirement: RequirementRow, defect: DefectRow },
}

/// POST `/projects/:projectId/defects/:defectId/convert-to-requirement`
///
/// 事务保证原子性：查缺陷 →（幂等返回 / 409）/ 建 requirement（origin=defect）→ 回链缺陷
/// （status=processing）。注意本函数**只做 DB 写入、不记 activity**，调用方在事务提交成功后
/// 再记 `DEFECT_CONVERTED`（对齐旧实现 convert 在 `$transaction` 回调外才 logActivity）。
pub async fn convert_defect_to_requirement(
    pool: &PgPool,
    defect_id: &str,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<ConvertDefectResult, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;

    let mut tx = pool.begin().await?;
    // 1. 查缺陷（事务内）
    let defect = match defect::get_defect_by_id(&mut *tx, defect_id, &project_id).await? {
        Some(d) => d,
        None => {
            // 无写入；tx 随函数返回而 Drop 自动回滚。
            return Ok(ConvertDefectResult::NotFound);
        }
    };

    // 2. 已派生且 requirement 仍存在 → 幂等返回既有需求
    //    注意用**全局** findUnique（不带 projectId）：缺陷的 requirementId 可由 create/patch
    //    写成别的项目的需求 ID，旧实现照样命中并幂等返回，加 projectId 会改变行为。
    if let Some(rid) = defect.requirement_id.as_deref() {
        if let Some(req) = requirement::find_requirement_by_id_global(&mut *tx, rid).await? {
            tx.commit().await?;
            return Ok(ConvertDefectResult::Ok {
                requirement: req,
                defect,
            });
        }
        // requirement 已被删：旧实现 `if (existing) return` 未满足，落到下方重新创建
    }

    // 3. 状态非 open → 409（无写入，tx Drop 自动回滚）
    if defect.status != "open" {
        return Ok(ConvertDefectResult::NotConvertible);
    }

    // 4. 生成修复需求文案
    let label = severity_label(&defect.severity);
    let display = defect_display_label(&defect);
    let title = format!("[来自缺陷·{label}] {display}");
    let mut source_parts: Vec<String> = vec![
        format!("原缺陷 ID：{}", defect.id),
        format!("严重级别：{label}"),
    ];
    if let Some(desc) = defect.description.as_ref().filter(|s| !s.trim().is_empty()) {
        source_parts.push(format!("详情：\n{}", desc.trim()));
    }
    let source_text = source_parts.join("\n\n");

    // 5. 建 requirement（origin=defect，status=pending，coverage=none）
    let requirement = requirement::create_requirement(
        &mut *tx,
        CreateRequirementInput {
            project_id: &project_id,
            repository_id: None,
            description: &title,
            source_text: Some(&source_text),
            client_notes: None,
            status: Some("pending"),
            coverage: Some("none"),
            origin: Some("defect"),
            owner_id: None,
        },
    )
    .await?;

    // 6. 回链缺陷
    defect::link_defect_to_requirement(&mut *tx, defect_id, &requirement.id).await?;

    tx.commit().await?;
    Ok(ConvertDefectResult::Ok {
        requirement,
        defect,
    })
}
