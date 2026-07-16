// Live public data for the `/stats` route.
//
// Every fetcher here is fail-soft: any network, HTTP, or parse error resolves
// to `null`, and the page renders the same honest "Unavailable" state it
// server-renders before the first fetch. No dummy values, ever.
//
// Endpoint inventory (verified against production 2026-07-16):
// - GET /api/public/khala-tokens-served              -> headline counter
// - GET /api/public/khala-tokens-served/history      -> daily bar chart
// - GET /api/public/khala-tokens-served/model-mix    -> model-family mix
// - GET /api/public/khala-tokens-served/channel-mix  -> channel mix
// - GET /api/public/pylon-stats                      -> pylon + accounting + relay panels
// - GET /api/forum/launch-status                     -> forum launch/tip gates
// - GET /api/forum/tip-leaderboards                  -> RETIRED: returns HTTP 410
//   `money_surface_retired` (2026-07-14), so forum tip totals stay Unavailable.

export const TOKENS_SERVED_URL = '/api/public/khala-tokens-served'
export const TOKENS_SERVED_HISTORY_URL = '/api/public/khala-tokens-served/history'
export const TOKENS_SERVED_MODEL_MIX_URL = '/api/public/khala-tokens-served/model-mix'
export const TOKENS_SERVED_CHANNEL_MIX_URL = '/api/public/khala-tokens-served/channel-mix'
export const STATS_PYLON_STATS_URL = '/api/public/pylon-stats'
export const FORUM_LAUNCH_STATUS_URL = '/api/forum/launch-status'

export type TokensServedSnapshot = {
  readonly tokensServed?: number
  readonly generatedAt?: string
}

export type TokensServedHistoryPoint = {
  readonly day?: string
  readonly tokensServed?: number
}

export type TokensServedHistorySnapshot = {
  readonly window?: string
  readonly timezone?: string
  readonly series?: ReadonlyArray<TokensServedHistoryPoint>
}

export type MixGroup = {
  readonly label?: string
  readonly pct?: number
  readonly reqs?: number
  readonly tokens?: number
}

export type MixSnapshot = {
  readonly window?: string
  readonly totalTokens?: number
  readonly groups?: ReadonlyArray<MixGroup>
}

export type SettlementGateSnapshot = {
  readonly state?: string
  readonly stateLabel?: string
  readonly settledReceiptRefs?: ReadonlyArray<string>
}

export type EarningGateSnapshot = {
  readonly state?: string
  readonly stateLabel?: string
}

export type RecentPylonSnapshot = {
  readonly nostrPubkeyShort?: string
  readonly relayUrls?: ReadonlyArray<string>
}

export type StatsPylonSnapshot = {
  readonly available?: boolean
  readonly status?: string
  readonly asOfLabel?: string
  readonly pylonsOnlineNow?: number
  readonly pylonsSeen24h?: number
  readonly pylonsAssignmentReadyNow?: number
  readonly earningLaunchGate?: EarningGateSnapshot | null
  readonly nexusAcceptedWorkPayoutSatsPaidTotal?: number
  readonly nexusAcceptedWorkSettlementGate?: SettlementGateSnapshot | null
  readonly hostedNexusRelayUrl?: string | null
  readonly recentPylons?: ReadonlyArray<RecentPylonSnapshot>
}

export type ForumLaunchGate = {
  readonly id?: string
  readonly label?: string
  readonly state?: string
}

export type ForumLaunchStatusSnapshot = {
  readonly status?: string
  readonly summary?: string
  readonly orangeChecksSold?: number
  readonly gates?: ReadonlyArray<ForumLaunchGate>
  readonly publicTipping?: { readonly gates?: ReadonlyArray<ForumLaunchGate> }
}

// Fail-soft JSON fetch: any error -> null (rendered as Unavailable).
export const fetchPublicJson = async <T>(
  url: string,
  fetchFn: typeof fetch = fetch,
): Promise<T | null> => {
  try {
    const response = await fetchFn(url, { headers: { accept: 'application/json' } })
    if (!response.ok) return null
    return (await response.json()) as T
  } catch {
    return null
  }
}

// A panel's data is loading (first paint), ok (live evidence), or
// unavailable (fetch failed / evidence missing). Never a fabricated value.
export type Loadable<T> =
  | { readonly state: 'loading' }
  | { readonly state: 'ok'; readonly data: T }
  | { readonly state: 'unavailable' }

export const toLoadable = <T>(data: T | null): Loadable<T> =>
  data === null ? { state: 'unavailable' } : { state: 'ok', data }

const finite = (value: number | null | undefined): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? value : null

export const formatCount = (value: number): string => Math.round(value).toLocaleString('en-US')

// Headline counter display: formatted live total, or the honest `—`.
export const tokensServedDisplay = (
  snapshot: Loadable<TokensServedSnapshot>,
): { readonly value: string; readonly live: boolean } => {
  if (snapshot.state !== 'ok') return { live: false, value: '—' }
  const tokens = finite(snapshot.data.tokensServed)
  if (tokens === null) return { live: false, value: '—' }
  return { live: true, value: formatCount(tokens) }
}

export type HistoryMetric = 'cumulative' | 'daily'

export type HistoryBar = {
  readonly day: string
  readonly heightPct: number
  readonly tokens: number
}

