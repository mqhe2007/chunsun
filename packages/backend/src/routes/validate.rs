//! 路由层入参校验（对齐旧后端 Elysia + TypeBox 的 schema 语义）。
//!
//! 三个校验器覆盖 TypeBox 的三种字段形态，**返回值形状刻意不同**，逼调用方在类型上
//! 就把「字段缺省」和「显式 null」分开处理：
//!
//! | TypeBox 声明 | 校验器 | 缺省 | 显式 `null` | 合法值 |
//! | --- | --- | --- | --- | --- |
//! | `t.String(...)` | [`required_string`] | 422 | 422 | `&str` |
//! | `t.Optional(t.String(...))` | [`optional_string`] | `None` | 422 | `Some(&str)` |
//! | `t.Optional(t.Nullable(t.String(...)))` | [`nullable_optional_string`] | `None` | `Some(None)` | `Some(Some(&str))` |
//!
//! 入参一律建模成 `Option<Option<String>>`，且**必须**配
//! [`crate::core::serde_ext::double_option`]：serde 默认会把 JSON `null` 折叠进外层
//! `None`，与「字段缺省」撞车，导致显式 null 被静默放行（project 域曾因此误建过数据）。

use crate::api::AppError;

/// 旧后端 TypeBox 校验失败固定 422；这里统一成标准包络。
pub fn validation_error(message: impl Into<String>) -> AppError {
    AppError::unprocessable("VALIDATION_ERROR").with_message(message)
}

/// JS `String.prototype.length` 语义：按 **UTF-16 code unit** 计数，不是字符数。
///
/// TypeBox 的 `minLength` / `maxLength` 直接比对 `value.length`，所以 BMP 之外的字符
/// （emoji、部分生僻字）在 JS 里占 2。用 `chars().count()` 会把 60 个 emoji 算成 60，
/// 而旧后端算 120——`maxLength: 100` 时一个放行一个 422，是真实可观测的 DIFF。
fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn check_len(field: &str, s: &str, min: usize, max: usize) -> Result<(), AppError> {
    let len = js_len(s);
    if len < min || len > max {
        return Err(validation_error(format!(
            "{field} 长度需在 {min}~{max} 之间"
        )));
    }
    Ok(())
}

/// `t.String({ minLength, maxLength })`：缺省和 null 一律 422。
pub fn required_string<'a>(
    field: &str,
    value: &'a Option<Option<String>>,
    min: usize,
    max: usize,
) -> Result<&'a str, AppError> {
    match value {
        None => Err(validation_error(format!("{field} 为必填项"))),
        Some(None) => Err(validation_error(format!("{field} 不能为 null"))),
        Some(Some(s)) => {
            check_len(field, s, min, max)?;
            Ok(s.as_str())
        }
    }
}

/// `t.Optional(t.String({ minLength, maxLength }))`：缺省放行，null 422。
pub fn optional_string<'a>(
    field: &str,
    value: &'a Option<Option<String>>,
    min: usize,
    max: usize,
) -> Result<Option<&'a str>, AppError> {
    match value {
        None => Ok(None),
        Some(None) => Err(validation_error(format!("{field} 不能为 null"))),
        Some(Some(s)) => {
            check_len(field, s, min, max)?;
            Ok(Some(s.as_str()))
        }
    }
}

/// `t.Optional(t.Nullable(t.String({ minLength, maxLength })))`：缺省与 null 都合法。
///
/// 返回三态而不是直接摊平成 `Option<&str>`：写库时「不提供」与「显式清空」在
/// PATCH 端点上语义不同，摊平会把这个区别永久丢掉。POST 端点若两者同义，
/// 调用方自己 `.flatten()` 即可。
pub fn nullable_optional_string<'a>(
    field: &str,
    value: &'a Option<Option<String>>,
    min: usize,
    max: usize,
) -> Result<Option<Option<&'a str>>, AppError> {
    match value {
        None => Ok(None),
        Some(None) => Ok(Some(None)),
        Some(Some(s)) => {
            check_len(field, s, min, max)?;
            Ok(Some(Some(s.as_str())))
        }
    }
}

/// `t.Optional(t.Boolean())`：缺省放行，显式 null 走 422。
///
/// 与 [`optional_string`] 同构，只是没有长度维度。布尔字段同样要过 `double_option`：
/// `{"isSecret": null}` 在旧后端是 422，若折叠成「缺省」就会静默套用默认值。
pub fn optional_bool(field: &str, value: &Option<Option<bool>>) -> Result<Option<bool>, AppError> {
    match value {
        None => Ok(None),
        Some(None) => Err(validation_error(format!("{field} 不能为 null"))),
        Some(Some(b)) => Ok(Some(*b)),
    }
}

