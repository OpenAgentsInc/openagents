const MINUTE_MS = 60_000
const HOUR_MS = 60 * MINUTE_MS
const DAY_MS = 24 * HOUR_MS
const MONTH_MS = 30 * DAY_MS
const YEAR_MS = 365 * DAY_MS

const plural = (count: number, unit: string): string =>
  `${count} ${unit}${count === 1 ? '' : 's'} ago`

/** Pure, injected-`nowMs` relative time formatter ("just now", "5 minutes
 * ago", "3 hours ago", "2 days ago", ...) — `nowMs` is a parameter rather
 * than `Date.now()` so this stays deterministic/testable. */
export const formatRelativeTimeWords = (iso: string, nowMs: number): string => {
  const deltaMs = nowMs - Date.parse(iso)
  if (!Number.isFinite(deltaMs) || deltaMs < 0) return 'just now'
  if (deltaMs < MINUTE_MS) return 'just now'
  if (deltaMs < HOUR_MS) return plural(Math.floor(deltaMs / MINUTE_MS), 'minute')
  if (deltaMs < DAY_MS) return plural(Math.floor(deltaMs / HOUR_MS), 'hour')
  if (deltaMs < MONTH_MS) return plural(Math.floor(deltaMs / DAY_MS), 'day')
  if (deltaMs < YEAR_MS) return plural(Math.floor(deltaMs / MONTH_MS), 'month')
  return plural(Math.floor(deltaMs / YEAR_MS), 'year')
}
