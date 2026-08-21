//! serde 补丁：区分「字段缺省」与「字段显式为 null」。
//!
//! # 为什么需要
//!
//! TypeBox 的 `t.Optional(t.String())` 有两种截然不同的入参：
//!
//! | 请求体                      | TypeBox        | 期望行为          |
//! |-----------------------------|----------------|-------------------|
//! | `{}`                        | 字段缺省       | 不更新 / 走默认值 |
//! | `{"description": null}`     | 违反 `string`  | **422**           |
//!
//! 直觉上 `Option<Option<String>>` 就能表达这两种状态，但 serde 默认把 JSON `null`
//! 反序列化成**外层** `None`——与「字段缺省」撞成同一个值，于是显式 null 被静默放行，
//! 旧后端 422 / 新后端 200，直接产生 DIFF（project 域对拍实测踩到过两次）。
//!
//! 正确做法是让外层 `Option` 只由 `#[serde(default)]` 在字段缺省时填 `None`，
//! 字段存在时无论值是不是 null 都进入 `Some(_)`：
//!
//! ```ignore
//! #[derive(Deserialize)]
//! struct Body {
//!     #[serde(default, deserialize_with = "crate::core::serde_ext::double_option")]
//!     description: Option<Option<String>>,
//! }
//! ```
//!
//! 结果：缺省 → `None`；`null` → `Some(None)`；`"x"` → `Some(Some("x"))`。

use serde::{Deserialize, Deserializer};

/// 配合 `#[serde(default)]` 使用，把「字段存在但值为 null」保留成 `Some(None)`。
pub fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Body {
        #[serde(default, deserialize_with = "double_option")]
        name: Option<Option<String>>,
    }

    #[test]
    fn absent_null_and_value_are_three_distinct_states() {
        let absent: Body = serde_json::from_str("{}").unwrap();
        assert_eq!(absent.name, None);

        let explicit_null: Body = serde_json::from_str(r#"{"name":null}"#).unwrap();
        assert_eq!(explicit_null.name, Some(None));

        let value: Body = serde_json::from_str(r#"{"name":"x"}"#).unwrap();
        assert_eq!(value.name, Some(Some("x".to_string())));
    }

    #[test]
    fn wrong_type_still_fails_to_deserialize() {
        assert!(serde_json::from_str::<Body>(r#"{"name":123}"#).is_err());
    }
}
