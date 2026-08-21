//! 项目统计（1:1 移植自 `services/projectStatsService.ts`）。
//!
//! 聚合逻辑与 SQL 分离：`aggregate_*` 是纯函数，直接对 groupBy 结果做累加，便于单测覆盖
//! 「未知状态值只进 byStatus、不进具名计数」这类边界。

use serde_json::{json, Map, Value};
use sqlx::PgPool;

use crate::api::AppError;
use crate::core::pct::pct;

/// requirement 按状态聚合的结果。
#[derive(Debug, Default, PartialEq)]
pub struct RequirementStats {
    pub total: i64,
    pub pending: i64,
    pub running: i64,
    pub abandoned: i64,
    pub completed: i64,
    pub by_status: Vec<(String, i64)>,
}

/// defect 按状态聚合的结果。
#[derive(Debug, Default, PartialEq)]
pub struct DefectStats {
    pub total: i64,
    pub open: i64,
    pub processing: i64,
    pub resolved: i64,
    pub closed: i64,
    pub by_status: Vec<(String, i64)>,
    pub critical: i64,
}

pub fn aggregate_requirements(groups: &[(String, i64)]) -> RequirementStats {
    let mut stats = RequirementStats::default();
    for (status, count) in groups {
        stats.total += count;
        stats.by_status.push((status.clone(), *count));
        match status.as_str() {
            "pending" => stats.pending += count,
            "running" => stats.running += count,
            "abandoned" => stats.abandoned += count,
            "completed" => stats.completed += count,
            _ => {}
        }
    }
    stats
}

pub fn aggregate_defects(
    by_status: &[(String, i64)],
    by_severity: &[(String, i64)],
) -> DefectStats {
    let mut stats = DefectStats::default();
    for (status, count) in by_status {
        stats.total += count;
        stats.by_status.push((status.clone(), *count));
        match status.as_str() {
            "open" => stats.open += count,
            "processing" => stats.processing += count,
            "resolved" => stats.resolved += count,
            "closed" => stats.closed += count,
            _ => {}
        }
    }
    for (severity, count) in by_severity {
        if severity == "critical" {
            stats.critical += count;
        }
    }
    stats
}

fn to_map(pairs: &[(String, i64)]) -> Value {
    let mut map = Map::new();
    for (k, v) in pairs {
        map.insert(k.clone(), Value::from(*v));
    }
    Value::Object(map)
}

pub fn build_statistics(req: &RequirementStats, def: &DefectStats) -> Value {
    json!({
        "requirements": {
            "total": req.total,
            "pending": req.pending,
            "running": req.running,
            "abandoned": req.abandoned,
            "completed": req.completed,
            "byStatus": to_map(&req.by_status),
        },
        "rates": {
            "requirementCompletionPct": pct(req.completed, req.total),
        },
        "defects": {
            "total": def.total,
            "open": def.open,
            "processing": def.processing,
            "resolved": def.resolved,
            "closed": def.closed,
            "byStatus": to_map(&def.by_status),
            "critical": def.critical,
        }
    })
}

/// getProjectStatistics：三次 groupBy 聚合。
pub async fn get_project_statistics(
    pool: &PgPool,
    project_id: &str,
) -> Result<Value, AppError> {
    let requirement_groups: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status::text, COUNT(*)::bigint FROM requirement WHERE project_id = $1 \
         GROUP BY status",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let defect_status_groups: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status::text, COUNT(*)::bigint FROM defect WHERE project_id = $1 GROUP BY status",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let defect_severity_groups: Vec<(String, i64)> = sqlx::query_as(
        "SELECT severity::text, COUNT(*)::bigint FROM defect \
         WHERE project_id = $1 AND status IN ('open','processing') GROUP BY severity",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let req = aggregate_requirements(&requirement_groups);
    let def = aggregate_defects(&defect_status_groups, &defect_severity_groups);
    Ok(build_statistics(&req, &def))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_requirements_by_status() {
        let groups = vec![
            ("pending".to_string(), 2),
            ("completed".to_string(), 3),
            ("running".to_string(), 1),
        ];
        let stats = aggregate_requirements(&groups);
        assert_eq!(stats.total, 6);
        assert_eq!(stats.pending, 2);
        assert_eq!(stats.completed, 3);
        assert_eq!(stats.running, 1);
        assert_eq!(stats.abandoned, 0);
    }

    #[test]
    fn unknown_status_counts_into_total_and_by_status_only() {
        let groups = vec![("archived".to_string(), 4)];
        let stats = aggregate_requirements(&groups);
        assert_eq!(stats.total, 4);
        assert_eq!(stats.pending + stats.running + stats.abandoned + stats.completed, 0);
        assert_eq!(stats.by_status, vec![("archived".to_string(), 4)]);
    }

    #[test]
    fn critical_only_counts_open_and_processing_slice() {
        let by_status = vec![("open".to_string(), 2), ("closed".to_string(), 5)];
        let by_severity = vec![("critical".to_string(), 1), ("minor".to_string(), 1)];
        let stats = aggregate_defects(&by_status, &by_severity);
        assert_eq!(stats.total, 7);
        assert_eq!(stats.open, 2);
        assert_eq!(stats.closed, 5);
        assert_eq!(stats.critical, 1);
    }

    #[test]
    fn completion_pct_is_null_when_no_requirements() {
        let value = build_statistics(&RequirementStats::default(), &DefectStats::default());
        assert_eq!(value["rates"]["requirementCompletionPct"], Value::Null);
        assert_eq!(value["requirements"]["byStatus"], json!({}));
    }

    #[test]
    fn completion_pct_rounds_like_legacy() {
        let req = aggregate_requirements(&[
            ("completed".to_string(), 1),
            ("pending".to_string(), 2),
        ]);
        let value = build_statistics(&req, &DefectStats::default());
        assert_eq!(value["rates"]["requirementCompletionPct"], json!(33));
    }
}
