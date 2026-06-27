// Artanis token-pace awareness (epic #6359).
//
// MISSION: 10x daily Khala tokens served. The concrete daily target is "at
// least 4x the prior day, goal 10x". Artanis must be able to SEE, every turn,
// whether today's served-token pace is on track to hit that target — and treat
// being-below-pace as urgent.
//
// This module is the single source of truth for:
//   - computeArtanisTokenPaceBlock: a PURE function that turns the per-day
//     Central-time history series + the Worker runtime clock into the pace block
//     (today's tokens, fraction of the Central day elapsed, the midnight
//     projection, yesterday's tokens, the 4x/10x targets, the gap, behindPace).
//     This mirrors the /stats homepage projection logic
//     (`apps/web/src/page/loggedOut/page/home.ts`: latestDayProjection) so the
//     operator agent and the public page agree.
//   - fetchArtanisNetworkStats: fetches the three live public stats endpoints
//     (all-time scalar, per-day history, model-mix) and returns a compact,
//     public-safe snapshot WITH the computed pace block. Used by the
//     `get_network_stats` operator tool (#6359) and by the situational-awareness
//     `tokenPace` reader.
//
// Public-safe only: every value here is an aggregate already exposed on /stats.
// No per-user, per-team, provider, secret, or wallet material ever enters.
//
// I/O boundary note: the live fetch is a plain async function (Promise), not an
// Effect program. That keeps the awareness reader (a `() => Promise<...>`) free
// of an Effect->Promise bridge; the get_network_stats tool wraps it with
// `Effect.promise` since the fetch is fail-soft and never rejects.

import { currentIsoTimestamp } from './runtime-primitives'

// One Central calendar day in seconds. Mirrors HISTORY_DAY_SECONDS on the
// homepage projection.
const PACE_DAY_SECONDS = 24 * 60 * 60

// The canonical timezone the /stats per-day series is bucketed in.
export const ARTANIS_TOKEN_PACE_TIMEZONE = 'America/Chicago'

// The default public base URL the operator fetches the live stats endpoints
// from. Overridable for tests.
export const ARTANIS_NETWORK_STATS_BASE_URL = 'https://openagents.com'

// A bare per-day point of the public history series (Central time).
export type ArtanisTokenHistoryPoint = Readonly<{
  day: string
  tokensServed: number
}>

// The computed pace block injected into Artanis's awareness AND returned by the
// get_network_stats tool. Everything is a public-safe aggregate.
export type ArtanisTokenPaceBlock = Readonly<{
  // The current Central calendar day (YYYY-MM-DD).
  day: string
  // Tokens served so far today (Central).
  todayTokens: number
  // Fraction of the Central day elapsed at compute time, in [0, 1].
  fractionOfCentralDayElapsed: number
  // Today's tokens extrapolated to midnight Central at the current pace.
  paceProjection: number
  // The prior Central day's total tokens (the baseline the target multiplies).
  yesterdayTokens: number
  // The daily target floor: 4x the prior day.
  target4x: number
  // The daily stretch goal: 10x the prior day.
  target10x: number
  // target4x - paceProjection (positive means we are behind the floor).
  gapToTarget4x: number
  // True when the midnight projection is below the 4x floor: URGENT.
  behindPace: boolean
}>

// The compact, public-safe network-stats snapshot the operator tool returns.
export type ArtanisNetworkStats = Readonly<{
  generatedAt: string
  timezone: string
  // All-time network-wide tokens served.
  allTimeTokensServed: number
  // Today's Central-day tokens (convenience scalar; same as pace.todayTokens).
  todayTokens: number
  // The last few days of the per-day series (Central), oldest first.
  history: ReadonlyArray<ArtanisTokenHistoryPoint>
  // Per-model-family mix over the requested window.
  modelMix: ReadonlyArray<
    Readonly<{ family: string; label: string; tokens: number; pct: number }>
  >
  // The computed pace block, or null when the series cannot ground a projection
  // (e.g. unparseable clock).
  pace: ArtanisTokenPaceBlock | null
}>

