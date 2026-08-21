//! 导出权限策略 SSOT 为 JSON，供 `scripts/gen-permissions.ts` 生成前端镜像与文档矩阵。
//!
//! 运行：`cargo run --quiet --example dump_permissions`
//!
//! 用 `#[path]` 直接包含策略模块本体（而非复制一份），保证导出的永远是后端正在
//! 使用的那份矩阵。除动作元数据外，还把 `meets_level` 在全部访问上下文组合下的
//! 判定结果一并导出成真值表——前端镜像因此可以纯查表，不必重写判定逻辑，也就
//! 不存在两端逻辑漂移的可能。

// 策略模块含后端运行时才会用到的判定入口，example 里未全部触及
#[allow(dead_code)]
#[path = "../src/core/permission_policy.rs"]
mod permission_policy;

use permission_policy::{
    meets_level, ProjectAccessContext, ProjectPrivilegeLevel, ProjectRole, PROJECT_ACTIONS,
};
use serde_json::json;

fn level_str(level: ProjectPrivilegeLevel) -> &'static str {
    match level {
        ProjectPrivilegeLevel::Member => "member",
        ProjectPrivilegeLevel::Manager => "manager",
        ProjectPrivilegeLevel::Owner => "owner",
    }
}

fn main() {
    let actions: Vec<_> = PROJECT_ACTIONS
        .iter()
        .map(|(action, meta)| {
            json!({
                "key": action.as_str(),
                "level": level_str(meta.level),
                "label": meta.label,
                "group": meta.group,
            })
        })
        .collect();

    // 判定真值表：level|isPlatformAdmin|isCreator|memberRole -> bool
    let levels = [
        ProjectPrivilegeLevel::Member,
        ProjectPrivilegeLevel::Manager,
        ProjectPrivilegeLevel::Owner,
    ];
    let roles = [
        Some(ProjectRole::Owner),
        Some(ProjectRole::Admin),
        Some(ProjectRole::Member),
        None,
    ];
    let mut decisions = serde_json::Map::new();
    for level in levels {
        for is_platform_admin in [false, true] {
            for is_creator in [false, true] {
                for role in roles {
                    let ctx = ProjectAccessContext {
                        is_platform_admin,
                        is_creator,
                        member_role: role,
                    };
                    let key = format!(
                        "{}|{}|{}|{}",
                        level_str(level),
                        u8::from(is_platform_admin),
                        u8::from(is_creator),
                        role.map(|r| r.as_str()).unwrap_or("NONE"),
                    );
                    decisions.insert(key, json!(meets_level(level, &ctx)));
                }
            }
        }
    }

    let out = json!({ "actions": actions, "decisions": decisions });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
