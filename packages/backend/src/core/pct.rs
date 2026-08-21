//! 百分比计算（1:1 移植自 `lib/pct.ts`）。

/// 0–100 的整数百分比；分母为 0 时返回 `None`（旧后端是 `null`）。
///
/// 舍入必须与 JS `Math.round` 一致：**向正无穷**取整（`.5` 进位到更大的数），
/// 而 Rust 的 `f64::round` 是「远离零」。本函数分子分母均非负，两者等价，
/// 但仍显式用 `floor(x + 0.5)` 表达 JS 语义，避免后续被复制到有负数的场景时踩坑。
pub fn pct(numerator: i64, denominator: i64) -> Option<i64> {
    if denominator <= 0 {
        return None;
    }
    let ratio = (numerator as f64 / denominator as f64) * 100.0;
    Some((ratio + 0.5).floor() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_none_for_zero_denominator() {
        assert_eq!(pct(0, 0), None);
        assert_eq!(pct(3, 0), None);
        assert_eq!(pct(1, -2), None);
    }

    #[test]
    fn rounds_like_math_round() {
        assert_eq!(pct(1, 3), Some(33));
        assert_eq!(pct(2, 3), Some(67));
        assert_eq!(pct(1, 2), Some(50));
        assert_eq!(pct(1, 8), Some(13)); // 12.5 → 13（JS Math.round 向上）
        assert_eq!(pct(0, 5), Some(0));
        assert_eq!(pct(5, 5), Some(100));
    }
}
