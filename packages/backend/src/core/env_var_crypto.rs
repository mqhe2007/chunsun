//! 环境变量 at-rest 加密（AES-256-GCM），1:1 移植自
//! `packages/backend/src/lib/envVarCrypto.ts`（node:crypto → aes-gcm crate）。
//!
//! 封套格式（与旧后端严格一致，保证存量密文可解）：
//! `enc:v1:<iv base64url>:<authTag base64url>:<ciphertext base64url>`
//! 密钥解析优先级：ENV_VAR_ENCRYPTION_KEY（hex64 / base64(32B) / sha256(utf8)）
//! → 缺省从 JWT_SECRET 派生：sha256("chunsun:project-env-var:" + jwt_secret)。

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::api::AppError;

pub const ENVELOPE_PREFIX: &str = "enc:v1:";

/// 解析 32 字节密钥。
pub fn resolve_env_var_encryption_key(
    env_var_encryption_key: Option<&str>,
    jwt_secret: &str,
) -> Result<Vec<u8>, AppError> {
    if let Some(explicit) = env_var_encryption_key.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        // hex64
        if explicit.len() == 64 && explicit.chars().all(|c| c.is_ascii_hexdigit()) {
            return hex::decode(explicit).map_err(|_| AppError::internal("加密密钥 hex 解码失败"));
        }
        // base64(32B)
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(explicit) {
            if decoded.len() == 32 {
                return Ok(decoded);
            }
        }
        // 其他字符串 → sha256(utf8)
        return Ok(Sha256::digest(explicit.as_bytes()).to_vec());
    }

    let jwt_secret = jwt_secret.trim();
    if jwt_secret.is_empty() {
        return Err(AppError::internal("ENV_VAR_ENCRYPTION_KEY 或 JWT_SECRET 未设置，无法加密项目环境变量"));
    }
    Ok(Sha256::digest(format!("chunsun:project-env-var:{jwt_secret}").as_bytes()).to_vec())
}

pub fn is_encrypted_env_envelope(stored: &str) -> bool {
    stored.starts_with(ENVELOPE_PREFIX)
}

/// 写入 DB：is_secret 时 AES-256-GCM 封存；否则存明文。
pub fn seal_env_var_value(
    plain: &str,
    is_secret: bool,
    env_var_encryption_key: Option<&str>,
    jwt_secret: &str,
) -> Result<String, AppError> {
    if !is_secret {
        return Ok(plain.to_string());
    }
    let key = resolve_env_var_encryption_key(env_var_encryption_key, jwt_secret)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| AppError::internal("AES 密钥初始化失败"))?;
    let mut iv = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut iv);
    let nonce = Nonce::from_slice(&iv);

    // Aead::encrypt 输出 = ciphertext || tag
    let ct_with_tag = cipher
        .encrypt(nonce, plain.as_bytes())
        .map_err(|_| AppError::internal("环境变量加密失败"))?;
    let (ciphertext, tag) = ct_with_tag.split_at(ct_with_tag.len() - 16);

    let enc = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    Ok(format!(
        "{ENVELOPE_PREFIX}{}:{}:{}",
        enc.encode(iv),
        enc.encode(tag),
        enc.encode(ciphertext)
    ))
}

/// 从 DB 读出明文；兼容 P0 遗留明文与 enc:v1 封套。
pub fn open_env_var_value(
    stored: &str,
    env_var_encryption_key: Option<&str>,
    jwt_secret: &str,
) -> Result<String, AppError> {
    if !is_encrypted_env_envelope(stored) {
        return Ok(stored.to_string());
    }

    let parts: Vec<&str> = stored[ENVELOPE_PREFIX.len()..].split(':').collect();
    if parts.len() != 3 {
        return Err(AppError::internal("环境变量密文格式无效"));
    }
    let enc = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let iv = enc
        .decode(parts[0])
        .map_err(|_| AppError::internal("环境变量密文 iv 无效"))?;
    let tag = enc
        .decode(parts[1])
        .map_err(|_| AppError::internal("环境变量密文 tag 无效"))?;
    let ciphertext = enc
        .decode(parts[2])
        .map_err(|_| AppError::internal("环境变量密文数据无效"))?;

    let key = resolve_env_var_encryption_key(env_var_encryption_key, jwt_secret)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| AppError::internal("AES 密钥初始化失败"))?;
    let nonce = Nonce::from_slice(&iv);

    let mut ct_with_tag = ciphertext;
    ct_with_tag.extend_from_slice(&tag);
    let plain = cipher
        .decrypt(nonce, ct_with_tag.as_slice())
        .map_err(|_| AppError::internal("环境变量解密失败"))?;
    String::from_utf8(plain).map_err(|_| AppError::internal("环境变量明文非 UTF-8"))
}

