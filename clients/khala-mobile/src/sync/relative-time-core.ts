const MINUTE_MS = 60_000
const HOUR_MS = 60 * MINUTE_MS
const DAY_MS = 24 * HOUR_MS

/** Pure, injected-`nowMs` relative time formatter ("just now", "5m", "3h", "2d"). */
export const formatRelativeTime = (iso: string, nowMs: number): string => {
  const deltaMs = nowMs - Date.parse(iso)
  if (!Number.isFinite(deltaMs) || deltaMs < 0) return "just now"
  if (deltaMs < MINUTE_MS) return "just now"
  if (deltaMs < HOUR_MS) return `${Math.floor(deltaMs / MINUTE_MS)}m`
  if (deltaMs < DAY_MS) return `${Math.floor(deltaMs / HOUR_MS)}h`
  return `${Math.floor(deltaMs / DAY_MS)}d`
}
