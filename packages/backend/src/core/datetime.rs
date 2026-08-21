//! 时间戳序列化：截断到毫秒，对齐 Prisma/JS `Date.toISOString()` 的 3 位毫秒精度。
//!
//! 直接 `DateTime<Utc>` 经 chrono 默认 serde（`to_rfc3339()` → `SecondsFormat::Secs`）会丢掉亚秒，
//! 与旧后端（Elysia 用 JS `Date` 序列化，恒为 `2026-08-09T12:34:56.789Z`）产生 DIFF。
//! 故统一走本模块的 `to_value`。

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;

/// 把 `DateTime<Utc>` 格式化为 3 位毫秒精度的 RFC3339 字符串。
///
/// PG `timestamptz(6)` 存微秒，但 JS `Date` 只有毫秒精度；为保证与旧后端逐字节一致，
/// 截断到毫秒（micros 部分直接丢弃，与 JS `toISOString` 行为相同）。
pub fn format_millis(dt: &DateTime<Utc>) -> String {
    let millis = dt.timestamp_millis();
    let secs = millis.div_euclid(1000);
    let ms = millis.rem_euclid(1000) as u32;
    let normalized = DateTime::<Utc>::from_timestamp(secs, ms * 1_000_000).unwrap_or(*dt);
    normalized.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// 转为 `serde_json::Value::String`，便于在 `json!({...})` 构建响应时内联使用。
pub fn to_value(dt: &DateTime<Utc>) -> Value {
    Value::String(format_millis(dt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn truncates_micros_to_millis_like_js() {
        // PG 微秒精度 123456μs → JS 只保留 123ms
        let dt = Utc.with_ymd_and_hms(2026, 8, 9, 12, 34, 56).unwrap()
            + chrono::Duration::microseconds(123_456);
        assert_eq!(format_millis(&dt), "2026-08-09T12:34:56.123Z");
    }

    #[test]
    fn whole_seconds_still_gets_dot_zero() {
        let dt = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
        assert_eq!(format_millis(&dt), "2026-01-02T03:04:05.000Z");
    }
}
