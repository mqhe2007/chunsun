//! 项目知识聚合（对齐 `projectKnowledgeDocumentRepository.listProjectKnowledge`）。
//!
//! 单独成一层是因为 `listProjectKnowledge` 被**两个域**共用：
//! `GET /projects/:id/knowledge`（项目概览，第 12 域）与知识文档 CRUD（第 11 域）。
//! 旧后端靠 import 同一个函数保证两处一致，这里靠本模块保证。

use serde_json::Value;
use sqlx::PgPool;

use crate::api::AppError;
use crate::repos::project_knowledge::{get_project_policy, list_knowledge_documents};
use crate::routes::dto::knowledge_item_dto;

/// 宪法置顶 + 自定义文档（`sortOrder asc, createdAt desc`）。
///
/// 两个细节容易漏：
/// 1. **宪法恒定出现**，哪怕 `project_policy` 里根本没有这一行——
///    `policy?.constitutionMd ?? ""` 会退化成空串，而不是跳过该条目。
/// 2. 文档条目的 `key` 是**文档 id**，不是什么 slug；`system` 恒为 `false`。
///
/// `strategy` 为 None 时返回全部（含宪法）；为 Some("eager") 时只返回 eager 文档
/// （宪法恒为 eager，始终包含）；为 Some("lazy") 时只返回 lazy 文档（不含宪法）。
pub async fn list_project_knowledge(
    pool: &PgPool,
    project_id: &str,
    strategy: Option<&str>,
) -> Result<Vec<Value>, AppError> {
    let policy = get_project_policy(pool, project_id).await?;
    let docs = list_knowledge_documents(pool, project_id, strategy).await?;

    let constitution = policy.as_ref().map_or("", |p| p.constitution_md.as_str());
    let mut items = Vec::with_capacity(docs.len() + 1);

    // 宪法恒为 eager；当 strategy=lazy 时不包含宪法
    if strategy != Some("lazy") {
        items.push(knowledge_item_dto("constitution", "项目宪法", constitution, true, "eager"));
    }
    for doc in &docs {
        items.push(knowledge_item_dto(&doc.id, &doc.title, &doc.content, false, &doc.load_strategy));
    }
    Ok(items)
}
