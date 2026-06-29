import type {
  CodingCodexSession,
  CodingTranscriptMessage,
} from "./coding-status"

export type ApmPoint = {
  readonly actionCount: number
  readonly apm: number
  readonly endMs: number
  readonly label: string
  readonly startMs: number
}

export type ApmStats = {
  readonly actionCount: number
  readonly activeSessionCount: number
  readonly currentApm: number
  readonly firstActionAt: string | null
  readonly lastActionAt: string | null
  readonly peakApm: number
  readonly recentApm: number
  readonly series: readonly ApmPoint[]
  readonly sessionCount: number
  readonly source: "coding_sessions"
  readonly windowMinutes: number
}

const APM_WINDOW_MINUTES = 60
const APM_BUCKET_COUNT = 24
const MINUTE_MS = 60_000

const apmNumberFormatter = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 1,
  minimumFractionDigits: 1,
})

export const formatApm = (value: number): string =>
  apmNumberFormatter.format(Number.isFinite(value) ? Math.max(0, value) : 0)

const actionWeight = (message: CodingTranscriptMessage): number => {
  if (message.timestamp === null) return 0
  if (message.role === "user" || message.role === "assistant") return 1
  if (message.role === "tool" && message.kind === "tool call") return 1
  return 0
}

const timestampMs = (message: CodingTranscriptMessage): number | null => {
  if (message.timestamp === null) return null
  const millis = Date.parse(message.timestamp)
  return Number.isFinite(millis) ? millis : null
}

const collectActionTimestamps = (
  sessions: readonly CodingCodexSession[],
): readonly number[] =>
  sessions
    .flatMap(session =>
      session.messages.flatMap(message => {
        const weight = actionWeight(message)
        const millis = timestampMs(message)
        if (weight === 0 || millis === null) return []
        return Array.from({ length: weight }, () => millis)
      }),
    )
    .sort((left, right) => left - right)

const activeDurationMinutes = (timestamps: readonly number[]): number => {
  if (timestamps.length < 2) return timestamps.length === 0 ? 0 : 1
  const spanMinutes =
    (timestamps[timestamps.length - 1] - timestamps[0]) / MINUTE_MS
  return Math.max(1, spanMinutes)
}

const pointLabel = (endMs: number): string =>
  new Intl.DateTimeFormat("en-US", {
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(endMs))

export const calculateApmStats = (
  sessions: readonly CodingCodexSession[],
  nowMs = Date.now(),
): ApmStats => {
  const timestamps = collectActionTimestamps(sessions)
  const first = timestamps.at(0) ?? null
  const last = timestamps.at(-1) ?? null
  const currentApm =
    timestamps.length === 0 ? 0 : timestamps.length / activeDurationMinutes(timestamps)
  const recentStartMs = nowMs - APM_WINDOW_MINUTES * MINUTE_MS
  const recentTimestamps = timestamps.filter(timestamp => timestamp >= recentStartMs)
  const recentApm = recentTimestamps.length / APM_WINDOW_MINUTES
  const bucketMinutes = APM_WINDOW_MINUTES / APM_BUCKET_COUNT
  const bucketMs = bucketMinutes * MINUTE_MS
  const series = Array.from({ length: APM_BUCKET_COUNT }, (_, index): ApmPoint => {
    const startMs = recentStartMs + index * bucketMs
    const endMs = startMs + bucketMs
    const actionCount = timestamps.filter(
      timestamp => timestamp >= startMs && timestamp < endMs,
    ).length
    return {
      actionCount,
      apm: actionCount / bucketMinutes,
      endMs,
      label: pointLabel(endMs),
      startMs,
    }
  })
  const peakApm = series.reduce(
    (peak, point) => (point.apm > peak ? point.apm : peak),
    0,
  )

  return {
    actionCount: timestamps.length,
    activeSessionCount: sessions.filter(session => session.active).length,
    currentApm,
    firstActionAt: first === null ? null : new Date(first).toISOString(),
    lastActionAt: last === null ? null : new Date(last).toISOString(),
    peakApm,
    recentApm,
    series,
    sessionCount: sessions.length,
    source: "coding_sessions",
    windowMinutes: APM_WINDOW_MINUTES,
  }
}

export const apmSummaryText = (stats: ApmStats): string =>
  stats.actionCount === 0
    ? "No timestamped Coding actions loaded yet."
    : `${formatApm(stats.currentApm)} APM across ${stats.actionCount} actions from ${stats.sessionCount} sessions. Last ${stats.windowMinutes}m: ${formatApm(stats.recentApm)} APM; peak bucket ${formatApm(stats.peakApm)} APM.`
