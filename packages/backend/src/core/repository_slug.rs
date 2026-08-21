//! 仓库 slug 归一化（1:1 移植自 `repositoryRepository.ts` 的 `normalizeRepositorySlug`）。

/// 对齐 JS 实现：
/// ```js
/// value.trim().toLowerCase()
///   .replace(/[^a-z0-9]+/g, "-")
///   .replace(/^-+|-+$/g, "") || "repo"
/// ```
///
/// 注意 `[^a-z0-9]+` 是**贪婪连续段**替换成单个 `-`，不是逐字符替换；
/// 且它在小写化之后执行，所以中文、空格、标点会整段塌缩成一个 `-`，
/// 全部被过滤后回落到 `"repo"`。
pub fn normalize_repository_slug(value: &str) -> String {
    let lowered = value.trim().to_lowercase();

    let mut out = String::with_capacity(lowered.len());
    let mut in_gap = false;
    for ch in lowered.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            // 只有已经写过合法字符时才补 '-'，否则等价于 /^-+/ 被去掉
            if in_gap && !out.is_empty() {
                out.push('-');
            }
            in_gap = false;
            out.push(ch);
        } else {
            // 连续非法字符只塌缩成一个 '-'
            in_gap = true;
        }
    }
    // 尾部的 in_gap 不补 '-'，等价于 JS 先补再被 /-+$/ 去掉
    // 头部：第一个合法字符前的 in_gap 也不补，等价于 /^-+/ 被去掉
    if out.is_empty() {
        return "repo".to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_and_replaces_separators() {
        assert_eq!(normalize_repository_slug("My Repo"), "my-repo");
        assert_eq!(normalize_repository_slug("Foo_Bar.Baz"), "foo-bar-baz");
    }

    #[test]
    fn collapses_consecutive_illegal_chars() {
        assert_eq!(normalize_repository_slug("a   ///b"), "a-b");
        assert_eq!(normalize_repository_slug("a---b"), "a-b");
    }

    #[test]
    fn trims_leading_and_trailing_dashes() {
        assert_eq!(normalize_repository_slug("  --hello--  "), "hello");
        assert_eq!(normalize_repository_slug("///abc///"), "abc");
    }

    #[test]
    fn falls_back_to_repo_when_everything_filtered() {
        assert_eq!(normalize_repository_slug(""), "repo");
        assert_eq!(normalize_repository_slug("   "), "repo");
        assert_eq!(normalize_repository_slug("中文项目"), "repo");
        assert_eq!(normalize_repository_slug("---"), "repo");
    }

    #[test]
    fn keeps_digits_and_mixed_content() {
        assert_eq!(normalize_repository_slug("Repo 2026"), "repo-2026");
        assert_eq!(normalize_repository_slug("服务 API v2"), "api-v2");
    }
}
