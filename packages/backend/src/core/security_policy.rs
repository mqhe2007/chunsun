//! 安全策略（纯逻辑部分），1:1 移植自
//! `packages/backend/src/lib/securityPolicy.ts` + `systemSettingService.ts` 默认值。
//! DB 动态读取（getPasswordPolicy / getLoginLockoutPolicy）在后续域移植时接入。

use std::collections::HashSet;

/// 密码强度策略（默认值来自 DEFAULT_SETTINGS.security.*）。
#[derive(Debug, Clone)]
pub struct PasswordPolicy {
    pub min_length: usize,
    pub require_number: bool,
    pub require_uppercase: bool,
    pub require_special_char: bool,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 8,
            require_number: true,
            require_uppercase: false,
            require_special_char: false,
        }
    }
}

pub struct PasswordValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

/// 与 TS 相同的特殊字符集合：
/// `!@#$%^&*()_+\-=[]{};':"\\|,.<>/?`
fn has_special_char(s: &str) -> bool {
    const SPECIAL: &str = r#"!@#$%^&*()_+\-=[]{};':"\|,.<>/?"#;
    let set: HashSet<char> = SPECIAL.chars().collect();
    s.chars().any(|c| set.contains(&c))
}

pub fn validate_password(password: &str, policy: &PasswordPolicy) -> PasswordValidationResult {
    let mut errors: Vec<String> = Vec::new();
    if password.chars().count() < policy.min_length {
        errors.push(format!("密码至少需要 {} 位", policy.min_length));
    }
    if policy.require_number && !password.chars().any(|c| c.is_ascii_digit()) {
        errors.push("密码需要包含数字".to_string());
    }
    if policy.require_uppercase && !password.chars().any(|c| c.is_ascii_uppercase()) {
        errors.push("密码需要包含大写字母".to_string());
    }
    if policy.require_special_char && !has_special_char(password) {
        errors.push("密码需要包含特殊字符".to_string());
    }
    PasswordValidationResult { valid: errors.is_empty(), errors }
}

/// 登录锁定策略（默认值来自 DEFAULT_SETTINGS.security.*）。
#[derive(Debug, Clone)]
pub struct LoginLockoutPolicy {
    pub max_attempts: u32,
    pub lockout_minutes: u32,
}

impl Default for LoginLockoutPolicy {
    fn default() -> Self {
        Self { max_attempts: 5, lockout_minutes: 30 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LockoutStatus {
    pub locked: bool,
    pub remaining_seconds: u64,
}

/// 判定当前失败次数是否触发锁定（纯函数；DB 读写由上层负责）。
pub fn evaluate_lockout(attempts: u32, policy: &LoginLockoutPolicy) -> LockoutStatus {
    if attempts >= policy.max_attempts {
        LockoutStatus { locked: true, remaining_seconds: policy.lockout_minutes as u64 * 60 }
    } else {
        LockoutStatus { locked: false, remaining_seconds: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_requires_length_and_number() {
        let policy = PasswordPolicy::default();
        assert!(!validate_password("short", &policy).valid);
        assert!(!validate_password("nonumericpassword", &policy).valid);
        assert!(validate_password("validpass1", &policy).valid);
    }

    #[test]
    fn policy_flags_are_enforced() {
        let policy = PasswordPolicy {
            min_length: 8,
            require_number: true,
            require_uppercase: true,
            require_special_char: true,
        };
        let r = validate_password("abc12345", &policy);
        assert!(!r.valid);
        assert!(r.errors.iter().any(|e| e.contains("大写字母")));
        assert!(r.errors.iter().any(|e| e.contains("特殊字符")));

        assert!(validate_password("Abc123!x", &policy).valid);
    }

    #[test]
    fn lockout_triggers_at_max_attempts() {
        let policy = LoginLockoutPolicy::default();
        assert_eq!(evaluate_lockout(4, &policy).locked, false);
        let s = evaluate_lockout(5, &policy);
        assert_eq!(s.locked, true);
        assert_eq!(s.remaining_seconds, 30 * 60);
    }
}
