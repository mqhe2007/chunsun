/** Relative time in zh (e.g. 「刚刚」「3 分钟前」「昨天」). */
export function formatRelativeTime(
  input: string | Date,
  now: Date = new Date(),
): string {
  const date = typeof input === "string" ? new Date(input) : input;
  if (Number.isNaN(date.getTime())) return "";

  const diffMs = now.getTime() - date.getTime();
  const future = diffMs < 0;
  const abs = Math.abs(diffMs);
  const sec = Math.floor(abs / 1000);
  const min = Math.floor(sec / 60);
  const hour = Math.floor(min / 60);
  const day = Math.floor(hour / 24);

  if (sec < 45) return "刚刚";
  if (min < 60) return future ? `${min} 分钟后` : `${min} 分钟前`;
  if (hour < 24) return future ? `${hour} 小时后` : `${hour} 小时前`;
  if (day === 1) return future ? "明天" : "昨天";
  if (day < 7) return future ? `${day} 天后` : `${day} 天前`;

  return date.toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "numeric",
    day: "numeric",
  });
}

/** Delta label for pulse windows, e.g. 「↑25%」「↓10%」「持平」. */
export function formatPulseDelta(current: number, previous: number): string {
  if (previous === 0 && current === 0) return "持平";
  if (previous === 0) return `↑${current}`;
  const pctChange = Math.round(((current - previous) / previous) * 100);
  if (pctChange === 0) return "持平";
  if (pctChange > 0) return `↑${pctChange}%`;
  return `↓${Math.abs(pctChange)}%`;
}