/// `t.Optional(t.Number())`：缺省放行，显式 null 走 422。
///
/// 只管「是不是数字」，**不管整数性**——TypeBox 的 `t.Number()` 接受任意 JSON 数字，
/// `3.7` / `1e3` 全部放行，取整发生在写库那一刻（见
/// [`crate::core::js_number::prisma_int`]）。非数字类型（`"3"` / `true`）在 serde
/// 反序列化阶段就已经失败，同样落 422，与旧后端一致。
pub fn optional_number(field: &str, value: &Option<Option<f64>>) -> Result<Option<f64>, AppError> {
    match value {
        None => Ok(None),
        Some(None) => Err(validation_error(format!("{field} 不能为 null"))),
        Some(Some(n)) => Ok(Some(*n)),
    }
}

/// 乐观锁版本号：**必填正整数**，缺失 / null / 负数 / 非整数一律 400 `MISSING_REVISION`。
///
/// 与上面几个校验器的三个不同，都不是随手定的：
///
/// 1. **状态码是 400 而不是 422**。422 在这里会让 Agent 误以为「请求体形状不对，改改再发」，
///    而缺版本号的真实含义是「你还没读过这行」——补救动作是**先去读一次**，不是改 body。
///    用 400 把它和 schema 层面（TypeBox 语义）的 422 分开。
/// 2. **不配 `double_option`**。`Option<Option<i32>>` 的三态是为了区分「不提供」与「显式清空」，
///    而版本号没有「清空」语义：`null` 与缺省的下场完全相同（都是没告诉我你基于哪一版），
///    多一层反而要在这里把 `Some(None)` 重新折叠回 `None`。直接 `Option<i32>`，
///    但**必须**配 `#[serde(default)]`，否则字段缺失时 serde 报错落 422。
///
/// 类型是 `i32` 而非 `i64`：`revision` 列在库里是 `INTEGER`（INT4），sqlx 的
/// `FromRow` 按类型严格解码，`i64` 会在**运行时**报
/// `mismatched types; Rust type i64 (as SQL type INT8) is not compatible with SQL type INT4`
/// —— 单元测试碰不到真列，编译期也拦不住。仓库里其余整数列（`sort_order`、`index`）
/// 也都是 `INTEGER` ↔ `i32`，这里对齐同一惯例。
/// 3. **不做「缺失时自动拉取当前版本」的兜底**。那次读与本次写之间的窗口正是要防的竞态，
///    自动补等于把保护拆掉（方案里已定：Agent 必须先显式读一次）。
///
/// `0` 是合法值，表示「我认为这行还不存在」——首次写宪法 / 首次写某需求记忆时用它。
/// 负数没有对应语义，直接拒。
pub fn required_revision(field: &str, value: Option<i32>) -> Result<i32, AppError> {
    let Some(n) = value else {
        return Err(AppError::bad_request("MISSING_REVISION")
            .with_message(format!("{field} 为必填项：请先读取一次拿到当前版本号再写"))
            .with_hint("首次创建传 0；更新传你读到的那一版"));
    };
    if n < 0 {
        return Err(AppError::bad_request("MISSING_REVISION")
            .with_message(format!("{field} 不能为负数"))
            .with_hint("首次创建传 0；更新传你读到的那一版"));
    }
    Ok(n)
}

