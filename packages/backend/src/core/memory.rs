//! 工作记忆 / 项目记忆的 snapshot 校验（需求级与项目级共用）。
//!
//! 2026-09-09：容量上限从 20_000 统一调整为 10_000 字符（需求工作记忆与项目记忆一致），
//! 校验逻辑从 `routes/harness.rs` 上移至此，供两个路由域共用。

use crate::api::AppError;

/// 记忆 snapshot 粒度上限（字符数）：协议约定 ~10k 字符内，超出拒绝写入。
pub const MEMORY_SNAPSHOT_MAX_CHARS: usize = 10_000;

/// 校验 PUT memory 的 snapshot 三态，通过则返回可下传仓储层的值：
/// - 字段缺失 → 400 SNAPSHOT_REQUIRED（缺失即调用方 bug；静默回旧行会让调用方
///   误以为已写入，是「工作记忆没生效」类缺陷的温床）
/// - 显式 null → 放行（落 NULL，清空记忆）
/// - 超上限 → 400 MEMORY_TOO_LARGE（记忆是唯一进 prompt 的工作记忆，粒度严控）
pub fn validate_memory_snapshot(
    snapshot: &Option<Option<String>>,
) -> Result<Option<&str>, AppError> {
    let Some(value) = snapshot.as_ref().map(|v| v.as_ref()) else {
        return Err(AppError::bad_request("SNAPSHOT_REQUIRED"));
    };
    if let Some(v) = value {
        if v.chars().count() > MEMORY_SNAPSHOT_MAX_CHARS {
            return Err(AppError::bad_request("MEMORY_TOO_LARGE"));
        }
    }
    Ok(value.map(|x| x.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_snapshot_validation_rejects_missing_and_oversize() {
        // 缺失 → SNAPSHOT_REQUIRED（不再静默回旧行）
        let err = validate_memory_snapshot(&None).unwrap_err();
        assert_eq!(err.code, "SNAPSHOT_REQUIRED");
        // 显式 null → 放行
        assert_eq!(validate_memory_snapshot(&Some(None)).unwrap(), None);
        // 正常值 → 放行
        let v = "## 本轮总结\n- 做了什么".to_string();
        assert_eq!(
            validate_memory_snapshot(&Some(Some(v.clone()))).unwrap(),
            Some(v.as_str())
        );
        // 超 10k 字符 → MEMORY_TOO_LARGE
        let big = "x".repeat(MEMORY_SNAPSHOT_MAX_CHARS + 1);
        let err = validate_memory_snapshot(&Some(Some(big))).unwrap_err();
        assert_eq!(err.code, "MEMORY_TOO_LARGE");
        // 恰好贴边（≤ 上限）→ 放行
        let edge = "x".repeat(MEMORY_SNAPSHOT_MAX_CHARS);
        assert!(validate_memory_snapshot(&Some(Some(edge))).is_ok());
    }
}
