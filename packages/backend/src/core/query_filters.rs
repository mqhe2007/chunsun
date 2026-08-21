//! 列表筛选工具：1:1 移植自 `packages/backend/src/lib/queryFilters.ts`。

/// 解析列表筛选中的逗号分隔枚举值（兼容单值）。
/// 非法片段会被丢弃；全部非法时返回 None（表示不按该字段过滤）。
pub fn parse_comma_separated_enum<T: PartialEq + Clone>(
    raw: Option<&str>,
    allowed: &[T],
    key_of: impl Fn(&T) -> &str,
) -> Option<Vec<T>> {
    let raw = raw.map(|s| s.trim()).filter(|s| !s.is_empty())?;
    let mut seen = std::collections::HashSet::new();
    let mut values: Vec<T> = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if let Some(item) = allowed.iter().find(|item| key_of(item) == part) {
            let key = key_of(item).to_string();
            if seen.insert(key) {
                values.push(item.clone());
            }
        }
    }
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

/// 单值或数组 → 数组；空输入 → None。
#[allow(dead_code)]
pub fn to_enum_list<T: Clone>(value: Option<Vec<T>>) -> Option<Vec<T>> {
    let list = value?;
    if list.is_empty() {
        None
    } else {
        Some(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUSES: &[&str] = &["open", "processing", "resolved", "closed"];

    fn parse(raw: Option<&str>) -> Option<Vec<&'static str>> {
        parse_comma_separated_enum(raw, STATUSES, |s| *s)
    }

    #[test]
    fn returns_none_for_empty_input() {
        assert_eq!(parse(None), None);
        assert_eq!(parse(Some("")), None);
        assert_eq!(parse(Some("  ")), None);
    }

    #[test]
    fn returns_single_value_for_one_valid_token() {
        assert_eq!(parse(Some("open")), Some(vec!["open"]));
    }

    #[test]
    fn returns_vec_for_multiple_valid_tokens_and_dedupes() {
        assert_eq!(parse(Some("open,processing")), Some(vec!["open", "processing"]));
        assert_eq!(parse(Some("open,open,resolved")), Some(vec!["open", "resolved"]));
    }

    #[test]
    fn drops_invalid_tokens() {
        assert_eq!(parse(Some("open,bogus,closed")), Some(vec!["open", "closed"]));
        assert_eq!(parse(Some("bogus")), None);
    }

    #[test]
    fn to_list_normalizes() {
        assert_eq!(to_enum_list(None::<Vec<&str>>), None);
        assert_eq!(to_enum_list::<&str>(Some(vec![])), None);
        assert_eq!(to_enum_list(Some(vec!["open"])), Some(vec!["open"]));
    }
}