/// `t.Optional(t.Union([t.Literal("a"), t.Literal("b"), …]))`：缺省放行，
/// null 与不在白名单里的值都 422。
///
/// 与 [`optional_string`] 的区别是没有长度维度、改为**成员校验**。注意这里是
/// 严格相等，不 trim、不忽略大小写——TypeBox 的 `t.Literal` 就是严格相等。
pub fn optional_enum<'a>(
    field: &str,
    value: &'a Option<Option<String>>,
    allowed: &[&str],
) -> Result<Option<&'a str>, AppError> {
    match value {
        None => Ok(None),
        Some(None) => Err(validation_error(format!("{field} 不能为 null"))),
        Some(Some(s)) => {
            if !allowed.contains(&s.as_str()) {
                return Err(validation_error(format!(
                    "{field} 只能是 {} 之一",
                    allowed.join(" / ")
                )));
            }
            Ok(Some(s.as_str()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn missing() -> Option<Option<String>> {
        None
    }
    fn explicit_null() -> Option<Option<String>> {
        Some(None)
    }
    fn value(s: &str) -> Option<Option<String>> {
        Some(Some(s.to_string()))
    }

    /// 版本号的四态：缺省 / null / 负数全部 400，只有非负整数放行。
    ///
    /// 两条断言值得单独说：
    /// - **`null` 与缺省同下场**。这是它不用 `double_option` 的直接后果，
    ///   也是刻意的：JSON 客户端传 `"revision": null` 与不传，表达的都是
    ///   「我说不清基于哪一版」，没有第三种解释。
    /// - **`0` 放行**。`0` 不是「没有版本号」，是「我认为这行还不存在」，
    ///   首次建宪法 / 首次写记忆时的合法值。把 `0` 当 falsy 拒掉会让首次创建永远写不进去。
    #[test]
    fn required_revision_rejects_missing_null_and_negative_but_allows_zero() {
        let err = required_revision("revision", None).expect_err("缺省必须 400");
        assert_eq!(err.code, "MISSING_REVISION");

        // 显式 null 走到校验器时已经是 None：`Option<i32>` 没配 `double_option`，
        // serde 把 `"revision": null` 与字段缺失折叠成同一个值。
        // 这里从 JSON 侧验一遍，确认折叠确实发生（而不是靠 `None` 手动构造蒙混过关）。
        let from_null: Option<i32> = serde_json::from_str(r#"{"revision": null}"#)
            .map(|v: serde_json::Value| v.get("revision").and_then(|r| r.as_i64()))
            .unwrap()
            .map(|n| n as i32);
        let err = required_revision("revision", from_null).expect_err("null 同样 400");
        assert_eq!(err.code, "MISSING_REVISION");

        let err = required_revision("revision", Some(-1)).expect_err("负数必须 400");
        assert_eq!(err.code, "MISSING_REVISION");

        assert_eq!(required_revision("revision", Some(0)).unwrap(), 0);
        assert_eq!(required_revision("revision", Some(7)).unwrap(), 7);
    }

    #[test]
    fn required_rejects_missing_and_null() {
        assert!(required_string("name", &missing(), 1, 100).is_err());
        assert!(required_string("name", &explicit_null(), 1, 100).is_err());
        assert_eq!(required_string("name", &value("hi"), 1, 100).unwrap(), "hi");
    }

    #[test]
    fn optional_allows_missing_but_rejects_null() {
        assert_eq!(optional_string("slug", &missing(), 1, 100).unwrap(), None);
        assert!(optional_string("slug", &explicit_null(), 1, 100).is_err());
        assert_eq!(
            optional_string("slug", &value("a"), 1, 100).unwrap(),
            Some("a")
        );
    }

    #[test]
    fn nullable_optional_keeps_three_states() {
        assert_eq!(
            nullable_optional_string("rootHint", &missing(), 1, 500).unwrap(),
            None
        );
        assert_eq!(
            nullable_optional_string("rootHint", &explicit_null(), 1, 500).unwrap(),
            Some(None)
        );
        assert_eq!(
            nullable_optional_string("rootHint", &value("."), 1, 500).unwrap(),
            Some(Some("."))
        );
    }

    #[test]
    fn empty_string_fails_min_length_one() {
        assert!(required_string("name", &value(""), 1, 100).is_err());
        assert!(optional_string("slug", &value(""), 1, 100).is_err());
        assert!(nullable_optional_string("rootHint", &value(""), 1, 500).is_err());
    }

    #[test]
    fn optional_bool_rejects_only_explicit_null() {
        assert_eq!(optional_bool("isSecret", &None).unwrap(), None);
        assert!(optional_bool("isSecret", &Some(None)).is_err());
        assert_eq!(optional_bool("isSecret", &Some(Some(false))).unwrap(), Some(false));
        assert_eq!(optional_bool("isSecret", &Some(Some(true))).unwrap(), Some(true));
    }

    #[test]
    fn optional_enum_is_strict_membership() {
        const COVERAGE: &[&str] = &["none", "partial", "full"];
        assert_eq!(optional_enum("coverage", &missing(), COVERAGE).unwrap(), None);
        assert!(optional_enum("coverage", &explicit_null(), COVERAGE).is_err());
        assert_eq!(
            optional_enum("coverage", &value("full"), COVERAGE).unwrap(),
            Some("full")
        );
        // 严格相等：大小写与空白都不容错
        assert!(optional_enum("coverage", &value("Full"), COVERAGE).is_err());
        assert!(optional_enum("coverage", &value(" full"), COVERAGE).is_err());
        assert!(optional_enum("coverage", &value(""), COVERAGE).is_err());
    }

    #[test]
    fn length_counts_utf16_code_units_like_js() {
        // 中文在 BMP 内，JS length 与字符数一致
        let cjk = "中".repeat(100);
        assert!(required_string("name", &value(&cjk), 1, 100).is_ok());
        assert!(required_string("name", &value(&"中".repeat(101)), 1, 100).is_err());

        // emoji 是代理对，JS 里一个占 2 —— 51 个即 102 > 100，必须 422
        let emoji = "🍜".repeat(51);
        assert_eq!(emoji.chars().count(), 51, "字符数视角只有 51");
        assert!(
            required_string("name", &value(&emoji), 1, 100).is_err(),
            "按 JS length 应为 102，超出 maxLength=100"
        );
        assert!(required_string("name", &value(&"🍜".repeat(50)), 1, 100).is_ok());
    }
}
