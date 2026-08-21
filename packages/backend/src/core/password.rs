//! 密码哈希：新写入 Argon2id；存量 bcrypt（含历史 bcryptjs `$2a$`/`$2b$`）校验后登录时升级。

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use argon2::password_hash::rand_core::OsRng;

use crate::api::AppError;

const BCRYPT_PREFIX: &str = "$2";
const ARGON2_PREFIX: &str = "$argon2";

/// 新密码一律 Argon2id（OWASP 默认参数）。
pub fn hash_password(plain: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| {
            tracing::error!(error = %e, "argon2 hash failed");
            AppError::internal("密码哈希失败")
        })?;
    Ok(hash.to_string())
}

pub fn verify_password(plain: &str, stored: &str) -> Result<bool, AppError> {
    if stored.starts_with(BCRYPT_PREFIX) {
        return bcrypt::verify(plain, stored).map_err(|e| {
            tracing::error!(error = %e, "bcrypt verify failed");
            AppError::internal("密码校验失败")
        });
    }
    if stored.starts_with(ARGON2_PREFIX) {
        let parsed = PasswordHash::new(stored).map_err(|e| {
            tracing::error!(error = %e, "argon2 hash parse failed");
            AppError::internal("密码校验失败")
        })?;
        return Ok(Argon2::default()
            .verify_password(plain.as_bytes(), &parsed)
            .is_ok());
    }
    tracing::warn!(prefix = &stored[..stored.len().min(8)], "unknown password hash format");
    Ok(false)
}

/// 存量 bcrypt 哈希在成功校验后应升级为 Argon2。
pub fn needs_rehash(stored: &str) -> bool {
    stored.starts_with(BCRYPT_PREFIX)
}

/// 校验通过后，若仍为 bcrypt 则返回新 Argon2 哈希供写库。
pub fn rehash_if_legacy(plain: &str, stored: &str) -> Result<Option<String>, AppError> {
    if needs_rehash(stored) {
        Ok(Some(hash_password(plain)?))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 由旧后端 bcryptjs 生成的哈希（cost 12 / cost 10），证明存量密码可无缝校验。
    const BCRYPTJS_HASH_12: &str = "$2b$12$YvplT0X5dHzCG.ps2d22l.cpAnXGrQmojismdrn.MXnd7q1cyWEO.";
    const BCRYPTJS_HASH_10: &str = "$2b$10$l8bs.ZmxevPtA3N0.wZ8j.0s1Nnb53UGF.4.4PA5BHu/bcWShd8we";
    const PLAIN: &str = "S3cretPass!2026";

    #[test]
    fn verifies_bcryptjs_hash_cost12() {
        assert!(verify_password(PLAIN, BCRYPTJS_HASH_12).unwrap());
        assert!(!verify_password("wrong-password", BCRYPTJS_HASH_12).unwrap());
    }

    #[test]
    fn verifies_bcryptjs_hash_cost10() {
        assert!(verify_password(PLAIN, BCRYPTJS_HASH_10).unwrap());
    }

    #[test]
    fn argon2_roundtrip_hash_and_verify() {
        let h = hash_password(PLAIN).unwrap();
        assert!(h.starts_with(ARGON2_PREFIX));
        assert!(verify_password(PLAIN, &h).unwrap());
        assert!(!verify_password("nope", &h).unwrap());
        assert_ne!(h, BCRYPTJS_HASH_12);
    }

    #[test]
    fn needs_rehash_only_for_bcrypt() {
        assert!(needs_rehash(BCRYPTJS_HASH_12));
        let h = hash_password(PLAIN).unwrap();
        assert!(!needs_rehash(&h));
    }

    #[test]
    fn rehash_if_legacy_upgrades_bcrypt() {
        let upgraded = rehash_if_legacy(PLAIN, BCRYPTJS_HASH_12)
            .unwrap()
            .expect("bcrypt should upgrade");
        assert!(upgraded.starts_with(ARGON2_PREFIX));
        assert!(verify_password(PLAIN, &upgraded).unwrap());
    }

    #[test]
    fn rehash_if_legacy_skips_argon2() {
        let h = hash_password(PLAIN).unwrap();
        assert!(rehash_if_legacy(PLAIN, &h).unwrap().is_none());
    }
}
