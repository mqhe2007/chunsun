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
        "createdBy": r.created_by,
        "creator": r.creator.as_ref().map_or(Value::Null, |c| json!({
            "id": c.id,
            "nickname": c.nickname,
            "qq": c.qq,
            "email": c.email,
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
        "createdBy": d.created_by,
        "creator": d.creator.as_ref().map_or(Value::Null, |c| json!({
            "id": c.id,
            "nickname": c.nickname,
            "qq": c.qq,
            "email": c.email,
        })),
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
        "loadStrategy": d.load_strategy,
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
/// （只有 `key/title/content/system/loadStrategy` 五个字段，**没有** `sortOrder`/`updatedAt`）。
///
/// 宪法的 loadStrategy 恒为 'eager'（系统固定项，启动时必须加载）。
pub fn knowledge_item_dto(key: &str, title: &str, content: &str, system: bool, load_strategy: &str) -> Value {
    json!({
        "key": key,
        "title": title,
        "content": content,
        "system": system,
        "loadStrategy": load_strategy,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::defect::{DefectCreator, DefectRequirementLink};
    use crate::repos::requirement::{RequirementCreator, RequirementOwner};

    fn dt(secs: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0).expect("valid ts")
    }

    fn base_requirement_row() -> RequirementRow {
        RequirementRow {
            id: "req_1".into(),
            project_id: "p_1".into(),
            repository_id: None,
            description: "需求描述".into(),
            source_text: None,
            client_notes: None,
            status: "pending".into(),
            coverage: "none".into(),
            origin: "manual".into(),
            owner_id: None,
            created_by: None,
            released_at: None,
            created_at: dt(0),
            updated_at: dt(0),
            owner: None,
            creator: None,
        }
    }

    #[test]
    fn requirement_dto_emits_creator_when_present() {
        let mut row = base_requirement_row();
        row.created_by = Some("u_1".into());
        row.creator = Some(RequirementCreator {
            id: "u_1".into(),
            nickname: Some("alice".into()),
            qq: Some("12345".into()),
            email: Some("alice@example.com".into()),
        });

        let v = requirement_dto(&row);
        assert_eq!(v["createdBy"], "u_1");
        assert_eq!(v["creator"]["id"], "u_1");
        assert_eq!(v["creator"]["nickname"], "alice");
        assert_eq!(v["creator"]["qq"], "12345");
        assert_eq!(v["creator"]["email"], "alice@example.com");
    }

    #[test]
    fn requirement_dto_creator_is_null_when_absent() {
        let row = base_requirement_row();
        let v = requirement_dto(&row);
        assert_eq!(v["createdBy"], Value::Null);
        assert_eq!(v["creator"], Value::Null);
    }

    #[test]
    fn requirement_dto_owner_and_creator_are_independent() {
        let mut row = base_requirement_row();
        row.owner_id = Some("u_2".into());
        row.owner = Some(RequirementOwner {
            id: "u_2".into(),
            nickname: Some("bob".into()),
            qq: None,
        });
        row.created_by = Some("u_1".into());
        row.creator = Some(RequirementCreator {
            id: "u_1".into(),
            nickname: None,
            qq: None,
            email: Some("alice@example.com".into()),
        });

        let v = requirement_dto(&row);
        assert_eq!(v["owner"]["id"], "u_2");
        assert_eq!(v["owner"]["nickname"], "bob");
        assert_eq!(v["creator"]["id"], "u_1");
        assert_eq!(v["creator"]["email"], "alice@example.com");
    }

    #[test]
    fn defect_dto_emits_creator_when_present() {
        let row = DefectRow {
            id: "def_1".into(),
            project_id: "p_1".into(),
            description: Some("缺陷描述".into()),
            status: "open".into(),
            severity: "major".into(),
            requirement_id: None,
            created_by: Some("u_1".into()),
            created_at: dt(0),
            updated_at: dt(0),
            requirement: Some(DefectRequirementLink {
                id: "req_1".into(),
                description: "需求描述".into(),
                status: "pending".into(),
            }),
            creator: Some(DefectCreator {
                id: "u_1".into(),
                nickname: Some("alice".into()),
                qq: None,
                email: Some("alice@example.com".into()),
            }),
        };

        let v = defect_dto(&row);
        assert_eq!(v["createdBy"], "u_1");
        assert_eq!(v["creator"]["id"], "u_1");
        assert_eq!(v["creator"]["nickname"], "alice");
        assert_eq!(v["creator"]["email"], "alice@example.com");
        // requirement 关联不受 creator 影响
        assert_eq!(v["requirement"]["id"], "req_1");
    }

    #[test]
    fn defect_dto_creator_is_null_when_absent() {
        let row = DefectRow {
            id: "def_1".into(),
            project_id: "p_1".into(),
            description: None,
            status: "open".into(),
            severity: "minor".into(),
            requirement_id: None,
            created_by: None,
            created_at: dt(0),
            updated_at: dt(0),
            requirement: None,
            creator: None,
        };
        let v = defect_dto(&row);
        assert_eq!(v["createdBy"], Value::Null);
        assert_eq!(v["creator"], Value::Null);
    }
}
