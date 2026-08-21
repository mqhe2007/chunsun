//! 系统设置服务：DEFAULT_SETTINGS + 读取/解析（对齐 `systemSettingService.ts`）。
//! 动态策略（密码/锁定/邀请/限流/SMTP）均从 DB 读取，缺省回落默认值表。

use sqlx::PgPool;

use crate::api::AppError;
use crate::repos::system_setting;

/// DEFAULT_SETTINGS 全量默认值（与 TS 一致）。
pub const DEFAULT_SETTINGS: &[(&str, &str)] = &[
    ("registration.inviteOnly", "true"),
    ("security.passwordMinLength", "8"),
    ("security.passwordRequireNumber", "true"),
    ("security.passwordRequireUppercase", "false"),
    ("security.passwordRequireSpecialChar", "false"),
    ("security.loginMaxAttempts", "5"),
    ("security.loginLockoutMinutes", "30"),
    ("security.jwtExpiresIn", "2h"),
    ("email.fromAddress", ""),
    ("email.fromName", "春笋"),
    ("email.smtpHost", ""),
    ("email.smtpPort", "587"),
    ("email.smtpSecure", "false"),
    ("email.smtpUser", ""),
    ("email.smtpPassword", ""),
    ("rateLimit.generalMax", "1000"),
    ("rateLimit.generalWindowMs", "60000"),
    ("rateLimit.authMax", "20"),
    ("rateLimit.authWindowMs", "60000"),
];

pub fn get_default(key: &str) -> Option<&'static str> {
    DEFAULT_SETTINGS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}

pub async fn get_string_setting(pool: &PgPool, key: &str, default: Option<&str>) -> Result<String, AppError> {
    if let Some(v) = system_setting::get_setting(pool, key).await? {
        return Ok(v);
    }
    Ok(default
        .map(str::to_string)
        .or_else(|| get_default(key).map(str::to_string))
        .unwrap_or_default())
}

pub async fn get_boolean_setting(pool: &PgPool, key: &str, default: Option<bool>) -> Result<bool, AppError> {
    if let Some(v) = system_setting::get_setting(pool, key).await? {
        return Ok(v == "true");
    }
    let fallback = default.map(|b| b.to_string()).or_else(|| get_default(key).map(str::to_string));
    Ok(fallback.as_deref() == Some("true"))
}

pub async fn get_number_setting(pool: &PgPool, key: &str, default: Option<f64>) -> Result<f64, AppError> {
    if let Some(v) = system_setting::get_setting(pool, key).await? {
        if !v.is_empty() {
            if let Ok(n) = v.parse::<f64>() {
                return Ok(n);
            }
            return Ok(default.unwrap_or(0.0));
        }
    }
    if let Some(d) = get_default(key) {
        return Ok(d.parse::<f64>().unwrap_or(default.unwrap_or(0.0)));
    }
    Ok(default.unwrap_or(0.0))
}

#[derive(Debug, Clone)]
pub struct PasswordPolicy {
    pub min_length: u32,
    pub require_number: bool,
    pub require_uppercase: bool,
    pub require_special_char: bool,
}

pub async fn get_password_policy(pool: &PgPool) -> Result<PasswordPolicy, AppError> {
    Ok(PasswordPolicy {
        min_length: get_number_setting(pool, "security.passwordMinLength", Some(8.0)).await? as u32,
        require_number: get_boolean_setting(pool, "security.passwordRequireNumber", Some(true)).await?,
        require_uppercase: get_boolean_setting(pool, "security.passwordRequireUppercase", Some(false)).await?,
        require_special_char: get_boolean_setting(pool, "security.passwordRequireSpecialChar", Some(false)).await?,
    })
}

#[derive(Debug, Clone)]
pub struct LoginLockoutPolicy {
    pub max_attempts: u32,
    pub lockout_minutes: u32,
}

pub async fn get_login_lockout_policy(pool: &PgPool) -> Result<LoginLockoutPolicy, AppError> {
    Ok(LoginLockoutPolicy {
        max_attempts: get_number_setting(pool, "security.loginMaxAttempts", Some(5.0)).await? as u32,
        lockout_minutes: get_number_setting(pool, "security.loginLockoutMinutes", Some(30.0)).await? as u32,
    })
}

