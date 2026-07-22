// Client fetchers for the admin operator dashboard (#9188).
//
// The primary source is the admin-gated `/api/admin/operator/overview`
// snapshot (agent chains + tokens + traces + fleet + cloud health). The
// dashboard ALSO polls the existing owner-scoped live endpoints the snapshot
// deliberately leaves as `unavailable` markers — Full Auto runs, FleetRuns,
// and the ops health strip — and composes them client-side. All requests
// carry the session cookie; a non-admin gets a 403 on the overview and the
// page shows the refusal view.

export const ADMIN_OPERATOR_OVERVIEW_URL = '/api/admin/operator/overview'

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

export type OverviewSnapshot = Readonly<{
  ok: boolean
  generatedAt: string
  agentChains: {
    activeCount: number
    recentCount: number
    chains: ReadonlyArray<AgentChain>
  }
  tokens: TokenRollup
  traces: ReadonlyArray<TraceSummary>
  fleet: FleetSummary
  cloudHealth: Record<string, HealthSignal>
}>

export type AgentChain = Readonly<{
  assignmentRef: string
  pylonRef: string
  ownerUserId: string
  jobKind: string
  state: string
  active: boolean
  leaseExpiresAt: string
  createdAt: string
  updatedAt: string
  projection: unknown
  events: ReadonlyArray<{
    eventRef: string
    eventKind: string
    status: string
    createdAt: string
    projection: unknown
  }>
}>

export type TokenRollup = Readonly<{
  total: { events: number; tokens: number }
  last24h: { events: number; tokens: number }
  byDemandSource: ReadonlyArray<{
    demandSource: string
    events: number
    tokens: number
  }>
  byProvider: ReadonlyArray<{ provider: string; events: number; tokens: number }>
  recent: ReadonlyArray<{
    observedAt: string
    provider: string | null
    model: string | null
    demandSource: string | null
    demandKind: string | null
    totalTokens: number
    usageTruth: string
  }>
}>

export type TraceSummary = Readonly<{
  traceUuid: string
  ownerUserId: string
  agentRef: string
  schemaVersion: string
  visibility: string
  stepCount: number
  demandKind: string | null
  demandSource: string | null
  createdAt: string
}>

export type FleetSummary = Readonly<{
  pylons: ReadonlyArray<{
    pylonRef: string
    displayName: string
    status: string
    resourceMode: string
    walletReady: boolean
    latestHeartbeatAt: string | null
    updatedAt: string
  }>
  onlineCount: number
  totalCount: number
}>

export type HealthSignal = Readonly<{
  status: string
  value?: string
  reasonRef?: string
}>

export type OverviewResult =
  | Readonly<{ tag: 'loaded'; snapshot: OverviewSnapshot }>
  | Readonly<{ tag: 'forbidden' }>
  | Readonly<{ tag: 'unauthorized' }>
  | Readonly<{ tag: 'failed'; status: number; error: string }>

export const fetchOperatorOverview = async (
  fetchFn: typeof fetch = fetch,
): Promise<OverviewResult> => {
  try {
    const response = await fetchFn(ADMIN_OPERATOR_OVERVIEW_URL, {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
    if (response.status === 403) return { tag: 'forbidden' }
    if (response.status === 401) return { tag: 'unauthorized' }
    const payload = (await response.json().catch(() => null)) as unknown
    if (!response.ok || !isRecord(payload)) {
      return {
        tag: 'failed',
        status: response.status,
        error: `Overview returned HTTP ${response.status}.`,
      }
    }
    return { tag: 'loaded', snapshot: payload as unknown as OverviewSnapshot }
  } catch (cause) {
    return {
      tag: 'failed',
      status: 0,
      error: cause instanceof Error ? cause.message : String(cause),
    }
  }
}

// Best-effort side reads composed into the dashboard. Each returns a
// discriminated result so a single endpoint being unreachable or non-admin
// never breaks the whole page.
export type SideResult =
  | Readonly<{ tag: 'ok'; value: unknown }>
  | Readonly<{ tag: 'error'; status: number }>

export const fetchSide = async (
  url: string,
  fetchFn: typeof fetch = fetch,
): Promise<SideResult> => {
  try {
    const response = await fetchFn(url, {
      cache: 'no-store',
      credentials: 'include',
      headers: { accept: 'application/json' },
    })
    if (!response.ok) return { tag: 'error', status: response.status }
    const value = (await response.json().catch(() => null)) as unknown
    return { tag: 'ok', value }
  } catch {
    return { tag: 'error', status: 0 }
  }
}

export const fetchOpsHealth = (fetchFn: typeof fetch = fetch) =>
  fetchSide('/api/admin/ops/health', fetchFn)
export const fetchFullAutoRuns = (fetchFn: typeof fetch = fetch) =>
  fetchSide('/api/full-auto-runs', fetchFn)
export const fetchFleetRuns = (fetchFn: typeof fetch = fetch) =>
  fetchSide('/api/fleet-runs', fetchFn)