/// 列表 hasValue：不解密；加密封套视为有值。
pub fn env_var_has_stored_value(stored: &str) -> bool {
    if is_encrypted_env_envelope(stored) {
        return true;
    }
    !stored.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_JWT_SECRET: &str = "test-jwt-secret-for-env-var-crypto";

    #[test]
    fn does_not_encrypt_non_secret_values() {
        let sealed = seal_env_var_value("plain-value", false, None, TEST_JWT_SECRET).unwrap();
        assert_eq!(sealed, "plain-value");
        assert!(!is_encrypted_env_envelope(&sealed));
        assert_eq!(open_env_var_value(&sealed, None, TEST_JWT_SECRET).unwrap(), "plain-value");
    }

    #[test]
    fn encrypts_and_decrypts_secret_values() {
        let sealed = seal_env_var_value("s3cret!", true, None, TEST_JWT_SECRET).unwrap();
        assert!(is_encrypted_env_envelope(&sealed));
        assert!(!sealed.contains("s3cret!"));
        assert_eq!(open_env_var_value(&sealed, None, TEST_JWT_SECRET).unwrap(), "s3cret!");
        assert!(env_var_has_stored_value(&sealed));
    }

    #[test]
    fn round_trips_empty_secret_string() {
        let sealed = seal_env_var_value("", true, None, TEST_JWT_SECRET).unwrap();
        assert!(is_encrypted_env_envelope(&sealed));
        assert_eq!(open_env_var_value(&sealed, None, TEST_JWT_SECRET).unwrap(), "");
    }

    #[test]
    fn opens_legacy_plaintext_for_migration() {
        assert_eq!(open_env_var_value("legacy-plain", None, TEST_JWT_SECRET).unwrap(), "legacy-plain");
    }

    #[test]
    fn different_seals_for_same_plaintext() {
        let a = seal_env_var_value("same", true, None, TEST_JWT_SECRET).unwrap();
        let b = seal_env_var_value("same", true, None, TEST_JWT_SECRET).unwrap();
        assert_ne!(a, b);
        assert_eq!(open_env_var_value(&a, None, TEST_JWT_SECRET).unwrap(), "same");
        assert_eq!(open_env_var_value(&b, None, TEST_JWT_SECRET).unwrap(), "same");
    }

    #[test]
    fn key_derivation_matches_ts_spec() {
        // sha256("chunsun:project-env-var:" + jwt_secret)
        let key = resolve_env_var_encryption_key(None, TEST_JWT_SECRET).unwrap();
        let expected = Sha256::digest(format!("chunsun:project-env-var:{TEST_JWT_SECRET}").as_bytes());
        assert_eq!(key, expected.to_vec());
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn explicit_hex_and_base64_keys_are_accepted() {
        let hex_key = "a".repeat(64);
        let k = resolve_env_var_encryption_key(Some(&hex_key), TEST_JWT_SECRET).unwrap();
        assert_eq!(k, vec![0xaa; 32]);

        let b64_key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
        let k = resolve_env_var_encryption_key(Some(&b64_key), TEST_JWT_SECRET).unwrap();
        assert_eq!(k, vec![7u8; 32]);
    }

    #[test]
    fn missing_keys_error() {
        assert!(resolve_env_var_encryption_key(None, "").is_err());
        assert!(resolve_env_var_encryption_key(None, "   ").is_err());
    }

    /// 跨实现兼容：该封套由旧后端（node:crypto / envVarCrypto.ts）在相同 JWT_SECRET 下
    /// 对明文 "cross-check-plaintext" 生成，证明 Rust 侧可解开存量密文。
    #[test]
    fn opens_envelope_produced_by_node_impl() {
        let fixture = "enc:v1:_hxVsww1ihFsOPG3:amzdqXSFrNTtlpkEED3fHA:qV3_Up6EJRJvyDu7MS8Gjg6H7zKd";
        assert!(is_encrypted_env_envelope(fixture));
        assert_eq!(
            open_env_var_value(fixture, None, TEST_JWT_SECRET).unwrap(),
            "cross-check-plaintext"
        );
    }
}
