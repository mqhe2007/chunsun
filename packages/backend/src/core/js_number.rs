//! JS `Number(string)` 与 `JSON.stringify(number)` 的语义复刻。
//!
//! 分页参数在旧后端是 `query.page ? Number(query.page) : 1`，随后原样回写进 `meta`。
//! 这意味着几个**必须逐字节对齐**的行为：
//!
//! - `Number("")` → `0`（但路由里空串是 falsy，走默认值，不会到这里）
//! - `Number("abc")` → `NaN`，而 `JSON.stringify(NaN)` → `null`
//! - `pageSize = 0` 时 `Math.ceil(total / 0)` → `Infinity` → 序列化同样是 `null`
//! - `Number("2")` 回写是 `2` 而不是 `2.0`（Rust f64 默认会序列化成 `2.0`，是 DIFF 源）
//!
//! 另一个隐藏语义：旧仓储 `if (!options.page || !options.pageSize)` 用的是 **falsy 判断**，
//! 所以 `0` 和 `NaN` 都会退化成「不分页、返回全量」，而不是报错。见 `Pagination::resolve`。

use serde_json::Value;

/// 复刻 JS 全局 `Number(string)` 的字符串转换规则。
pub fn js_number(input: &str) -> f64 {
    let s = input.trim();
    if s.is_empty() {
        return 0.0;
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16)
            .map(|v| v as f64)
            .unwrap_or(f64::NAN);
    }
    match s {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    // Rust 的 f64 parser 比 JS 宽松：接受 "inf" / "infinity" / "NAN" 等大小写变体，
    // 而 JS 只认严格大小写的 "Infinity"。这里显式拒绝，避免放行出 DIFF。
    let lower = s.to_ascii_lowercase();
    if lower.contains("inf") || lower == "nan" {
        return f64::NAN;
    }
    s.parse::<f64>().unwrap_or(f64::NAN)
}

/// 复刻 `JSON.stringify(n)`：非有限数 → `null`；整数值不带小数点。
pub fn to_json_number(value: f64) -> Value {
    if !value.is_finite() {
        return Value::Null;
    }
    if value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 {
        // -0 在 JSON.stringify 下输出 0
        return Value::from(value as i64);
    }
    serde_json::Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

/// Prisma **写入 `Int` 列**时的取整语义。
///
/// 实测（`PUT /projects/:id/contexts/:docId` 的 `sortOrder`）：
/// - `3.7 → 3`、`-3.7 → -3`、`-0.5 → 0`：**向零截断**，静默放行不报错
/// - `1e3 → 1000`、`1.5e3 → 1500`：指数记法只是 JSON 数字的写法，同样截断
/// - `2147483647` 合法；`2147483648` / `-2147483649` 撞 PG `int4` 范围，
///   Prisma 抛 "Value out of range for the type"，旧后端未捕获 → **500**
///
/// 与 [`Pagination::resolve`] 的区别值得记牢：**查询参数**上的 `take` 反而不做这套
/// 校验（实测 `take: 3.7` 返回 3 条、`take: -99999999999` 返回全量、都不报错），
/// 因为它不落库、只进 SQL 的 LIMIT。落库的列才受 `int4` 约束。两处别互相套用。
///
/// 返回 `Err(())` 表示越界，调用方应映射成 500。
#[allow(clippy::result_unit_err)]
pub fn prisma_int(value: f64) -> Result<i32, ()> {
    let truncated = value.trunc();
    if !truncated.is_finite()
        || truncated < f64::from(i32::MIN)
        || truncated > f64::from(i32::MAX)
    {
        return Err(());
    }
    Ok(truncated as i32)
}

/// `Math.ceil(total / page_size)`，保留 Infinity/NaN 语义交给 [`to_json_number`] 处理。
pub fn total_pages(total: i64, page_size: f64) -> f64 {
    (total as f64 / page_size).ceil()
}

/// 分页窗口。`None` 表示旧后端的「不分页，返回全量」分支。
///
/// `take` **可以为负**：Prisma 的 `take: -N` 表示「从末尾往回取 N 条，但结果仍按
/// 原排序返回」。这不是错误路径，`pageSize=-5` 在旧后端会正常 200。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pagination {
    pub skip: i64,
    pub take: i64,
}

impl Pagination {
    /// 对齐 `if (!options.page || !options.pageSize) { 返回全量 }` 的 falsy 判断。
    ///
    /// 返回 `Err(())` 表示 skip/take 落到 Prisma 会直接抛错的区间，
    /// 调用方应映射为 500，与旧后端未捕获异常的表现一致。Prisma 的实际约束是：
    /// - `skip` 必须是**非负** Int（负数报 "Value can only be positive"）
    /// - `skip` / `take` 必须是**整数**且落在 Int32 范围内
    /// - `take` 允许为负（反向取），因此不在拒绝之列
    #[allow(clippy::result_unit_err)]
    pub fn resolve(page: f64, page_size: f64) -> Result<Option<Self>, ()> {
        // JS falsy：0、-0、NaN
        let falsy = |v: f64| v == 0.0 || v.is_nan();
        if falsy(page) || falsy(page_size) {
            return Ok(None);
        }
        if !page.is_finite() || !page_size.is_finite() {
            return Err(());
        }
        let skip = (page - 1.0) * page_size;
        if skip < 0.0 {
            return Err(());
        }
        let as_int32 = |v: f64| -> Result<i64, ()> {
            if v.fract() != 0.0 || v < i32::MIN as f64 || v > i32::MAX as f64 {
                return Err(());
            }
            Ok(v as i64)
        };
        Ok(Some(Pagination {
            skip: as_int32(skip)?,
            take: as_int32(page_size)?,
        }))
    }

