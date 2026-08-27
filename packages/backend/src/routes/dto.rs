//! 跨路由复用的响应序列化（对齐旧后端各 `serializeXxx` 函数）。
//!
//! 同一实体在不同路由里必须序列化成**同一形状**：旧后端靠 import 同一个
//! `serializeRepository` 保证，这里靠本模块保证。字段名、顺序、时间格式都以
//! 旧实现为准，不要在调用点就地拼 json。

use serde_json::{json, Value};

use crate::core::activity_heatmap::HeatmapEntry;
use crate::core::datetime::to_value as dt_value;
use crate::core::env_var_crypto::env_var_has_stored_value;
use crate::repos::defect::DefectRow;
use crate::repos::project::ProjectRow;
use crate::repos::project_activity::ProjectActivityRow;
use crate::repos::project_knowledge::KnowledgeDocRow;
use crate::repos::project_env_var::ProjectEnvVarRow;
use crate::repos::prompt::PromptRow;
use crate::repos::repository::RepositoryRow;
use crate::repos::requirement::RequirementRow;

/// `serializeProject`：注意 `secretKey` 本身不外泄，只暴露 `hasSecretKey`。
pub fn project_dto(p: &ProjectRow) -> Value {
    json!({
        "id": p.id,
        "userId": p.user_id,
        "name": p.name,
        "description": p.description,
        "hasSecretKey": p.secret_key.is_some(),
        "createdAt": dt_value(&p.created_at),
        "updatedAt": dt_value(&p.updated_at),
    })
}

/// `serializeRepository` = `toRepositorySummary(...)` 再补两个时间字段。
///
/// project 详情内嵌的 `repositories[]` 与 repository 路由的返回体共用此形状。
pub fn repository_dto(r: &RepositoryRow) -> Value {
    json!({
        "id": r.id,
        "projectId": r.project_id,
        "name": r.name,
        "slug": r.slug,
        "rootHint": r.root_hint,
        "isDefault": r.is_default,
        "createdAt": dt_value(&r.created_at),
        "updatedAt": dt_value(&r.updated_at),
    })
}

pub fn prompt_dto(p: &PromptRow) -> Value {
    json!({
        "systemPrompt": p.system_prompt,
        "userPromptTemplate": p.user_prompt_template,
    })
}

/// `serializeRequirement`。
///
/// 两个易错点：
/// - `origin` 在旧实现里是 `r.origin ?? "manual"`，DB 有非空默认值，这里等价直出。
/// - `owner` 是 `r.owner ?? null`——`create` 路径的行没有 owner 关联，序列化后是
///   `null`，**即使 `ownerId` 有值**。这个不对称是旧实现的既有行为，不要「顺手修好」。
pub fn requirement_dto(r: &RequirementRow) -> Value {
    json!({
        "id": r.id,
        "projectId": r.project_id,
        "repositoryId": r.repository_id,
        "description": r.description,
        "sourceText": r.source_text,
        "clientNotes": r.client_notes,
        "status": r.status,
        "coverage": r.coverage,
        "origin": r.origin,
        "ownerId": r.owner_id,
        "owner": r.owner.as_ref().map_or(Value::Null, |o| json!({
            "id": o.id,
            "nickname": o.nickname,
            "qq": o.qq,
        })),
        "releasedAt": r.released_at.as_ref().map_or(Value::Null, dt_value),
        "createdAt": dt_value(&r.created_at),
        "updatedAt": dt_value(&r.updated_at),
    })
}

/// `serializeDefect`：注意 requirement 关联只取 3 个字段（id/description/status），
/// 与列表、详情、写回共用此形状；不存在时为 `null`（不是空对象）。
pub fn defect_dto(d: &DefectRow) -> Value {
    json!({
        "id": d.id,
        "projectId": d.project_id,
        "description": d.description,
        "status": d.status,
        "severity": d.severity,
        "requirementId": d.requirement_id,
        "createdAt": dt_value(&d.created_at),
        "updatedAt": dt_value(&d.updated_at),
        "requirement": d.requirement.as_ref().map_or(Value::Null, |r| json!({
            "id": r.id,
            "description": r.description,
            "status": r.status,
        })),
    })
}

