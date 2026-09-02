use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::Value;

use crate::api::ApiClient;
use crate::commands::{print_json, CmdError, CmdResult};
use crate::config::load_config;

#[derive(Args)]
pub struct DependencyArgs {
    #[command(subcommand)]
    command: DependencyCmd,
}

#[derive(Subcommand)]
enum DependencyCmd {
    /// 列出项目内全部依赖边（source blocks target）
    List {
        #[arg(long)]
        json: bool,
    },
    /// 项目级调度分析：拓扑分层 / 关键路径 / 阻塞状态 / 可执行集合
    Schedule {
        #[arg(long)]
        json: bool,
    },
    /// 单节点阻塞状态：是否被阻塞 + 阻塞原因（未完成前置）+ 是否可执行
    Blocked {
        /// 节点类型：requirement | defect
        node_type: String,
        /// 节点 ID
        node_id: String,
        #[arg(long)]
        json: bool,
    },
    /// 解锁分析：模拟某节点完成后，其直接下游中哪些解锁、哪些仍被阻塞
    Unlock {
        /// 节点类型：requirement | defect
        node_type: String,
        /// 节点 ID
        node_id: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

fn deps_path(project_id: &str) -> String {
    format!("/projects/{project_id}/dependencies")
}

fn node_type_valid(t: &str) -> bool {
    matches!(t, "requirement" | "defect")
}

/// 按字符（而非字节）截断描述文本，避免中文等多字节字符在字节边界 panic。
fn truncate_desc(desc: &str, max_chars: usize) -> String {
    if desc.chars().count() > max_chars {
        let cut: String = desc.chars().take(max_chars).collect();
        format!("{cut}…")
    } else {
        desc.to_string()
    }
}

fn node_label(v: &Value) -> String {
    let kind = v["kind"].as_str().unwrap_or("");
    let id = v["id"].as_str().unwrap_or("");
    let status = v["status"].as_str().unwrap_or("");
    let desc = v["description"].as_str().unwrap_or("");
    format!(
        "{kind}:{id} [{}] {}",
        status,
        truncate_desc(desc, 40)
    )
}

fn print_schedule_text(v: &Value) {
    let stats = &v["stats"];
    println!(
        "[chunsun] 调度统计：total={} done={} pending={} blocked={} ready={}",
        stats["total"].as_i64().unwrap_or(0),
        stats["done"].as_i64().unwrap_or(0),
        stats["pending"].as_i64().unwrap_or(0),
        stats["blocked"].as_i64().unwrap_or(0),
        stats["ready"].as_i64().unwrap_or(0),
    );

    let node_label = |n: &Value| -> String { node_label(n) };

    // 拓扑分层
    println!("[chunsun] 拓扑执行顺序（每层可并行，层间串行）：");
    if let Some(levels) = v["levels"].as_array() {
        if levels.is_empty() {
            println!("  （无未完成任务）");
        } else {
            for (i, level) in levels.iter().enumerate() {
                let names: Vec<String> = level
                    .as_array()
                    .map(|arr| arr.iter().map(node_label).collect())
                    .unwrap_or_default();
                println!("  层{}: {}", i + 1, names.join("  ||  "));
            }
        }
    }

    // 关键路径
    println!("[chunsun] 关键路径（最长依赖链，优先推进瓶颈）：");
    if let Some(cp) = v["criticalPath"].as_array() {
        if cp.is_empty() {
            println!("  （无未完成节点）");
        } else {
            let names: Vec<String> = cp.iter().map(node_label).collect();
            println!("  {}", names.join(" → "));
        }
    }

    // 阻塞明细
    println!("[chunsun] 被阻塞任务（阻塞原因 + 前置任务）：");
    if let Some(blocked) = v["blocked"].as_array() {
        let blocked_list: Vec<&Value> = blocked
            .iter()
            .filter(|b| b["blocked"].as_bool().unwrap_or(false))
            .collect();
        if blocked_list.is_empty() {
            println!("  （无被阻塞任务）");
        } else {
            for b in blocked_list {
                println!("  - {}", node_label(&b["node"]));
                if let Some(blockers) = b["blockers"].as_array() {
                    for bl in blockers {
                        println!("      被 {} 阻塞", node_label(bl));
                    }
                }
            }
        }
    }
}

fn print_blocked_text(v: &Value) {
    let node = &v["node"];
    let node_label = |n: &Value| -> String { node_label(n) };
    println!("[chunsun] 节点：{}", node_label(node));
    let blocked = v["blocked"].as_bool().unwrap_or(false);
    if blocked {
        println!("[chunsun] 状态：被阻塞（未完成前置 {} 个）", {
            v["blockers"].as_array().map(|a| a.len()).unwrap_or(0)
        });
        if let Some(blockers) = v["blockers"].as_array() {
            for b in blockers {
                println!("  - 前置未完成：{}", node_label(b));
            }
        }
    } else {
        println!("[chunsun] 状态：可执行（无未完成前置）");
        if let Some(completed) = v["completedBlockers"].as_array() {
            if !completed.is_empty() {
                println!("  已完成前置：");
                for c in completed {
                    println!("  - {}", node_label(c));
                }
            }
        }
    }
}

fn print_unlock_text(v: &Value) {
    let node_label = |n: &Value| -> String {
        let kind = n["kind"].as_str().unwrap_or("");
        let id = n["id"].as_str().unwrap_or("");
        let status = n["status"].as_str().unwrap_or("");
        format!("{kind}:{id} [{}]", status)
    };
    println!("[chunsun] 节点完成：{}", node_label(&v["node"]));
    if let Some(unlocked) = v["unlocked"].as_array() {
        if unlocked.is_empty() {
            println!("[chunsun] 解锁：无下游解锁");
        } else {
            println!("[chunsun] 解锁下游：");
            for u in unlocked {
                println!("  - {}（可进入执行）", node_label(u));
            }
        }
    }
    if let Some(still) = v["stillBlocked"].as_array() {
        if !still.is_empty() {
            println!("[chunsun] 仍被阻塞（有其他未完成前置）：");
            for s in still {
                println!("  - {}", node_label(s));
            }
        }
    }
}

fn run_list(json: bool) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let resp: ApiResponse<Vec<Value>> = api.get(&deps_path(&config.project_id))?;
    if !resp.success {
        return Err(CmdError::new(
            resp.error.unwrap_or_else(|| "获取依赖列表失败".into()),
        ));
    }
    let data = resp.data.unwrap_or_default();
    if json {
        return print_json(&data);
    }
    if data.is_empty() {
        println!("[chunsun] 暂无依赖关系。");
        return Ok(());
    }
    println!("[chunsun] 项目依赖边（source blocks target）：");
    for e in &data {
        println!(
            "  {}:{}  →  {}:{}",
            e["sourceType"].as_str().unwrap_or(""),
            e["sourceId"].as_str().unwrap_or(""),
            e["targetType"].as_str().unwrap_or(""),
            e["targetId"].as_str().unwrap_or(""),
        );
    }
    Ok(())
}

fn run_schedule(json: bool) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let resp: ApiResponse<Value> =
        api.get(&format!("{}/schedule", deps_path(&config.project_id)))?;
    if !resp.success {
        return Err(CmdError::new(
            resp.error.unwrap_or_else(|| "获取调度分析失败".into()),
        ));
    }
    let data = resp.data.ok_or_else(|| CmdError::new("调度分析无数据"))?;
    if json {
        return print_json(&data);
    }
    print_schedule_text(&data);
    Ok(())
}

