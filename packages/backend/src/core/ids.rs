//! 主键生成：对齐 Prisma `@default(nanoid(N))`。
//!
//! **关键兼容点**：Prisma 的 `nanoid()` 是**应用层**默认值，DDL 里 `id TEXT NOT NULL` 并没有
//! 数据库 DEFAULT。因此 Rust 侧每次 INSERT 都必须显式生成主键，否则会违反 NOT NULL。
//!
//! 字母表沿用 nanoid.js 的 `urlAlphabet`（Prisma 内部使用的那一套），
//! 保证新旧数据的 ID 形状完全一致（同字符集、同长度）。

/// nanoid.js 的 urlAlphabet（64 字符，URL 安全）。
const URL_ALPHABET: [char; 64] = [
    'u', 's', 'e', 'a', 'n', 'd', 'o', 'm', '-', '2', '6', 'T', '1', '9', '8', '3', '4', '0', 'P',
    'X', '7', '5', 'p', 'x', 'J', 'A', 'C', 'K', 'V', 'E', 'R', 'Y', 'M', 'I', 'N', 'D', 'B', 'U',
    'S', 'H', 'W', 'O', 'L', 'F', '_', 'G', 'Q', 'Z', 'b', 'f', 'g', 'h', 'j', 'k', 'l', 'q', 'v',
    'w', 'y', 'z', 'r', 'i', 'c', 't',
];

/// 生成指定长度的 nanoid。
pub fn nanoid(size: usize) -> String {
    nanoid::nanoid!(size, &URL_ALPHABET)
}

/// 本仓库绝大多数模型使用 `nanoid(16)`。
pub fn nanoid16() -> String {
    nanoid(16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generates_requested_length() {
        assert_eq!(nanoid16().chars().count(), 16);
        assert_eq!(nanoid(12).chars().count(), 12);
        assert_eq!(nanoid(10).chars().count(), 10);
    }

    #[test]
    fn uses_url_safe_alphabet_only() {
        let allowed: HashSet<char> = URL_ALPHABET.iter().copied().collect();
        for _ in 0..200 {
            for c in nanoid16().chars() {
                assert!(allowed.contains(&c), "unexpected char {c:?}");
            }
        }
    }

    #[test]
    fn alphabet_matches_nanoid_url_alphabet() {
        // nanoid.js urlAlphabet 常量，逐字符比对，防止手抄漏字符
        let expected = "useandom-26T198340PX75pxJACKVERYMINDBUSHWOLF_GQZbfghjklqvwyzrict";
        let actual: String = URL_ALPHABET.iter().collect();
        assert_eq!(actual, expected);
        assert_eq!(URL_ALPHABET.len(), 64);
    }

    #[test]
    fn ids_are_unique_enough() {
        let set: HashSet<String> = (0..2000).map(|_| nanoid16()).collect();
        assert_eq!(set.len(), 2000);
    }
}
