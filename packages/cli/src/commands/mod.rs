
pub mod defect;
pub mod dependency;
pub mod env;


pub mod harness;
pub mod init;
pub mod knowledge;
pub mod memory;
pub mod repo;
pub mod requirement;
pub mod update;

use std::fmt;


#[derive(Debug)]
pub struct CmdError {
    message: String,
    exit_code: u8,
    silent: bool,
}

impl CmdError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 1,
            silent: false,
        }
    }

    pub fn with_code(message: impl Into<String>, code: u8) -> Self {
        Self {
            message: message.into(),
            exit_code: code,
            silent: false,
        }
    }

    pub fn exit_only(code: u8) -> Self {
        Self {
            message: String::new(),
            exit_code: code,
            silent: true,
        }
    }

    pub fn exit_code(&self) -> u8 {
        self.exit_code
    }

    pub fn is_silent(&self) -> bool {
        self.silent
    }
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CmdError {}

impl From<crate::api::ApiError> for CmdError {
    fn from(value: crate::api::ApiError) -> Self {
        CmdError::new(value.to_string())
    }
}

impl From<std::io::Error> for CmdError {
    fn from(value: std::io::Error) -> Self {
        CmdError::new(value.to_string())
    }
}

impl From<serde_json::Error> for CmdError {
    fn from(value: serde_json::Error) -> Self {
        CmdError::new(value.to_string())
    }
}

impl From<anyhow::Error> for CmdError {
    fn from(value: anyhow::Error) -> Self {
        CmdError::new(value.to_string())
    }
}

pub type CmdResult = Result<(), CmdError>;

pub fn print_json<T: serde::Serialize>(value: &T) -> CmdResult {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// 乐观锁冲突（409 `*_CONFLICT`）的统一人读输出。
///
/// 抽成共用函数是因为四个写入点（知识文档 / 宪法 / 项目记忆 / 需求记忆）的
/// 报文形状完全一致 —— `data` 里都带 `yourRevision`、`currentRevision`、
/// `currentContent` 或 `currentSnapshot`。各写一份的结果必然是会漏掉其中一处，
/// 而漏掉的那一处恰恰是 Agent 最需要它的时候。
///
/// **正文按需打印，不无条件铺满终端**：知识文档正文上限是 `usize::MAX`，
/// 一份几万字的文档砸进 stdout 只会把「发生了什么」淹掉。默认只给一个字符数
/// 和取全文的 flag，让调用方（Agent）自己决定要不要读 —— 它是能跑命令的，
/// 不需要我们把整篇塞给它。
///
/// 返回 `CmdError::exit_only(1)`：信息已经打印过了，走 silent 避免重复输出
/// （沿用 `commands/mod.rs` 既有的约定，与 `harness.rs` 的撞锁分支同形）。
/// 已被解析的冲突详情 —— 供 [`print_conflict`] 渲染。
///
/// 与渲染分开是为了**可测**：`print_conflict` 只往 stdout 写，没法断言；
/// 而字段抽取（四种写入点的 data 形状差别、`content` 与 `snapshot` 的择一、
/// `runId` 与 `runningRunId` 的择一）恰恰是最容易写错、也最值得测的部分。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ConflictInfo {
    pub your_revision: Option<i64>,
    pub current_revision: Option<i64>,
    /// 既可能是 `updatedAt`（乐观锁冲突），也可能是 `lastActiveAt`（Run 撞锁）。
    pub updated_at: Option<String>,
    pub run_id: Option<String>,
    /// 当前正文 / 记忆快照的全文。`None` 表示报文里没带正文。
    pub content: Option<String>,
    /// 正文是记忆快照（`currentSnapshot`）还是知识文档正文（`currentContent`）。
    pub content_is_snapshot: bool,
}