// ---------------------------------------------------------------------------
// Central-time clock helpers (ported from the homepage projection so the
// operator and /stats agree exactly).
// ---------------------------------------------------------------------------

type CentralDayParts = Readonly<{
  year: number
  month: number
  day: number
  hour: number
  minute: number
  second: number
}>

const pad2 = (value: number): string => value.toString().padStart(2, '0')

const isoTimestampMs = (value: string): number | undefined => {
  const match =
    /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,3}))?Z$/.exec(
      value,
    )
  if (match === null) return undefined
  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  const hour = Number(match[4])
  const minute = Number(match[5])
  const second = Number(match[6])
  const millisecond = Number((match[7] ?? '0').padEnd(3, '0'))
  if (
    month < 1 ||
    month > 12 ||
    day < 1 ||
    day > 31 ||
    hour > 23 ||
    minute > 59 ||
    second > 59 ||
    millisecond > 999
  ) {
    return undefined
  }
  return Date.UTC(year, month - 1, day, hour, minute, second, millisecond)
}

// The Central-time wall-clock parts for a UTC instant (epoch millis).
const centralParts = (
  timestampMs: number,
  timezone: string,
): CentralDayParts | undefined => {
  try {
    const formatter = new Intl.DateTimeFormat('en-US', {
      day: '2-digit',
      hour: '2-digit',
      hourCycle: 'h23',
      minute: '2-digit',
      month: '2-digit',
      second: '2-digit',
      timeZone: timezone,
      year: 'numeric',
    })
    const parts = formatter.formatToParts(timestampMs)
    const num = (type: Intl.DateTimeFormatPartTypes): number =>
      Number(parts.find(part => part.type === type)?.value ?? '0')
    return {
      day: num('day'),
      hour: num('hour'),
      minute: num('minute'),
      month: num('month'),
      second: num('second'),
      year: num('year'),
    }
  } catch {
    return undefined
  }
}

const dayStringFromParts = (parts: CentralDayParts): string =>
  `${parts.year}-${pad2(parts.month)}-${pad2(parts.day)}`

// ---------------------------------------------------------------------------
// The pure pace computation.
// ---------------------------------------------------------------------------

export type ComputeArtanisTokenPaceInput = Readonly<{
  // The per-day history series (Central), in any order.
  series: ReadonlyArray<ArtanisTokenHistoryPoint>
  // The Worker runtime clock as a UTC ISO timestamp (e.g. currentIsoTimestamp()).
  nowIso: string
  // The timezone the series is bucketed in (defaults to America/Chicago).
  timezone?: string | undefined
}>

