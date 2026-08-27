use clap::{Args, Subcommand};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::ApiClient;
use crate::commands::{print_json, CmdError, CmdResult};
use crate::config::load_config;

fn req_path(project_id: &str, requirement: &str) -> String {
    format!(
        "/projects/{}/requirements/{}",
        project_id,
        urlencoding::encode(requirement)
    )
}

// ---------- 通用响应 ----------

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    id: String,
    requirement_id: String,
    index: i64,
    status: String,
    #[serde(default)]
    end_reason: Option<String>,
    started_at: String,
    #[serde(default)]
    ended_at: Option<String>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    id: String,
    run_id: String,
    seq: i64,
    kind: String,
    summary: String,
    #[serde(default)]
    detail: Option<String>,
    created_at: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Scenario {
    id: String,
    #[serde(default)]
    requirement_id: String,
    key: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    status: String,
}

#[derive(Debug, Deserialize)]
struct ScenarioWithCases {
    key: String,
    title: String,
    status: String,
    #[serde(default)]
    cases: Vec<Value>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseRow {
    id: String,
    requirement_id: String,
    scenario_id: String,
    title: String,
    kind: String,
    status: String,
    execution_plan: String,
    #[serde(default)]
    local_path: Option<String>,
    #[serde(default)]
    actual_result: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

fn run_status_label(s: &str) -> &str {
    match s {
        "running" => "运行中",
        "finished" => "已结束",
        "completed" => "已完成",
        "abandoned" => "已放弃",
        _ => s,
    }
}

fn scenario_status_label(s: &str) -> &str {
    match s {
        "pending" => "待验收",
        "passing" => "通过",
        "failing" => "失败",
        "blocked" => "受阻",
        "waived" => "已豁免",
        _ => s,
    }
}

fn case_status_label(s: &str) -> &str {
    match s {
        "pending" => "待执行",
        "passed" => "通过",
        "failed" => "失败",
        "blocked" => "受阻",
        "skipped" => "跳过",
        _ => s,
    }
}

fn print_run(r: &Run) {
    println!(
        "#{}  {}  [{}]",
        r.index,
        r.id,
        run_status_label(&r.status)
    );
    if let Some(reason) = &r.end_reason {
        println!("结束原因：{reason}");
    }
    println!("开始：{}", r.started_at);
    if let Some(end) = &r.ended_at {
        println!("结束：{end}");
    }
}

fn ensure_ok<T>(result: ListResponse<T>, fallback: &str) -> Result<T, CmdError> {
    if !result.success {
        return Err(CmdError::new(
            result.error.unwrap_or_else(|| fallback.to_string()),
        ));
    }
    result
        .data
        .ok_or_else(|| CmdError::new(fallback.to_string()))
}

// ---------- run ----------

#[derive(Args)]
pub struct RunArgs {
    #[command(subcommand)]
    cmd: RunCmd,
}

#[derive(Subcommand)]
pub enum RunCmd {
    /// 列出需求的全部 Run
    List { req: String },
    /// 开新 Run（撞锁时提示 lastActiveAt 并建议 takeover）
    Start { req: String, #[arg(long)] json: bool },
    /// 撞锁接管：把 running 旧 Run 置 finished 再开新 Run
    Takeover { req: String, #[arg(long)] json: bool },
    /// Run 状态迁移（completed / finished / abandoned）
    Status {
        req: String,
        /// completed | finished | abandoned
        #[arg(long)]
        status: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// 规则提醒层：输出当前未满足的柔性约束（进 prompt 前调用）
    Remind { req: String },
}

fn list_runs(api: &ApiClient, project_id: &str, req: &str) -> Result<Vec<Run>, CmdError> {
    let result: ListResponse<Vec<Run>> =
        api.get(&format!("{}/runs", req_path(project_id, req)))?;
    ensure_ok(result, "获取 Run 列表失败")
}

fn fetch_reminders(api: &ApiClient, project_id: &str, req: &str) -> Result<Vec<String>, CmdError> {
    let mut reminders: Vec<String> = Vec::new();

    let runs = list_runs(api, project_id, req)?;
    let scenarios: ListResponse<Vec<Scenario>> =
        api.get(&format!("{}/scenarios", req_path(project_id, req)))?;
    let scenarios = ensure_ok(scenarios, "获取场景失败")?;

    if scenarios.is_empty() {
        reminders.push("当前需求尚无任何验收场景——请先 upsert 至少一个 happy path 场景。".into());
    }
    for s in scenarios.iter().filter(|s| s.status == "failing") {
        reminders.push(format!("场景「{}」当前为 failing，需修复后置 passing。", s.title));
    }

    if let Some(context) = api
        .get::<Value>(&format!("{}/memory", req_path(project_id, req)))
        .ok()
        .and_then(|v| v.get("data").cloned())
    {
        let open = context
            .get("snapshot")
            .and_then(|s| s.get("openDecisions"))
            .and_then(|o| o.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        if open > 0 {
            reminders.push(format!("存在 {open} 个未决 open decision，处理或更新后再继续。"));
        }
    }

    if let Some(run) = runs.iter().find(|r| r.status == "running") {
        let steps: ListResponse<Vec<Step>> = api.get(&format!(
            "{}/runs/{}/steps",
            req_path(project_id, req),
            run.id
        ))?;
        if let Some(steps) = steps.data {
            if steps.len() >= 8 && !steps.iter().any(|s| s.kind == "test" || s.kind == "verify") {
                reminders.push(format!(
                    "当前 Run 已执行 {} 个 Step 尚无 test/verify——注意验收闭环。",
                    steps.len()
                ));
            }
            // RRI：有 code Step 且其后无 reflect（评审-反思-改进）时柔性提醒
            if let Some(idx) = steps.iter().rposition(|s| s.kind == "code") {
                let has_reflect_after = steps[idx + 1..].iter().any(|s| s.kind == "reflect");
                if steps.len() >= 4 && !has_reflect_after {
                    reminders.push(
                        "当前 Run 已有 code Step 尚无 reflect——关键环节建议做一次评审-反思-改进（step add --kind reflect）。"
                            .into(),
                    );
                }
            }
        }
    }

    Ok(reminders)
}

fn print_reminders(reminders: &[String]) {
    if reminders.is_empty() {
        println!("[chunsun] 无待提醒的柔性约束。");
        return;
    }
    println!("[chunsun] 柔性约束提醒：");
    for r in reminders {
        println!("  - {r}");
    }
}

pub fn run_run(args: RunArgs) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    match args.cmd {
        RunCmd::List { req } => {
            let data = list_runs(&api, &config.project_id, &req)?;
            if data.is_empty() {
                println!("[chunsun] 该需求暂无 Run。");
                return Ok(());
            }
            for r in &data {
                print_run(r);
            }
            Ok(())
        }
        RunCmd::Start { req, json } => {
            let result = api.post(
                &format!("{}/runs", req_path(&config.project_id, &req)),
                json!({}),
            );
            match result {
                Ok(r) => {
                    let data: Run = ensure_ok(r, "开 Run 失败")?;
                    if json {
                        return print_json(&data);
                    }
                    println!("[chunsun] 新 Run 已启动：{}", data.id);
                    println!("  下一步建议：{}", data.id);
                    let reminders = fetch_reminders(&api, &config.project_id, &req)?;
                    print_reminders(&reminders);
                    Ok(())
                }
                Err(e) if e.to_string().contains("RUN_ALREADY_RUNNING") => {
                    let runs = list_runs(&api, &config.project_id, &req)?;
                    if let Some(r) = runs.iter().find(|r| r.status == "running") {
                        println!(
                            "[chunsun] 该需求已有 Run 在跑：#{} {}（最后活跃：{}）",
                            r.index, r.id, r.started_at
                        );
                        println!("  -> 若该 Run 已僵死（CLI 崩溃残留），请执行：chunsun run takeover {req}");
                        return Err(CmdError::exit_only(1));
                    }
                    Err(e.into())
                }
                Err(e) => Err(e.into()),
            }
        }
        RunCmd::Takeover { req, json } => {
            let result: ListResponse<Value> = api.post(
                &format!("{}/runs/takeover", req_path(&config.project_id, &req)),
                json!({}),
            )?;
            let data = ensure_ok(result, "接管失败")?;
            if json {
                return print_json(&data);
            }
            println!("[chunsun] 已接管：旧 Run 置 finished，新 Run 已启动。");
            Ok(())
        }
        RunCmd::Status {
            req,
            status,
            reason,
            json,
        } => {
            let runs = list_runs(&api, &config.project_id, &req)?;
            let run = runs
                .iter()
                .find(|r| r.status == "running")
                .ok_or_else(|| CmdError::new("当前没有 running 的 Run"))?;
            let mut body = json!({ "status": status });
            if let Some(r) = reason {
                body["endReason"] = json!(r);
            }
            let result = api.patch(
                &format!(
                    "{}/runs/{}",
                    req_path(&config.project_id, &req),
                    run.id
                ),
                body,
            );
            match result {
                Ok(r) => {
                    let data: Run = ensure_ok(r, "状态迁移失败")?;
                    if json {
                        return print_json(&data);
                    }
                    println!(
                        "[chunsun] Run #{} 已置为 {}",
                        data.index,
                        run_status_label(&data.status)
                    );
                    if data.status == "completed" {
                        println!("  验收全绿，需求已完成。");
                    } else if data.status == "finished" || data.status == "abandoned" {
                        println!("  本轮已结束，可随时 /chunsun 开新 Run 继续。");
                    }
                    Ok(())
                }
                Err(e) if e.to_string().contains("COMPLETION_GATE_NOT_MET") => {
                    println!("[chunsun] 无法 completed：验收硬条件未满足（场景须全部 passing/waived 且无 open decisions）。");
                    println!("  可用 --reason 说明结束原因，或先处理未通过场景。");
                    Err(CmdError::exit_only(1))
                }
                Err(e) => Err(e.into()),
            }
        }
        RunCmd::Remind { req } => {
            let reminders = fetch_reminders(&api, &config.project_id, &req)?;
            print_reminders(&reminders);
            Ok(())
        }
    }
}

// ---------- step ----------

#[derive(Args)]
pub struct StepArgs {
    #[command(subcommand)]
    cmd: StepCmd,
}

#[derive(Subcommand)]
pub enum StepCmd {
    /// 追加一个 Step（seq 自动递增）
    Add {
        req: String,
        #[arg(long)]
        run: String,
        /// think | code | test | verify | ask_user | info | reflect
        #[arg(long)]
        kind: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        detail: Option<String>,
        /// JSON 字符串（artifacts）
        #[arg(long)]
        artifacts: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// 列出某 Run 的 Steps
    List { req: String, #[arg(long)] run: String },
}

pub fn run_step(args: StepArgs) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    match args.cmd {
        StepCmd::Add {
            req,
            run,
            kind,
            summary,
            detail,
            artifacts,
            json,
        } => {
            let mut body = json!({
                "kind": kind,
                "summary": summary,
            });
            if let Some(d) = detail {
                body["detail"] = json!(d);
            }
            if let Some(a) = artifacts {
                let parsed: Value = serde_json::from_str(&a)
                    .map_err(|_| CmdError::new("--artifacts 必须是合法 JSON"))?;
                body["artifacts"] = parsed;
            }
            let result: ListResponse<Step> = api.post(
                &format!(
                    "{}/runs/{}/steps",
                    req_path(&config.project_id, &req),
                    urlencoding::encode(&run)
                ),
                body,
            )?;
            let data = ensure_ok(result, "上报 Step 失败")?;
            if json {
                return print_json(&data);
            }
            println!(
                "[chunsun] Step #{} ({}) 已记录：{}",
                data.seq, data.kind, data.summary
            );
            Ok(())
        }
        StepCmd::List { req, run } => {
            let result: ListResponse<Vec<Step>> = api.get(&format!(
                "{}/runs/{}/steps",
                req_path(&config.project_id, &req),
                urlencoding::encode(&run)
            ))?;
            let data = ensure_ok(result, "获取 Steps 失败")?;
            if data.is_empty() {
                println!("[chunsun] 该 Run 暂无 Step。");
                return Ok(());
            }
            for s in data {
                println!("[{}] #{}  {:<8}  {}", s.created_at, s.seq, s.kind, s.summary);
            }
            Ok(())
        }
    }
}

// ---------- scenario ----------

#[derive(Args)]
pub struct ScenarioArgs {
    #[command(subcommand)]
    cmd: ScenarioCmd,
}

#[derive(Subcommand)]
pub enum ScenarioCmd {
    /// 列出场景（--include-cases 时带用例）
    List { req: String, #[arg(long)] include_cases: bool },
    /// upsert 场景（按 key）
    Upsert {
        req: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        description: Option<String>,
        /// pending | passing | failing | blocked | waived
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// 场景状态（passing/failing/blocked/waived/pending）
    Status {
        req: String,
        scenario: String,
        status: String,
        #[arg(long)]
        json: bool,
    },
}

pub fn run_scenario(args: ScenarioArgs) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    match args.cmd {
        ScenarioCmd::List { req, include_cases } => {
            let query = if include_cases { "?includeCases=true" } else { "" };
            let result: ListResponse<Vec<ScenarioWithCases>> = api.get(&format!(
                "{}/scenarios{query}",
                req_path(&config.project_id, &req)
            ))?;
            let data = ensure_ok(result, "获取场景失败")?;
            if data.is_empty() {
                println!("[chunsun] 该需求暂无场景（循环中 Agent 会 upsert）。");
                return Ok(());
            }
            for s in data {
                println!(
                    "{}  [{}]  {}",
                    s.key,
                    scenario_status_label(&s.status),
                    s.title
                );
                if include_cases {
                    for c in &s.cases {
                        println!("    - {}  [{}]", c.get("title").and_then(|v| v.as_str()).unwrap_or("?"), c.get("status").and_then(|v| v.as_str()).unwrap_or("?"));
                    }
                }
            }
            Ok(())
        }
        ScenarioCmd::Upsert {
            req,
            key,
            title,
            description,
            status,
            json,
        } => {
            let mut body = json!({ "key": key, "title": title });
            if let Some(d) = description {
                body["description"] = json!(d);
            }
            if let Some(s) = status {
                body["status"] = json!(s);
            }
            let result: ListResponse<Scenario> = api.put(
                &format!("{}/scenarios", req_path(&config.project_id, &req)),
                body,
            )?;
            let data = ensure_ok(result, "upsert 场景失败")?;
            if json {
                return print_json(&data);
            }
            println!(
                "[chunsun] 场景已写入：{}  [{}]",
                data.key,
                scenario_status_label(&data.status)
            );
            Ok(())
        }
        ScenarioCmd::Status {
            req,
            scenario,
            status,
            json,
        } => {
            let result: ListResponse<Scenario> = api.patch(
                &format!(
                    "{}/scenarios/{}/status",
                    req_path(&config.project_id, &req),
                    urlencoding::encode(&scenario)
                ),
                json!({ "status": status }),
            )?;
            let data = ensure_ok(result, "更新场景状态失败")?;
            if json {
                return print_json(&data);
            }
            println!(
                "[chunsun] 场景「{}」已置为 {}",
                data.key,
                scenario_status_label(&data.status)
            );
            Ok(())
        }
    }
}

// ---------- case ----------

#[derive(Args)]
pub struct CaseArgs {
    #[command(subcommand)]
    cmd: CaseCmd,
}

#[derive(Subcommand)]
pub enum CaseCmd {
    /// 列出需求全部用例
    List { req: String },
    /// upsert 用例（--scenario 可传场景 id 或 key）
    Upsert {
        req: String,
        #[arg(long)]
        scenario: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        title: String,
        /// unit | integration | e2e
        #[arg(long)]
        kind: Option<String>,
        /// auto | manual
        #[arg(long)]
        plan: Option<String>,
        #[arg(long)]
        local_path: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// 用例状态回写（passed/failed/blocked/skipped）
    Status {
        req: String,
        case: String,
        status: String,
        #[arg(long)]
        result: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// 删除用例（按 id）
    Delete {
        req: String,
        case: String,
        #[arg(long)]
        json: bool,
    },
}

pub fn run_case(args: CaseArgs) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    match args.cmd {
        CaseCmd::List { req } => {
            let result: ListResponse<Vec<CaseRow>> =
                api.get(&format!("{}/cases", req_path(&config.project_id, &req)))?;
            let data = ensure_ok(result, "获取用例失败")?;
            if data.is_empty() {
                println!("[chunsun] 该需求暂无用例。");
                return Ok(());
            }
            for c in data {
                println!(
                    "{}  [{}]  {}",
                    c.id,
                    case_status_label(&c.status),
                    c.title
                );
            }
            Ok(())
        }
        CaseCmd::Upsert {
            req,
            scenario,
            id,
            title,
            kind,
            plan,
            local_path,
            json,
        } => {
            let mut body = json!({
                "scenarioKey": scenario,
                "title": title,
            });
            if let Some(i) = id {
                body["id"] = json!(i);
            }
            if let Some(k) = kind {
                body["kind"] = json!(k);
            }
            if let Some(p) = plan {
                body["executionPlan"] = json!(p);
            }
            if let Some(p) = local_path {
                body["localPath"] = json!(p);
            }
            let result: ListResponse<CaseRow> = api.put(
                &format!("{}/cases", req_path(&config.project_id, &req)),
                body,
            )?;
            let data = ensure_ok(result, "upsert 用例失败")?;
            if json {
                return print_json(&data);
            }
            println!("[chunsun] 用例已写入：{}  [{}]", data.id, data.title);
            Ok(())
        }
        CaseCmd::Status {
            req,
            case,
            status,
            result,
            json,
        } => {
            let mut body = json!({ "status": status, "executedBy": "agent" });
            if let Some(r) = result {
                body["actualResult"] = json!(r);
            }
            let result: ListResponse<CaseRow> = api.patch(
                &format!(
                    "{}/cases/{}/status",
                    req_path(&config.project_id, &req),
                    urlencoding::encode(&case)
                ),
                body,
            )?;
            let data = ensure_ok(result, "回写用例状态失败")?;
            if json {
                return print_json(&data);
            }
            println!(
                "[chunsun] 用例「{}」已置为 {}",
                data.title,
                case_status_label(&data.status)
            );
            Ok(())
        }
        CaseCmd::Delete { req, case, json } => {
            let result: ListResponse<Value> = api.delete(&format!(
                "{}/cases/{}",
                req_path(&config.project_id, &req),
                urlencoding::encode(&case)
            ))?;
            let data = ensure_ok(result, "删除用例失败")?;
            if json {
                return print_json(&data);
            }
            println!("[chunsun] 用例 {} 已删除。", case);
            Ok(())
        }
    }
}

// ---------- reset / fix ----------

#[derive(Args)]
pub struct ResetArgs {
    /// 需求 ID
    pub req: String,
    #[arg(long)]
    pub json: bool,
}

pub fn run_reset(args: ResetArgs) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let result: ListResponse<Value> =
        api.post(&format!("{}/reset", req_path(&config.project_id, &args.req)), json!({}))?;
    let data = ensure_ok(result, "重置失败")?;
    if args.json {
        return print_json(&data);
    }
    println!("[chunsun] 已全量重置：Memory 工作记忆清空（保留澄清边界），场景/用例重置 pending，新 Run 已启动。");
    Ok(())
}

#[derive(Args)]
pub struct FixArgs {
    /// 缺陷 ID
    pub defect: String,
    #[arg(long)]
    pub json: bool,
}

pub fn run_fix(args: FixArgs) -> CmdResult {
    let config = load_config();
    let api = ApiClient::new(&config)?;
    let result: Value = api.post(
        &format!(
            "/projects/{}/defects/{}/convert-to-requirement",
            config.project_id,
            urlencoding::encode(&args.defect)
        ),
        json!({}),
    )?;
    if result.get("success").and_then(|v| v.as_bool()) != Some(true) {
        return Err(CmdError::new(
            result
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("缺陷转需求失败")
                .to_string(),
        ));
    }
    let req_id = result
        .pointer("/data/requirement/id")
        .or_else(|| result.pointer("/data/id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| CmdError::new("解析修复需求失败"))?;

    let run_result: ListResponse<Run> = api.post(
        &format!("{}/runs", req_path(&config.project_id, req_id)),
        json!({}),
    )?;
    let run = ensure_ok(run_result, "启动修复 Run 失败")?;
    if args.json {
        return print_json(&json!({
            "requirementId": req_id,
            "runId": run.id,
            "runIndex": run.index,
        }));
    }
    println!("[chunsun] 修复需求已派生：{req_id}，Run #{} 已启动。", run.index);
    println!("  下一步：直接进入自主交付迭代（/chunsun {req_id} 也可继续）。");
    Ok(())
}
