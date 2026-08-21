//! 数据访问层（sqlx + 现有 PG schema）。
//!
//! 约定：每张表用 `*_COLS` 列投影 + `FromRow` 结构体；避免 `SELECT *` / `RETURNING *`。

pub mod defect;
pub mod email_log;
pub mod email_token;
pub mod harness;
pub mod invitation;
pub mod login_attempt;
pub mod notification;
pub mod project;
pub mod project_activity;
pub mod project_context;
pub mod project_env_var;
pub mod project_member;
pub mod prompt;
pub mod repository;
pub mod requirement;
pub mod system_setting;
pub mod user;