// computeArtanisTokenPaceBlock — turns the history series + the Worker clock into
// the pace block, mirroring the /stats homepage projection. Returns null only
// when the clock cannot be parsed/zoned; otherwise it always returns a block
// (with zeros for missing today/yesterday rows — honest absence, not invention).
export const computeArtanisTokenPaceBlock = (
  input: ComputeArtanisTokenPaceInput,
): ArtanisTokenPaceBlock | null => {
  const timezone = input.timezone ?? ARTANIS_TOKEN_PACE_TIMEZONE
  const timestampMs = isoTimestampMs(input.nowIso)
  if (timestampMs === undefined) return null
  const parts = centralParts(timestampMs, timezone)
  if (parts === undefined) return null

  const today = dayStringFromParts(parts)
  const byDay = new Map<string, number>()
  for (const point of input.series) {
    if (typeof point.day === 'string' && Number.isFinite(point.tokensServed)) {
      byDay.set(point.day, Math.max(0, Math.trunc(point.tokensServed)))
    }
  }

  const todayTokens = byDay.get(today) ?? 0

  // Yesterday's baseline: the prior Central calendar day if present (derived by
  // re-zoning one day earlier, robust across month boundaries and DST), else the
  // most recent series day strictly before today (handles gaps honestly).
  const yesterdayParts = centralParts(
    timestampMs - PACE_DAY_SECONDS * 1000,
    timezone,
  )
  const priorDay =
    yesterdayParts !== undefined ? dayStringFromParts(yesterdayParts) : undefined
  let yesterdayTokens = priorDay !== undefined ? byDay.get(priorDay) : undefined
  if (yesterdayTokens === undefined) {
    const earlierDays = [...byDay.keys()].filter(day => day < today).sort()
    const lastEarlier = earlierDays[earlierDays.length - 1]
    yesterdayTokens = lastEarlier !== undefined ? byDay.get(lastEarlier) ?? 0 : 0
  }

  const elapsedSeconds = parts.hour * 60 * 60 + parts.minute * 60 + parts.second
  const fraction =
    elapsedSeconds <= 0 || elapsedSeconds >= PACE_DAY_SECONDS
      ? 0
      : elapsedSeconds / PACE_DAY_SECONDS

  // The midnight projection: today's tokens scaled up by the elapsed fraction,
  // never below what is already served. Degenerate fraction -> no extrapolation.
  const paceProjection =
    fraction <= 0
      ? todayTokens
      : Math.max(todayTokens, Math.round(todayTokens / fraction))

  const target4x = 4 * yesterdayTokens
  const target10x = 10 * yesterdayTokens
  const gapToTarget4x = target4x - paceProjection

  return {
    behindPace: paceProjection < target4x,
    day: today,
    fractionOfCentralDayElapsed: fraction,
    gapToTarget4x,
    paceProjection,
    target10x,
    target4x,
    todayTokens,
    yesterdayTokens,
  }
}

// A compact one-line human summary of the pace block for prompt injection /
// reporting. Public-safe.
export const formatArtanisTokenPaceLine = (
  pace: ArtanisTokenPaceBlock,
): string => {
  const pct = Math.round(pace.fractionOfCentralDayElapsed * 100)
  const status = pace.behindPace
    ? `BEHIND PACE (projection ${pace.paceProjection.toLocaleString(
        'en-US',
      )} < 4x floor ${pace.target4x.toLocaleString('en-US')}; gap ${Math.max(
        0,
        pace.gapToTarget4x,
      ).toLocaleString('en-US')})`
    : `on pace (projection ${pace.paceProjection.toLocaleString(
        'en-US',
      )} >= 4x floor ${pace.target4x.toLocaleString('en-US')})`
  return [
    `Token pace for ${pace.day} (${pct}% of the Central day elapsed):`,
    `today ${pace.todayTokens.toLocaleString('en-US')} tokens,`,
    `projecting ${pace.paceProjection.toLocaleString('en-US')} by midnight;`,
    `yesterday ${pace.yesterdayTokens.toLocaleString('en-US')};`,
    `targets 4x=${pace.target4x.toLocaleString(
      'en-US',
    )} / 10x=${pace.target10x.toLocaleString('en-US')};`,
    status + '.',
  ].join(' ')
}

// ---------------------------------------------------------------------------
// Live public-stats fetch (plain async I/O boundary; fail-soft).
// ---------------------------------------------------------------------------

export type ArtanisNetworkStatsConfig = Readonly<{
  baseUrl?: string | undefined
  fetchImpl?: typeof fetch | undefined
  nowIso?: (() => string) | undefined
  timezone?: string | undefined
  // History days to keep in the compact snapshot (oldest-first tail).
  historyDays?: number | undefined
  // Model-mix window (e.g. '30d').
  modelMixWindow?: string | undefined
}>

const safeJson = async (
  fetchImpl: typeof fetch,
  url: string,
): Promise<unknown> => {
  try {
    const response = await fetchImpl(url, {
      headers: { 'User-Agent': 'artanis-operator' },
    })
    if (!response.ok) return undefined
    return (await response.json()) as unknown
  } catch {
    return undefined
  }
}

const asInt = (value: unknown): number =>
  typeof value === 'number' && Number.isFinite(value) ? Math.trunc(value) : 0

