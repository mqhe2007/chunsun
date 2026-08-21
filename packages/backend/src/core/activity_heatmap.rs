//! 活跃图（activity-heatmap）纯函数，1:1 移植自
//! `packages/backend/src/routes/activity.ts`（clampDays / heatmapDateRange / aggregateHeatmap）。
//! 测试向量取自 `activity.heatmap.contract.test.ts` Part B。

use chrono::{DateTime, Datelike, Days, NaiveDate, TimeZone, Utc};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// 解析 `days` 参数：缺省/非法/NaN → 84；随后夹到 [1, 366]，小数截断。
pub fn clamp_days(input: Option<f64>) -> u32 {
    match input {
        None => 84,
        Some(v) if !v.is_finite() => 84,
        Some(v) => (v.trunc() as i64).clamp(1, 366) as u32,
    }
}

fn utc_date(y: i32, m: u32, d: u32) -> DateTime<Utc> {
    let date = NaiveDate::from_ymd_opt(y, m, d)
        .unwrap_or_else(|| panic!("invalid date {y}-{m}-{d}"));
    Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
}

/// 活跃图日期窗口。两种模式：
/// 1. `year` 模式：返回该自然年全年（1 月 1 日 → 12 月 31 日）。
/// 2. `days` 模式：`end` 为「今天 0 点 (UTC)」，`start = end - (days-1) 天`，
///    `end_plus_one_day` 作为 DB 查询的排他上界（包含今天全天）。
#[derive(Debug, Clone)]
pub struct HeatmapRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub end_plus_one_day: DateTime<Utc>,
    pub window_days: u32,
}

pub fn heatmap_date_range(opts: HeatmapOpts, now: DateTime<Utc>) -> HeatmapRange {
    if let Some(year) = opts.year {
        let start = utc_date(year, 1, 1);
        let end = utc_date(year, 12, 31);
        let end_plus_one_day = end + chrono::Duration::milliseconds(DAY_MS);
        let window_days = ((end.timestamp_millis() - start.timestamp_millis()) / DAY_MS + 1) as u32;
        return HeatmapRange { start, end, end_plus_one_day, window_days };
    }

    let days = opts.days.unwrap_or(84);
    let end = utc_date(now.year(), now.month(), now.day());
    let start = end - chrono::Duration::milliseconds((days as i64 - 1) * DAY_MS);
    let end_plus_one_day = end + chrono::Duration::milliseconds(DAY_MS);
    HeatmapRange { start, end, end_plus_one_day, window_days: days }
}

#[derive(Debug, Clone, Default)]
pub struct HeatmapOpts {
    pub year: Option<i32>,
    pub days: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeatmapEntry {
    pub date: String,
    pub count: u32,
}

/// 纯函数聚合：按 UTC 日 (YYYY-MM-DD) 统计 `created_ats`，补齐窗口内每一天
/// 并升序返回。`max` 为单日最大计数（无则为 0）。
pub fn aggregate_heatmap(
    range: &HeatmapRange,
    created_ats: &[DateTime<Utc>],
) -> (u32, Vec<HeatmapEntry>) {
    let mut counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for c in created_ats {
        let ts = c.timestamp_millis();
        if ts >= range.start.timestamp_millis() && ts < range.end_plus_one_day.timestamp_millis() {
            let key = c.format("%Y-%m-%d").to_string();
            *counts.entry(key).or_insert(0) += 1;
        }
    }

    let mut entries = Vec::new();
    let mut day = range.start;
    while day <= range.end {
        let date = day.format("%Y-%m-%d").to_string();
        entries.push(HeatmapEntry { date: date.clone(), count: counts.get(&date).copied().unwrap_or(0) });
        day = day + Days::new(1);
    }

    let max = entries.iter().map(|e| e.count).max().unwrap_or(0);
    (max, entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 固定 now，与 TS 契约测试一致：UTC 2026-07-28
    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 28, 10, 30, 0).unwrap()
    }

    fn build(opts: HeatmapOpts, created: &[DateTime<Utc>]) -> (u32, Vec<HeatmapEntry>, u32) {
        let range = heatmap_date_range(opts, now());
        let (max, entries) = aggregate_heatmap(&range, created);
        (max, entries, range.window_days)
    }

    fn date(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        utc_date(y, m, d)
    }

