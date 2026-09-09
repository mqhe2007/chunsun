//! Harness 技能模板（实例 SSOT）。
//!
//! 模板文件落在 `packages/backend/templates/`；CLI 通过相对路径 `include_str!`
//! 引用同一份正文，保证 `chunsun init` / `update` 与本端点返回内容一致。
//!
//! 2026-09-08-skill-only-harness：技能是唯一 harness 载体——斜线命令模板与
//! 常驻规则模板已并入技能模板（不再单独下发/安装），payload 仅含 3 个文件。

use serde_json::{json, Map, Value};

/// 当前模板版本号。技能 / 协议有结构性改动时必须递增。
/// 与 `templates/VERSION` 及 CLI 侧 `fixture_version` 测试保持一致。
pub const TEMPLATE_VERSION: &str = "2026-09-09-memory-writing-rules";

const SKILL: &str = include_str!("../templates/skill.md");
const LOOP_RULES: &str = include_str!("../templates/loop-rules.md");
const COMMANDS: &str = include_str!("../templates/commands.md");
#[cfg(test)]
const VERSION_FILE: &str = include_str!("../templates/VERSION");

/// API 响应中的逻辑文件名 → 正文。
pub fn template_files() -> [(&'static str, &'static str); 3] {
    [
        ("SKILL.md", SKILL),
        ("loop-rules.md", LOOP_RULES),
        ("commands.md", COMMANDS),
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
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn payload_no_longer_serves_slash_templates() {
        // 斜线命令模板已并入技能（意图路由 + 缺陷修复派生），不再单独下发。
        let data = template_payload();
        let files = data["files"].as_object().expect("files object");
        assert!(!files.contains_key("slash/chunsun.md"));
        assert!(!files.contains_key("slash/chunsun-fix.md"));
    }

    #[test]
    fn skill_is_single_harness_surface() {
        // 技能承载全部 harness 内容：意图路由、缺陷修复派生、核心规则引用、知识渐进式加载。
        assert!(SKILL.contains("意图路由"));
        assert!(SKILL.contains("缺陷修复派生"));
        assert!(SKILL.contains("references/loop-rules.md"));
        assert!(SKILL.contains("自主交付"));
        assert!(SKILL.contains("knowledge index"));
        assert!(!SKILL.contains("斜线命令（仅 2 个）"), "斜线命令章节应已移除");
        assert!(!SKILL.contains("AGENTS.md）"), "不应再引用 AGENTS.md 桥接");
        assert!(LOOP_RULES.contains("技能激活期间恒生效"));
    }
}