/// `serializeEnvVarListItem`：清单/写回形状，**`value` 恒为 null**。
///
/// Web 与 CLI 在这个形状上同权——想拿明文只能走 `by-key`（且仅 SK 通道）。
/// `hasValue` 不解密判断：加密封套一律视为有值，明文按是否空串。
/// 字段顺序照抄旧实现（id/key/description/isSecret/value/hasValue/时间），
/// 虽然 JSON 对象无序，但对拍是逐字节比对响应体的。
pub fn env_var_list_item_dto(e: &ProjectEnvVarRow) -> Value {
    json!({
        "id": e.id,
        "key": e.key,
        "description": e.description,
        "isSecret": e.is_secret,
        "value": Value::Null,
        "hasValue": env_var_has_stored_value(&e.value),
        "createdAt": dt_value(&e.created_at),
        "updatedAt": dt_value(&e.updated_at),
    })
}

/// `serializeEnvVarValue`：唯一携带明文的形状，**没有 `hasValue` 字段**。
/// 明文由调用方解密后传入，DTO 层不碰密钥。
pub fn env_var_value_dto(e: &ProjectEnvVarRow, plain: &str) -> Value {
    json!({
        "id": e.id,
        "key": e.key,
        "value": plain,
        "description": e.description,
        "isSecret": e.is_secret,
        "createdAt": dt_value(&e.created_at),
        "updatedAt": dt_value(&e.updated_at),
    })
}

/// 项目活动清单项：旧实现在路由里手写 `rows.map(r => ({...}))`，**没有 `projectId`/`userId`**，
/// 只有折叠后的 `user` 对象（id/nickname/qq 三个 select 字段）。
///
/// `metadata` 是 `Json?`，原样透传（无值为 `null`）。
pub fn activity_dto(a: &ProjectActivityRow) -> Value {
    json!({
        "id": a.id,
        "action": a.action,
        "entityType": a.entity_type,
        "entityId": a.entity_id,
        "description": a.description,
        "metadata": a.metadata,
        "createdAt": dt_value(&a.created_at),
        "user": {
            "id": a.user.id,
            "nickname": a.user.nickname,
            "qq": a.user.qq,
        },
    })
}

/// `serializeContextDocument`：**不含 `createdAt`**，只回 `updatedAt`。
///
/// 排序用的 `createdAt` 只在列表 `ORDER BY` 里出现，不外泄到响应里。
pub fn knowledge_doc_dto(d: &KnowledgeDocRow) -> Value {
    json!({
        "id": d.id,
        "title": d.title,
        "content": d.content,
        "sortOrder": d.sort_order,
        "updatedAt": dt_value(&d.updated_at),
    })
}

/// `PUT /contexts/constitution` 的响应：形状**不同于**列表里的宪法条目——
/// 多一个 `updatedAt`，因为这里拿得到 `project_policy` 行。
pub fn constitution_dto(constitution_md: &str, updated_at: &chrono::DateTime<chrono::Utc>) -> Value {
    json!({
        "key": "constitution",
        "title": "项目宪法",
        "content": constitution_md,
        "system": true,
        "updatedAt": dt_value(updated_at),
    })
}

/// `ProjectContextItem`：宪法与自定义文档在列表里被抹平成同一形状
/// （只有 `key/title/content/system` 四个字段，**没有** `sortOrder`/`updatedAt`）。
pub fn knowledge_item_dto(key: &str, title: &str, content: &str, system: bool) -> Value {
    json!({
        "key": key,
        "title": title,
        "content": content,
        "system": system,
    })
}

/// 活跃图响应体：`{ windowDays, max, entries: [{ date, count }] }`。
pub fn heatmap_dto(window_days: u32, max: u32, entries: &[HeatmapEntry]) -> Value {
    json!({
        "windowDays": window_days,
        "max": max,
        "entries": entries
            .iter()
            .map(|e| json!({ "date": e.date, "count": e.count }))
            .collect::<Vec<_>>(),
    })
}