    /// 负 take 需要把 SQL 排序方向翻过来再取，最后在内存里还原顺序。
    /// 返回 `(是否反转排序, LIMIT 值)`。
    pub fn sql_window(&self) -> (bool, i64) {
        if self.take < 0 {
            (true, -self.take)
        } else {
            (false, self.take)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_like_js_number() {
        assert_eq!(js_number("2"), 2.0);
        assert_eq!(js_number(" 3 "), 3.0);
        assert_eq!(js_number(""), 0.0);
        assert_eq!(js_number("1.5"), 1.5);
        assert_eq!(js_number("0x1A"), 26.0);
        assert_eq!(js_number("Infinity"), f64::INFINITY);
        assert!(js_number("abc").is_nan());
        // JS 不接受这些大小写变体
        assert!(js_number("inf").is_nan());
        assert!(js_number("infinity").is_nan());
    }

    #[test]
    fn serializes_integers_without_decimal_point() {
        assert_eq!(to_json_number(2.0).to_string(), "2");
        assert_eq!(to_json_number(-0.0).to_string(), "0");
        assert_eq!(to_json_number(1.5).to_string(), "1.5");
    }

    #[test]
    fn non_finite_becomes_null_like_json_stringify() {
        assert_eq!(to_json_number(f64::NAN), Value::Null);
        assert_eq!(to_json_number(f64::INFINITY), Value::Null);
    }

    #[test]
    fn total_pages_matches_math_ceil() {
        assert_eq!(total_pages(21, 20.0), 2.0);
        assert_eq!(total_pages(0, 20.0), 0.0);
        assert!(total_pages(5, 0.0).is_infinite());
        assert!(total_pages(5, f64::NAN).is_nan());
    }

    #[test]
    fn zero_or_nan_degrades_to_unpaginated() {
        assert_eq!(Pagination::resolve(0.0, 20.0), Ok(None));
        assert_eq!(Pagination::resolve(1.0, 0.0), Ok(None));
        assert_eq!(Pagination::resolve(f64::NAN, 20.0), Ok(None));
    }

    #[test]
    fn normal_window() {
        assert_eq!(
            Pagination::resolve(2.0, 20.0),
            Ok(Some(Pagination { skip: 20, take: 20 }))
        );
    }

    #[test]
    fn negative_page_is_an_error_like_prisma() {
        // skip = (-1 - 1) * 20 = -40 → Prisma "Value can only be positive"
        assert_eq!(Pagination::resolve(-1.0, 20.0), Err(()));
    }

    #[test]
    fn infinite_page_is_an_error_like_prisma() {
        assert_eq!(Pagination::resolve(f64::INFINITY, 20.0), Err(()));
    }

    #[test]
    fn negative_page_size_is_a_backwards_take_not_an_error() {
        // pageSize=-5 在旧后端是合法的：skip=(1-1)*-5=0、take=-5 → 反向取 5 条
        assert_eq!(
            Pagination::resolve(1.0, -5.0),
            Ok(Some(Pagination { skip: 0, take: -5 }))
        );
        assert_eq!(Pagination { skip: 0, take: -5 }.sql_window(), (true, 5));
        assert_eq!(Pagination { skip: 0, take: 5 }.sql_window(), (false, 5));
        // 负 pageSize 下 Math.ceil(4 / -5) = -0 → JSON 输出 0
        assert_eq!(to_json_number(total_pages(4, -5.0)).to_string(), "0");
    }

    #[test]
    fn prisma_int_truncates_toward_zero() {
        assert_eq!(prisma_int(3.7), Ok(3));
        assert_eq!(prisma_int(-3.7), Ok(-3));
        assert_eq!(prisma_int(-0.5), Ok(0));
        assert_eq!(prisma_int(1.5e3), Ok(1500));
        assert_eq!(prisma_int(0.0), Ok(0));
    }

    #[test]
    fn prisma_int_rejects_out_of_int32_range() {
        assert_eq!(prisma_int(2_147_483_647.0), Ok(i32::MAX));
        assert_eq!(prisma_int(-2_147_483_648.0), Ok(i32::MIN));
        assert_eq!(prisma_int(2_147_483_648.0), Err(()));
        assert_eq!(prisma_int(-2_147_483_649.0), Err(()));
        // 截断发生在范围判断之前：2147483647.9 截断后仍在范围内
        assert_eq!(prisma_int(2_147_483_647.9), Ok(i32::MAX));
    }

    #[test]
    fn non_integer_window_is_an_error_like_prisma_int() {
        // page=1.5 & pageSize=2 → skip=1.0 是整数，合法
        assert_eq!(
            Pagination::resolve(1.5, 2.0),
            Ok(Some(Pagination { skip: 1, take: 2 }))
        );
        // pageSize=2.5 → take 非整数，Prisma 的 Int 会拒绝
        assert_eq!(Pagination::resolve(1.0, 2.5), Err(()));
        // skip 溢出 Int32
        assert_eq!(Pagination::resolve(3.0e9, 2.0), Err(()));
    }
}
