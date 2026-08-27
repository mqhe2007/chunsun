//! Harness 技能模板（实例 SSOT）。
//!
//! 模板文件落在 `packages/backend/templates/`；CLI 通过相对路径 `include_str!`
//! 引用同一份正文，保证 `chunsun init` / `update` 与本端点返回内容一致。

use serde_json::{json, Map, Value};

/// 当前模板版本号。技能 / 斜线命令 / 自主交付协议有结构性改动时必须递增。
/// 与 `templates/VERSION` 及 CLI `TEMPLATE_VERSION` 保持一致。
pub const TEMPLATE_VERSION: &str = "2026-08-27-knowledge-load-strategy";

const SKILL: &str = include_str!("../templates/skill.md");
const LOOP_RULES: &str = include_str!("../templates/loop-rules.md");
const COMMANDS: &str = include_str!("../templates/commands.md");
const SLASH_CHUNSUN: &str = include_str!("../templates/slash/chunsun.md");
const SLASH_CHUNSUN_FIX: &str = include_str!("../templates/slash/chunsun-fix.md");
#[cfg(test)]
const VERSION_FILE: &str = include_str!("../templates/VERSION");

/// API 响应中的逻辑文件名 → 正文。
pub fn template_files() -> [(&'static str, &'static str); 5] {
    [
        ("SKILL.md", SKILL),
        ("loop-rules.md", LOOP_RULES),
        ("commands.md", COMMANDS),
        ("slash/chunsun.md", SLASH_CHUNSUN),
        ("slash/chunsun-fix.md", SLASH_CHUNSUN_FIX),
    ]
}

/// `GET /harness/template` 的 `data` 载荷。
pub fn template_payload() -> Value {
    let mut files = Map::new();
    for (name, body) in template_files() {
        files.insert(name.to_string(), Value::String(body.to_string()));
    }
    json!({
        "templateVersion": TEMPLATE_VERSION,
        "files": files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_file_matches_const() {
        assert_eq!(
            VERSION_FILE.trim(),
            TEMPLATE_VERSION,
            "templates/VERSION 必须与 TEMPLATE_VERSION 常量一致"
        );
    }

    #[test]
    fn payload_contains_all_expected_files() {
        let data = template_payload();
        assert_eq!(data["templateVersion"], TEMPLATE_VERSION);
        let files = data["files"].as_object().expect("files object");
        for (name, body) in template_files() {
            assert_eq!(
                files.get(name).and_then(|v| v.as_str()),
                Some(body),
                "missing or mismatched file: {name}"
            );
            assert!(!body.trim().is_empty(), "{name} 不应为空");
        }
        assert_eq!(files.len(), 5);
    }

    #[test]
    fn skill_mentions_chunsun_protocol() {
        assert!(SKILL.contains("自主交付"));
        assert!(LOOP_RULES.contains("验收定义") || LOOP_RULES.contains("passing"));
    }
}