    fn at_hms(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, s).unwrap()
    }

    #[test]
    fn default_window_returns_84_entries_ending_today() {
        assert_eq!(clamp_days(None), 84);
        let (max, entries, window_days) = build(HeatmapOpts { days: Some(84), ..Default::default() }, &[]);
        assert_eq!(window_days, 84);
        assert_eq!(entries.len(), 84);
        assert_eq!(max, 0);
        assert_eq!(entries.last().unwrap().date, "2026-07-28");
        assert!(entries.iter().all(|e| e.count == 0));
    }

    #[test]
    fn days_7_returns_7_ascending_unique() {
        let (_, entries, _) = build(HeatmapOpts { days: Some(7), ..Default::default() }, &[]);
        assert_eq!(entries.len(), 7);
        let dates: Vec<String> = entries.iter().map(|e| e.date.clone()).collect();
        let mut sorted = dates.clone();
        sorted.sort();
        assert_eq!(dates, sorted);
        let unique: std::collections::HashSet<&String> = dates.iter().collect();
        assert_eq!(unique.len(), 7);
        assert_eq!(dates[0], "2026-07-22");
        assert_eq!(dates[6], "2026-07-28");
    }

    #[test]
    fn counts_activities_by_day_and_max() {
        let seeded = vec![
            at_hms(2026, 7, 28, 5, 0, 0),
            at_hms(2026, 7, 28, 18, 0, 0),
            at_hms(2026, 7, 28, 23, 59, 0),
            at_hms(2026, 7, 26, 12, 0, 0),
        ];
        let (max, entries, _) = build(HeatmapOpts { days: Some(7), ..Default::default() }, &seeded);
        let by: std::collections::HashMap<&str, u32> =
            entries.iter().map(|e| (e.date.as_str(), e.count)).collect();
        assert_eq!(by.get("2026-07-28"), Some(&3));
        assert_eq!(by.get("2026-07-26"), Some(&1));
        assert_eq!(by.get("2026-07-25"), Some(&0));
        assert_eq!(by.get("2026-07-22"), Some(&0));
        assert_eq!(max, 3);
    }

    #[test]
    fn outside_window_excluded_inclusive_start_exclusive_end() {
        let range = heatmap_date_range(HeatmapOpts { days: Some(7), ..Default::default() }, now());
        let out = vec![
            at_hms(2026, 7, 21, 23, 59, 0), // 早于 start
            range.end_plus_one_day,                          // 恰好 endPlusOneDay（排他）
            date(2026, 6, 1),                                // 远早于窗口
        ];
        let (max, entries) = aggregate_heatmap(&range, &out);
        assert_eq!(max, 0);
        assert!(entries.iter().all(|e| e.count == 0));

        let at_start = vec![range.start];
        let (_, r2) = aggregate_heatmap(&range, &at_start);
        assert_eq!(r2[0].count, 1);
    }

    #[test]
    fn clamp_days_edge_cases() {
        assert_eq!(clamp_days(None), 84);
        assert_eq!(clamp_days(Some(f64::NAN)), 84);
        assert_eq!(clamp_days(Some(-5.0)), 1);
        assert_eq!(clamp_days(Some(50.7)), 50);
        assert_eq!(clamp_days(Some(366.0)), 366);
        assert_eq!(clamp_days(Some(367.0)), 366);
        assert_eq!(clamp_days(Some(999.0)), 366);
        assert_eq!(clamp_days(Some(0.0)), 1);

        let (_, entries, window) = build(HeatmapOpts { days: Some(366), ..Default::default() }, &[]);
        assert_eq!(entries.len(), 366);
        assert_eq!(window, 366);
        assert_eq!(entries.last().unwrap().date, "2026-07-28");
    }

    #[test]
    fn year_mode_full_calendar_year() {
        let (_, entries, window) = build(HeatmapOpts { year: Some(2026), ..Default::default() }, &[]);
        assert_eq!(window, 365);
        assert_eq!(entries.len(), 365);
        assert_eq!(entries[0].date, "2026-01-01");
        assert_eq!(entries.last().unwrap().date, "2026-12-31");
    }

    #[test]
    fn leap_year_has_366_days() {
        let (_, entries, window) = build(HeatmapOpts { year: Some(2024), ..Default::default() }, &[]);
        assert_eq!(window, 366);
        assert_eq!(entries.len(), 366);
        let dates: Vec<&str> = entries.iter().map(|e| e.date.as_str()).collect();
        assert!(dates.contains(&"2024-02-29"));
        assert_eq!(dates[0], "2024-01-01");
        assert_eq!(dates[365], "2024-12-31");
    }

    #[test]
    fn year_mode_only_counts_within_year() {
        let seeded = vec![
            at_hms(2026, 1, 15, 10, 0, 0),
            at_hms(2026, 12, 31, 23, 0, 0),
            at_hms(2025, 12, 31, 23, 0, 0),
            at_hms(2027, 1, 1, 1, 0, 0),
        ];
        let (max, entries, _) = build(HeatmapOpts { year: Some(2026), ..Default::default() }, &seeded);
        let by: std::collections::HashMap<&str, u32> =
            entries.iter().map(|e| (e.date.as_str(), e.count)).collect();
        assert_eq!(by.get("2026-01-15"), Some(&1));
        assert_eq!(by.get("2026-12-31"), Some(&1));
        assert!(!by.contains_key("2025-12-31"));
        assert!(!by.contains_key("2027-01-01"));
        assert_eq!(max, 1);
    }
}