impl ConflictInfo {
    /// 从后端错误报文的 `data` 段解析。非 JSON、或没有 `data` 时返回全空的默认值
    /// —— 调用方仍然会打印「写入被拒绝」和重试提示，只是没有细节。
    pub fn parse(err: &crate::api::ApiError) -> Self {
        let Some(data) = err.body().and_then(|b| b.get("data").cloned()) else {
            return Self::default();
        };
        let snapshot = data.get("currentSnapshot").and_then(|v| v.as_str());
        Self {
            your_revision: data.get("yourRevision").and_then(|v| v.as_i64()),
            current_revision: data.get("currentRevision").and_then(|v| v.as_i64()),
            updated_at: data
                .get("updatedAt")
                .or_else(|| data.get("lastActiveAt"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
            run_id: data
                .get("runId")
                .or_else(|| data.get("runningRunId"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
            content: data
                .get("currentContent")
                .and_then(|v| v.as_str())
                .or(snapshot)
                .map(str::to_string),
            content_is_snapshot: snapshot.is_some(),
        }
    }
}

pub fn print_conflict(err: &crate::api::ApiError) -> CmdError {
    let info = ConflictInfo::parse(err);

    println!("[chunsun] 写入被拒绝：这行已被他人修改（乐观锁冲突）。");
    if let (Some(yours), Some(current)) = (info.your_revision, info.current_revision) {
        println!("  你的版本：{yours}    当前版本：{current}");
    }
    if let Some(updated) = &info.updated_at {
        println!("  他方最后写入时间：{updated}");
    }
    if let Some(id) = &info.run_id {
        println!("  正在运行的 Run：{id}");
    }

    match &info.content {
        Some(text) => {
            let label = if info.content_is_snapshot {
                "记忆快照"
            } else {
                "当前正文"
            };
            println!(
                "  {label}共 {} 字符（未直接打印）。",
                text.chars().count()
            );
            println!("  -> 取全文：在命令后加 --json，从 data.currentContent / currentSnapshot 读取");
        }
        None => {
            println!("  -> 加 --json 可看到完整冲突详情");
        }
    }

    println!("  -> 合并后带**新**的 --revision 重写（当前版本号见上）");
    CmdError::exit_only(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http_err(body: &str) -> crate::api::ApiError {
        crate::api::ApiError::Http {
            method: "PUT".into(),
            path: "/x".into(),
            status: 409,
            detail: "detail".into(),
            code: Some("KNOWLEDGE_DOC_CONFLICT".into()),
            body: body.into(),
        }
    }

    /// 知识文档冲突：`currentContent` 是正文来源。
    #[test]
    fn conflict_info_reads_knowledge_doc_shape() {
        let e = http_err(
            r#"{"success":false,"error":"KNOWLEDGE_DOC_CONFLICT","data":{
                "docId":"d1","yourRevision":7,"currentRevision":9,
                "currentContent":"hello","currentTitle":"t",
                "updatedAt":"2026-09-28T10:11:12.123Z"}}"#,
        );
        let info = ConflictInfo::parse(&e);
        assert_eq!(info.your_revision, Some(7));
        assert_eq!(info.current_revision, Some(9));
        assert_eq!(info.content.as_deref(), Some("hello"));
        assert!(!info.content_is_snapshot);
        assert_eq!(info.updated_at.as_deref(), Some("2026-09-28T10:11:12.123Z"));
        assert_eq!(info.run_id, None);
    }

    /// 记忆冲突：正文来源换成 `currentSnapshot`，且必须被标记成快照，
    /// 否则用户会看到「当前正文」而其实那是记忆快照。
    #[test]
    fn conflict_info_reads_memory_shape_and_flags_snapshot() {
        // 不用 raw string：记忆正文天然以 `##` 开头，`"##` 会把它自己的分隔符
        // 提前闭合（r# 和 r## 都一样中招）。转义写法反而更稳。
        let e = http_err(
            "{\"success\":false,\"error\":\"MEMORY_CONFLICT\",\"data\":{\
                \"projectId\":\"p1\",\"yourRevision\":1,\"currentRevision\":4,\
                \"currentSnapshot\":\"## 填坑\",\"updatedAt\":\"2026-09-28T00:00:00.000Z\"}}",
        );
        let info = ConflictInfo::parse(&e);
        assert_eq!(info.content.as_deref(), Some("## 填坑"));
        assert!(info.content_is_snapshot);
        assert_eq!(info.current_revision, Some(4));
    }

    /// Run 撞锁：没有 revision，`runId` 与 `lastActiveAt` 才是有效信息。
    /// 且 `runId` 与 `runningRunId` 两个名字都要认 —— 前者来自 handler 预检查，
    /// 后者来自仓储层兜底 409。
    #[test]
    fn conflict_info_reads_run_lock_shape_and_both_run_id_keys() {
        let precheck = http_err(
            r#"{"error":"RUN_ALREADY_RUNNING","data":{
                "requirementId":"r1","runId":"run_a","index":3,
                "lastActiveAt":"2026-09-28T01:02:03.000Z"}}"#,
        );
        let info = ConflictInfo::parse(&precheck);
        assert_eq!(info.run_id.as_deref(), Some("run_a"));
        assert_eq!(info.updated_at.as_deref(), Some("2026-09-28T01:02:03.000Z"));
        assert_eq!(info.current_revision, None);

        let fallback = http_err(
            r#"{"error":"RUN_ALREADY_RUNNING","data":{
                "requirementId":"r1","runningRunId":"run_b"}}"#,
        );
        assert_eq!(
            ConflictInfo::parse(&fallback).run_id.as_deref(),
            Some("run_b")
        );
    }

    /// 报文体不是 JSON、或整体缺 `data` 时不得 panic：这条路径是**错误处理里的
    /// 错误处理**，它自己炸掉会把原始错误一起吞掉，那才是最难查的一类 bug。
    #[test]
    fn conflict_info_survives_malformed_bodies() {
        assert_eq!(ConflictInfo::parse(&http_err("not json")), ConflictInfo::default());
        assert_eq!(
            ConflictInfo::parse(&http_err(r#"{"error":"X"}"#)),
            ConflictInfo::default()
        );
        // data 是 null 而不是对象
        assert_eq!(
            ConflictInfo::parse(&http_err(r#"{"data":null}"#)),
            ConflictInfo::default()
        );
        // 字段类型不对（revision 是字符串而非数字）→ 该字段为 None，不 panic
        let weird = ConflictInfo::parse(&http_err(r#"{"data":{"yourRevision":"7"}}"#));
        assert_eq!(weird.your_revision, None);
    }

    /// 冲突走 silent + 退出码 1：信息已自行打印，不能再让 main 打印一遍。
    #[test]
    fn print_conflict_is_silent_with_exit_code_one() {
        let e = http_err(r#"{"data":{"yourRevision":1,"currentRevision":2}}"#);
        let err = print_conflict(&e);
        assert!(err.is_silent());
        assert_eq!(err.exit_code(), 1);
    }
}
