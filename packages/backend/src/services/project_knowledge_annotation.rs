//! 批注 → Agent 提示词的注入聚合（需求 u-WPvdvYh4Fw 第 3 步，方案 A）。
//!
//! **方案 A 的要点**：批注不经过 harness 技能模板，而是由后端折叠进知识读取响应。
//! 这样 CLI（`chunsun knowledge` / `chunsun knowledge doc`）与控制台阅读页拿到的是
//! **同一份数据**，不存在"Agent 看到的批注"和"人看到的批注"两套真相。
//! 模板侧零改动是硬约束——模板版本号是校验过的，动它会让存量项目全部失配。
//!
//! 两种形态，按端点的用途分：
//!
//! | 端点 | 形态 | 理由 |
//! | --- | --- | --- |
//! | `GET /knowledge`、`GET /knowledge/documents?strategy=` | **摘要** | Agent 一上来就拉全量知识，批注必须省着给 |
//! | `GET /knowledge/documents/:docId`、`GET /knowledge/constitution` | **全量** | 单篇深读，正是要改这篇文档，批注就是改动清单 |
//!
//! **只注入 open**。resolved 不进提示词：它已经被处理过，再进 prompt 纯挤上下文，
//! 而且会让 Agent 重复处理同一件事。阅读页与编辑态仍然看得到 resolved
//! （默认折叠但可见）——"不进 prompt"和"对人不可见"是两件事，不要混。

use sqlx::PgPool;

use crate::api::AppError;
use crate::repos::project_knowledge_annotation as ann_repo;

/// 摘要形态里 `body` 的截断长度（字符数，非字节）。
///
/// 取 80 是因为批注的典型形态是一两句话；再长就不是"提醒"，而是要求 Agent 展开，
/// 那种情况应该走单文档全量形态。截断后附 `…` 并提示展开手段，避免 Agent 误以为
/// 批注就到这里为止。
const SUMMARY_BODY_LIMIT: usize = 80;

/// 摘要形态里锚点文本的截断长度。锚点只用来让人/Agent 认出"说的是哪一段"，
/// 不需要全文。
const SUMMARY_ANCHOR_LIMIT: usize = 40;

/// 单篇注入的**总体积上限**（字符数）。
///
/// 存在的理由：eager 文档的正文本来就已全量进 prompt，如果一篇文档上挂着几十条长批注，
/// 叠加后会把上下文吃光。超限时截断并显式告知被截断了几条，**不做静默丢弃**——
/// 静默丢弃会让 Agent 以为自己已经看到全部批注，反而制造新的漏改。
pub const TOTAL_LIMIT: usize = 8000;

/// 单条批注的紧凑形态（供 eager 列表注入）。
fn summarize(row: &ann_repo::KnowledgeAnnotationRow) -> serde_json::Value {
    serde_json::json!({
        "id": row.id,
        "anchor": row.anchor_text.as_deref().map(|a| truncate(a, SUMMARY_ANCHOR_LIMIT)),
        "body": truncate(&row.body, SUMMARY_BODY_LIMIT),
        "status": row.status,
        "createdBy": row.created_by,
    })
}

/// 全量形态：单篇深读时给足信息，含锚点上下文（Agent 要靠 prefix/suffix 判断锚点位置）
/// 与作者（Agent 需要知道"这是谁的意见"，尤其涉及职责边界时）。
fn detailed(row: &ann_repo::KnowledgeAnnotationRow) -> serde_json::Value {
    serde_json::json!({
        "id": row.id,
        "anchorText": row.anchor_text,
        "anchorPrefix": row.anchor_prefix,
        "anchorSuffix": row.anchor_suffix,
        "body": row.body,
        "status": row.status,
        "createdBy": row.created_by,
        "createdAt": crate::core::datetime::to_value(&row.created_at),
    })
}

/// 按 UTF-16 计数的截断（与 `routes::validate` 的 `js_len` 同口径），
/// 避免与前端字符串长度判断出现分歧。
fn truncate(s: &str, limit: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    if units.len() <= limit {
        return s.to_string();
    }
    // 截断点可能落在代理对中间，用 from_utf16_lossy 兜底替换成 U+FFFD，
    // 而不是 panic 或吞掉整条批注
    let cut = String::from_utf16_lossy(&units[..limit.saturating_sub(1)]);
    format!("{cut}…")
}

