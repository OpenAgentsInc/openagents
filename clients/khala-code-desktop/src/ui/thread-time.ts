export const formatCompactThreadTimestamp = (
  seconds: number | null,
  nowMs: number = Date.now(),
): string => {
  if (seconds === null || !Number.isFinite(seconds)) return ""
  const normalizedSeconds = seconds > 10_000_000_000 ? seconds / 1000 : seconds

  const elapsedSeconds = Math.max(0, Math.floor((nowMs - normalizedSeconds * 1000) / 1000))
  if (elapsedSeconds < 60) return "now"

  const minutes = Math.floor(elapsedSeconds / 60)
  if (minutes < 60) return `${minutes}m`

  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h`

  const days = Math.floor(hours / 24)
  if (days < 7) return `${days}d`

  const weeks = Math.floor(days / 7)
  if (weeks < 5) return `${weeks}w`

  if (days < 365) return `${Math.max(1, Math.floor(days / 30))}mo`

  return `${Math.floor(days / 365)}y`
}