// Daily or running-cumulative bars, heights normalized to the series max.
export const historyBars = (
  snapshot: TokensServedHistorySnapshot,
  metric: HistoryMetric,
): ReadonlyArray<HistoryBar> => {
  const points = (snapshot.series ?? []).flatMap(point => {
    const tokens = finite(point.tokensServed)
    return typeof point.day === 'string' && tokens !== null && tokens >= 0
      ? [{ day: point.day, tokens }]
      : []
  })
  const values =
    metric === 'daily'
      ? points.map(point => point.tokens)
      : points.reduce<Array<number>>((acc, point) => {
          acc.push((acc[acc.length - 1] ?? 0) + point.tokens)
          return acc
        }, [])
  const max = values.reduce((a, b) => Math.max(a, b), 0)
  return points.map((point, index) => {
    const value = values[index] ?? 0
    return {
      day: point.day,
      heightPct: max > 0 ? Math.max((value / max) * 100, value > 0 ? 2 : 0) : 0,
      tokens: value,
    }
  })
}

export type MixRow = {
  readonly detail: string
  readonly label: string
  readonly pct: string
}

export const mixRows = (snapshot: MixSnapshot): ReadonlyArray<MixRow> =>
  (snapshot.groups ?? []).flatMap(group => {
    const pct = finite(group.pct)
    const tokens = finite(group.tokens)
    if (typeof group.label !== 'string' || pct === null || tokens === null) return []
    const reqs = finite(group.reqs)
    return [
      {
        detail: `${formatCount(tokens)} tokens${reqs === null ? '' : ` · ${formatCount(reqs)} reqs`}`,
        label: group.label,
        pct: `${pct.toFixed(pct >= 10 ? 1 : 2)}%`,
      },
    ]
  })

export type PylonPanelValues = {
  readonly assignedNow: string
  readonly earningGate: string
  readonly earningGateReady: boolean
  readonly meta: string
  readonly onlineNow: string
  readonly seen24h: string
}

export const pylonPanelValues = (snapshot: StatsPylonSnapshot): PylonPanelValues | null => {
  if (snapshot.available === false || snapshot.status === 'unavailable') return null
  const online = finite(snapshot.pylonsOnlineNow)
  const seen = finite(snapshot.pylonsSeen24h)
  const assigned = finite(snapshot.pylonsAssignmentReadyNow)
  const gate = snapshot.earningLaunchGate
  return {
    assignedNow: assigned === null ? 'Unavailable' : formatCount(assigned),
    earningGate: gate?.stateLabel ?? gate?.state ?? 'Unavailable',
    earningGateReady: gate?.state === 'ready',
    meta:
      typeof snapshot.asOfLabel === 'string'
        ? `Heartbeat freshness: ${snapshot.asOfLabel}.`
        : 'Heartbeat freshness unavailable.',
    onlineNow: online === null ? 'Unavailable' : formatCount(online),
    seen24h: seen === null ? 'Unavailable' : formatCount(seen),
  }
}

export type AccountingPanelValues = {
  readonly acceptedWorkGate: string
  readonly acceptedWorkGateReady: boolean
  readonly acceptedWorkSatsPaid: string
  readonly settlementRefs: string
}

export const accountingPanelValues = (
  snapshot: StatsPylonSnapshot,
): AccountingPanelValues | null => {
  if (snapshot.available === false || snapshot.status === 'unavailable') return null
  const satsPaid = finite(snapshot.nexusAcceptedWorkPayoutSatsPaidTotal)
  const gate = snapshot.nexusAcceptedWorkSettlementGate
  const refCount = gate?.settledReceiptRefs?.length
  return {
    acceptedWorkGate: gate?.stateLabel ?? gate?.state ?? 'Unavailable',
    acceptedWorkGateReady: gate?.state === 'ready',
    acceptedWorkSatsPaid: satsPaid === null ? 'Unavailable' : `${formatCount(satsPaid)} sats`,
    settlementRefs:
      typeof refCount === 'number' ? `${formatCount(refCount)} receipts` : 'Unavailable',
  }
}

export type NostrPanelValues = {
  readonly pubkeys: string
  readonly relayUrls: string
}

export const nostrPanelValues = (snapshot: StatsPylonSnapshot): NostrPanelValues | null => {
  if (snapshot.available === false || snapshot.status === 'unavailable') return null
  const recent = snapshot.recentPylons
  if (recent === undefined) return null
  const relayUrls = new Set<string>(
    typeof snapshot.hostedNexusRelayUrl === 'string' ? [snapshot.hostedNexusRelayUrl] : [],
  )
  const pubkeys = new Set<string>()
  recent.forEach(pylon => {
    ;(pylon.relayUrls ?? []).forEach(url => relayUrls.add(url))
    if (typeof pylon.nostrPubkeyShort === 'string' && pylon.nostrPubkeyShort.length > 0) {
      pubkeys.add(pylon.nostrPubkeyShort)
    }
  })
  return {
    pubkeys: `${formatCount(pubkeys.size)} recent`,
    relayUrls: `${formatCount(relayUrls.size)} published`,
  }
}

export type ForumPanelValues = {
  readonly meta: string
  readonly orangeChecksSold: string
  readonly tipGate: string
  readonly tipGateReady: boolean
}

export const forumPanelValues = (
  snapshot: ForumLaunchStatusSnapshot,
): ForumPanelValues | null => {
  const tippingGates = snapshot.publicTipping?.gates
  const tipGateReady =
    tippingGates !== undefined &&
    tippingGates.length > 0 &&
    tippingGates.every(gate => gate.state === 'ready')
  const orangeChecks = finite(snapshot.orangeChecksSold)
  return {
    meta:
      typeof snapshot.summary === 'string'
        ? snapshot.summary
        : 'Forum launch summary unavailable.',
    orangeChecksSold: orangeChecks === null ? 'Unavailable' : formatCount(orangeChecks),
    tipGate:
      tippingGates === undefined ? 'Unavailable' : tipGateReady ? 'Ready' : 'Gated',
    tipGateReady,
  }
}