const parseHistorySeries = (
  body: unknown,
): ReadonlyArray<ArtanisTokenHistoryPoint> => {
  const series =
    typeof body === 'object' && body !== null
      ? (body as Record<string, unknown>).series
      : undefined
  if (!Array.isArray(series)) return []
  return series
    .map(point => {
      if (typeof point !== 'object' || point === null) return null
      const record = point as Record<string, unknown>
      const day = typeof record.day === 'string' ? record.day : null
      if (day === null) return null
      return { day, tokensServed: asInt(record.tokensServed) }
    })
    .filter((point): point is ArtanisTokenHistoryPoint => point !== null)
}

const parseModelMix = (
  body: unknown,
): ReadonlyArray<
  Readonly<{ family: string; label: string; tokens: number; pct: number }>
> => {
  const groups =
    typeof body === 'object' && body !== null
      ? (body as Record<string, unknown>).groups
      : undefined
  if (!Array.isArray(groups)) return []
  return groups
    .map(group => {
      if (typeof group !== 'object' || group === null) return null
      const record = group as Record<string, unknown>
      const family = typeof record.family === 'string' ? record.family : null
      if (family === null) return null
      return {
        family,
        label: typeof record.label === 'string' ? record.label : family,
        pct:
          typeof record.pct === 'number' && Number.isFinite(record.pct)
            ? record.pct
            : 0,
        tokens: asInt(record.tokens),
      }
    })
    .filter(
      (
        group,
      ): group is Readonly<{
        family: string
        label: string
        tokens: number
        pct: number
      }> => group !== null,
    )
}

// fetchArtanisNetworkStats — fetches the three live public stats endpoints and
// returns the compact public-safe snapshot with the computed pace block. Every
// fetch is fail-soft: an unreachable or malformed endpoint degrades that bucket
// to empty/zero rather than failing the whole snapshot. Never rejects.
export const fetchArtanisNetworkStats = async (
  config: ArtanisNetworkStatsConfig = {},
): Promise<ArtanisNetworkStats> => {
  const baseUrl = (config.baseUrl ?? ARTANIS_NETWORK_STATS_BASE_URL).replace(
    /\/+$/,
    '',
  )
  const fetchImpl = config.fetchImpl ?? globalThis.fetch
  const nowIso = (config.nowIso ?? currentIsoTimestamp)()
  const timezone = config.timezone ?? ARTANIS_TOKEN_PACE_TIMEZONE
  const historyDays = config.historyDays ?? 5
  const modelMixWindow = config.modelMixWindow ?? '30d'

  const tzParam = encodeURIComponent(timezone)
  const [scalarBody, historyBody, mixBody] = await Promise.all([
    safeJson(fetchImpl, `${baseUrl}/api/public/khala-tokens-served`),
    safeJson(
      fetchImpl,
      `${baseUrl}/api/public/khala-tokens-served/history?tz=${tzParam}`,
    ),
    safeJson(
      fetchImpl,
      `${baseUrl}/api/public/khala-tokens-served/model-mix?window=${encodeURIComponent(
        modelMixWindow,
      )}`,
    ),
  ])

  const allTimeTokensServed =
    typeof scalarBody === 'object' && scalarBody !== null
      ? asInt((scalarBody as Record<string, unknown>).tokensServed)
      : 0

  const fullSeries = parseHistorySeries(historyBody)
  const sortedSeries = [...fullSeries].sort((a, b) =>
    a.day < b.day ? -1 : a.day > b.day ? 1 : 0,
  )
  const history = sortedSeries.slice(-Math.max(1, historyDays))
  const modelMix = parseModelMix(mixBody)

  const pace = computeArtanisTokenPaceBlock({
    nowIso,
    series: sortedSeries,
    timezone,
  })

  return {
    allTimeTokensServed,
    generatedAt: nowIso,
    history,
    modelMix,
    pace,
    timezone,
    todayTokens: pace?.todayTokens ?? 0,
  }
}
