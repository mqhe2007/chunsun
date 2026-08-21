//! Token 工具：1:1 移植自 `packages/backend/src/lib/tokens.ts`。

use rand::RngCore;
use sha2::{Digest, Sha256};

/// 生成加密安全随机 token，默认 32 字节（64 位 hex）。
pub fn generate_secure_token(length: usize) -> String {
    let mut buf = vec![0u8; length];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// 对 token 做 sha256 哈希，用于需要防泄露的场景（如数据库泄露时无法反查原始 token）。
#[allow(dead_code)]
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_hex_token_of_requested_length() {
        let t = generate_secure_token(32);
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(t, generate_secure_token(32));
    }

    #[test]
    fn hash_token_matches_sha256_hex() {
        // 与 node:crypto createHash("sha256").update("abc").digest("hex") 一致
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(hash_token("abc"), expected);
    }
}