pub async fn is_invite_only_registration(pool: &PgPool) -> Result<bool, AppError> {
    get_boolean_setting(pool, "registration.inviteOnly", Some(true)).await
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub from_address: String,
    pub from_name: String,
    pub host: String,
    pub port: u16,
    pub secure: bool,
    pub user: String,
    pub password: String,
}

pub async fn get_smtp_config(pool: &PgPool) -> Result<SmtpConfig, AppError> {
    let from_address = get_string_setting(pool, "email.fromAddress", None).await?;
    let from_name = get_string_setting(pool, "email.fromName", Some("春笋")).await?;
    let host = get_string_setting(pool, "email.smtpHost", None).await?;
    let port_str = get_string_setting(pool, "email.smtpPort", Some("587")).await?;
    let secure_str = get_string_setting(pool, "email.smtpSecure", Some("false")).await?;
    let user = get_string_setting(pool, "email.smtpUser", None).await?;
    let password = get_string_setting(pool, "email.smtpPassword", None).await?;

    // 环境变量兜底（与 TS 一致）
    let from_address = if from_address.is_empty() {
        std::env::var("SMTP_FROM").unwrap_or_default()
    } else {
        from_address
    };
    let from_name = if from_name.is_empty() { "春笋".to_string() } else { from_name };
    let host = if host.is_empty() { std::env::var("SMTP_HOST").unwrap_or_default() } else { host };
    let port = port_str
        .parse::<u16>()
        .ok()
        .or_else(|| std::env::var("SMTP_PORT").ok().and_then(|p| p.parse().ok()))
        .unwrap_or(587);
    let secure = secure_str == "true" || std::env::var("SMTP_SECURE").as_deref() == Ok("true");
    let user = if user.is_empty() { std::env::var("SMTP_USER").unwrap_or_default() } else { user };
    let password = if password.is_empty() { std::env::var("SMTP_PASS").unwrap_or_default() } else { password };

    Ok(SmtpConfig {
        from_address,
        from_name,
        host,
        port,
        secure,
        user,
        password,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_table_has_all_entries() {
        assert_eq!(DEFAULT_SETTINGS.len(), 19);
        assert_eq!(get_default("registration.inviteOnly"), Some("true"));
        assert_eq!(get_default("registration.defaultRole"), None);
        assert_eq!(get_default("security.passwordMinLength"), Some("8"));
        assert_eq!(get_default("email.fromName"), Some("春笋"));
        assert_eq!(get_default("rateLimit.generalMax"), Some("1000"));
        assert_eq!(get_default("not-a-key"), None);
    }

    #[test]
    fn number_parsing_uses_default_fallback() {
        // get_number_setting 需要 DB；这里仅验证默认值解析
        assert_eq!(get_default("security.loginMaxAttempts").unwrap().parse::<f64>().unwrap(), 5.0);
        assert_eq!(get_default("email.smtpPort").unwrap().parse::<u16>().unwrap(), 587);
    }

    /// 【与旧后端的**有意**差异，勿"修回去"】
    ///
    /// 旧 `systemSettingService.getBooleanSetting` 写成：
    /// ```js
    /// const fallback = defaultValue ?? getDefault(key); // 调用方传的是 boolean
    /// return fallback === "true";                       // boolean === string → 恒 false
    /// ```
    /// 于是只要 system_setting 缺行，布尔类设置一律退化为 false，
    /// 导致 `registration.inviteOnly` / `security.passwordRequireNumber`
    /// 这两条 DEFAULT_SETTINGS 标为 "true" 的策略实际从未生效。
    ///
    /// Rust 版按字面意图实现（缺行 → 用 DEFAULT_SETTINGS 的值）。
    /// 现网既有行为此前已显式写入 DB 保持不变。
    #[test]
    fn boolean_defaults_follow_declared_intent_not_legacy_bug() {
        for key in ["registration.inviteOnly", "security.passwordRequireNumber"] {
            assert_eq!(
                get_default(key),
                Some("true"),
                "{key} 的声明默认值应为 true；旧后端因类型比较缺陷把它退化成了 false"
            );
        }
        // 声明为 false 的项不受影响（旧实现在这两项上"碰巧正确"）
        assert_eq!(get_default("security.passwordRequireUppercase"), Some("false"));
        assert_eq!(get_default("security.passwordRequireSpecialChar"), Some("false"));
    }
}