/// 组装某个 `docRef` 的 open 批注注入块（全量形态）。
///
/// 返回 `None` 表示没有 open 批注——此时**不产生 `annotations` 字段**，
/// 保持与旧响应字节级一致（CLI 与存量对拍脚本靠这个）。
pub fn build_block(
    rows: &[ann_repo::KnowledgeAnnotationRow],
) -> Option<serde_json::Value> {
    let open: Vec<&ann_repo::KnowledgeAnnotationRow> =
        rows.iter().filter(|r| r.status == "open").collect();
    if open.is_empty() {
        return None;
    }

    let total = open.len();
    let mut used = 0usize;
    let mut included = Vec::with_capacity(total);
    let mut dropped = 0usize;
    for row in &open {
        let dto = detailed(row);
        let cost = dto.to_string().encode_utf16().count();
        if used + cost > TOTAL_LIMIT && !included.is_empty() {
            dropped += 1;
            continue;
        }
        used += cost;
        included.push(dto);
    }

    let mut block = serde_json::json!({
        "total": total,
        "annotations": included,
        // 让人和 Agent 都知道还有多少没展开，以及怎么展开
        "hint": if dropped > 0 {
            format!("尚有 {dropped} 条批注未展开（超出注入体积上限），可用 `chunsun knowledge doc <docId>` 逐条查看")
        } else {
            "以上为本文档的全部未处理批注".to_string()
        },
    });
    if dropped > 0 {
        block["dropped"] = serde_json::Value::from(dropped);
    }
    Some(block)
}

/// 单篇形态用：读该 `docRef` 下的 open 批注并组装注入块。
///
/// `doc_kind` / `document_id` 的组合与路由层 `parse_doc_ref` 保持一致。
pub async fn load_block_for_doc(
    pool: &PgPool,
    project_id: &str,
    doc_kind: &str,
    document_id: Option<&str>,
) -> Result<Option<serde_json::Value>, AppError> {
    let rows = ann_repo::list_open_annotations(pool, project_id, doc_kind, document_id).await?;
    Ok(build_block(&rows))
}

/// eager 列表注入用：**一次**查询拿整个项目的 open 批注，调用方按
/// `(doc_kind, document_id)` 分组取用。
///
/// 之所以不逐篇查：eager 列表里可能有几十篇文档，逐篇查就是 N+1，
/// 而 Agent 每次启动都会走这条路。
pub async fn load_summaries_for_project(
    pool: &PgPool,
    project_id: &str,
) -> Result<std::collections::HashMap<(String, Option<String>), Vec<serde_json::Value>>, AppError> {
    let rows = ann_repo::list_open_annotations_for_project(pool, project_id).await?;
    let mut map: std::collections::HashMap<(String, Option<String>), Vec<serde_json::Value>> =
        std::collections::HashMap::new();
    for row in &rows {
        map.entry((row.doc_kind.clone(), row.document_id.clone()))
            .or_default()
            .push(summarize(row));
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn row(id: &str, status: &str, anchor: Option<&str>, body: &str) -> ann_repo::KnowledgeAnnotationRow {
        ann_repo::KnowledgeAnnotationRow {
            id: id.into(),
            project_id: "p1".into(),
            doc_kind: "document".into(),
            document_id: Some("d1".into()),
            anchor_text: anchor.map(|a| a.to_string()),
            anchor_prefix: None,
            anchor_suffix: None,
            body: body.into(),
            status: status.into(),
            outcome: None,
            resolved_note: None,
            resolved_by: None,
            resolved_at: None,
            created_by: "u1".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn no_open_annotations_yields_no_block() {
        let rows = vec![row("a1", "resolved", None, "已处理")];
        assert!(build_block(&rows).is_none());
    }

    #[test]
    fn resolved_annotations_are_excluded_from_prompt() {
        let rows = vec![
            row("a1", "open", Some("锚点"), "未处理"),
            row("a2", "resolved", Some("锚点"), "已处理"),
            row("a3", "stale", Some("锚点"), "漂移了"),
        ];
        let block = build_block(&rows).expect("有 open 批注");
        assert_eq!(block["total"], 1);
        let arr = block["annotations"].as_array().expect("array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "a1");
    }

    #[test]
    fn oversize_block_truncates_but_reports_dropped_count() {
        let long = "字".repeat(2000);
        let rows: Vec<_> = (0..10)
            .map(|i| row(&format!("a{i}"), "open", Some("锚点"), &long))
            .collect();
        let block = build_block(&rows).expect("有 open 批注");
        assert_eq!(block["total"], 10);
        assert!(block["dropped"].as_u64().unwrap_or(0) > 0);
        assert!(block["hint"].as_str().unwrap_or("").contains("chunsun knowledge doc"));
    }

    #[test]
    fn truncate_counts_utf16_units() {
        assert_eq!(truncate("短", 10), "短");
        // 60 个字符截到 10 → 9 个字符 + 省略号
        let out = truncate(&"a".repeat(60), 10);
        assert_eq!(out.encode_utf16().count(), 10);
        assert!(out.ends_with('…'));
    }
}