fn run_blocked(node_type: String, node_id: String, json: bool) -> CmdResult {
    if !node_type_valid(&node_type) {
        return Err(CmdError::new("node_type 只能是 requirement / defect 之一"));
    }
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let path = format!(
        "{}/{}/{}/blocked",
        deps_path(&config.project_id),
        node_type,
        node_id
    );
    let resp: ApiResponse<Value> = api.get(&path)?;
    if !resp.success {
        return Err(CmdError::new(
            resp.error.unwrap_or_else(|| "获取阻塞状态失败".into()),
        ));
    }
    let data = resp.data.ok_or_else(|| CmdError::new("阻塞状态无数据"))?;
    if json {
        return print_json(&data);
    }
    print_blocked_text(&data);
    Ok(())
}

fn run_unlock(node_type: String, node_id: String, json: bool) -> CmdResult {
    if !node_type_valid(&node_type) {
        return Err(CmdError::new("node_type 只能是 requirement / defect 之一"));
    }
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let path = format!(
        "{}/{}/{}/unlock",
        deps_path(&config.project_id),
        node_type,
        node_id
    );
    let resp: ApiResponse<Value> = api.get(&path)?;
    if !resp.success {
        return Err(CmdError::new(
            resp.error.unwrap_or_else(|| "获取解锁分析失败".into()),
        ));
    }
    let data = resp.data.ok_or_else(|| CmdError::new("解锁分析无数据"))?;
    if json {
        return print_json(&data);
    }
    print_unlock_text(&data);
    Ok(())
}

pub fn run(args: DependencyArgs) -> CmdResult {
    match args.command {
        DependencyCmd::List { json } => run_list(json),
        DependencyCmd::Schedule { json } => run_schedule(json),
        DependencyCmd::Blocked {
            node_type,
            node_id,
            json,
        } => run_blocked(node_type, node_id, json),
        DependencyCmd::Unlock {
            node_type,
            node_id,
            json,
        } => run_unlock(node_type, node_id, json),
    }
}

/// 辅助：把某节点 id 拼成 `kind:id` 标签（供外部模块打印时复用）。
#[allow(dead_code)]
pub fn node_tag(kind: &str, id: &str) -> String {
    format!("{kind}:{id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_keeps_full() {
        assert_eq!(truncate_desc("短描述", 40), "短描述");
        assert_eq!(truncate_desc("", 40), "");
    }

    #[test]
    fn truncate_long_chinese_does_not_panic() {
        // 超过 40 字符的中文描述：按字符截断，不 panic
        let long = "春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋春笋";
        let out = truncate_desc(long, 40);
        assert_eq!(out.chars().count(), 41);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn truncate_mixed_boundary() {
        // 字节边界恰好落在多字节字符中间时也不 panic
        let mixed = "依赖A依赖B依赖C依赖D依赖E依赖F依赖G依赖H依赖I依赖J依赖K依赖L依赖M依赖N依赖O依赖P";
        let out = truncate_desc(mixed, 40);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn node_type_whitelist() {
        assert!(node_type_valid("requirement"));
        assert!(node_type_valid("defect"));
        assert!(!node_type_valid("task"));
        assert!(!node_type_valid(""));
    }

    #[test]
    fn node_label_renders_with_status_and_desc() {
        let v = serde_json::json!({
            "id": "abc123",
            "kind": "requirement",
            "status": "pending",
            "description": "测试描述",
        });
        let label = node_label(&v);
        assert!(label.starts_with("requirement:abc123 [pending]"));
        assert!(label.contains("测试描述"));
    }
}
