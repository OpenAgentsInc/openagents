// Live Pylon network stats for the `/pylons` route.
//
// Ported from `apps/web/src/scene/pylonNetworkStats.ts` — same public endpoint,
// same fail-soft behavior (any fetch/parse error -> null, treated as a dormant
// network), same activity-intensity mapping so the bezier network overlay and
// stat counters agree with the legacy Foldkit page's numbers. This app cannot
// import from `apps/web` (separate package), so the small amount of fetch/math
// logic is reproduced here rather than shared.

export type PylonStatsSnapshot = {
  readonly available?: boolean
  readonly status?: string
  readonly pylonsOnlineNow?: number
  readonly pylonsAssignmentReadyNow?: number
  readonly pylonSessionsOnlineNow?: number
  readonly publicRealSatsSettled24h?: number | null
  readonly trainingModelProgressContributors?: number
  readonly nip90MarketSettlementStats?: {
    readonly compute?: { readonly jobsSettled24h?: number; readonly satsSettled24h?: number }
    readonly data?: { readonly jobsSettled24h?: number; readonly satsSettled24h?: number }
    readonly labor?: { readonly jobsSettled24h?: number; readonly satsSettled24h?: number }
  } | null
}

export const PYLON_STATS_URL = '/api/public/pylon-stats'

const pos = (value: number | null | undefined): number =>
  typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0

// Soft saturating curve: immediate response to the first unit of work,
// asymptotes to 1. k = the half-bright point. Matches the Autopilot mapping.
const saturate = (value: number, k: number): number => {
  const v = pos(value)
  return v <= 0 ? 0 : 1 - 1 / (1 + v / k)
}

const clamp01 = (value: number): number => Math.max(0, Math.min(1, value))

const sumJobs24h = (snapshot: PylonStatsSnapshot): number => {
  const m = snapshot.nip90MarketSettlementStats
  if (!m) return 0
  return (
    pos(m.compute?.jobsSettled24h) + pos(m.data?.jobsSettled24h) + pos(m.labor?.jobsSettled24h)
  )
}

// activityIntensity = weighted sum of three saturated signals, each capped at
// its 1/3 weight (no single signal fills the bar). Idle network => 0.
export const computeActivityIntensity = (snapshot: PylonStatsSnapshot | null): number => {
  if (snapshot === null || snapshot.available === false || snapshot.status === 'unavailable') {
    return 0
  }
  const sessions = saturate(snapshot.pylonSessionsOnlineNow ?? 0, 3)
  const nip90 = saturate(sumJobs24h(snapshot), 5)
  const training = saturate(snapshot.trainingModelProgressContributors ?? 0, 4)
  return clamp01((sessions + nip90 + training) / 3)
}

const sumSats24h = (snapshot: PylonStatsSnapshot): number => {
  const publicReal = pos(snapshot.publicRealSatsSettled24h)
  if (publicReal > 0) return publicReal

  const m = snapshot.nip90MarketSettlementStats
  if (!m) return 0
  return (
    pos(m.compute?.satsSettled24h) + pos(m.data?.satsSettled24h) + pos(m.labor?.satsSettled24h)
  )
}

export type PylonStatKey = 'online' | 'working' | 'sats24h' | 'training'

export const PYLON_STATS: ReadonlyArray<{ key: PylonStatKey; label: string }> = [
  { key: 'online', label: 'pylons online' },
  { key: 'working', label: 'work-ready now' },
  { key: 'sats24h', label: 'sats settled · 24h' },
  { key: 'training', label: 'training contributors' },
]

const fmt = (n: number): string => n.toLocaleString('en-US')

// The exact placeholder shown before the first live poll resolves.
export const PYLON_STAT_LOADING = '…'

export const pylonStatValues = (
  snapshot: PylonStatsSnapshot | null,
): Record<PylonStatKey, string> => {
  if (snapshot === null) {
    return { online: '0', working: '0', sats24h: '0', training: '0' }
  }
  return {
    online: fmt(pos(snapshot.pylonsOnlineNow)),
    sats24h: fmt(sumSats24h(snapshot)),
    training: fmt(pos(snapshot.trainingModelProgressContributors)),
    working: fmt(pos(snapshot.pylonsAssignmentReadyNow)),
  }
}

// Fetch the live snapshot. Fail-soft -> null (dormant).
export const fetchPylonStats = async (
  fetchFn: typeof fetch = fetch,
  url: string = PYLON_STATS_URL,
): Promise<PylonStatsSnapshot | null> => {
  try {
    const response = await fetchFn(url, { headers: { accept: 'application/json' } })
    if (!response.ok) return null
    return (await response.json()) as PylonStatsSnapshot
  } catch {
    return null
  }
}

// --- Bezier network graph math (ported from pylonBezierNetworkElement.ts) ---

const MAX_NODES = 48
const CX = 50
const CY = 50

export type PylonBezierNode = Readonly<{ cx: number; cy: number; lit: boolean; opacity: number; radius: number }>
export type PylonBezierEdge = Readonly<{ d: string; lit: boolean }>
export type PylonBezierGraph = Readonly<{
  edges: ReadonlyArray<PylonBezierEdge>
  nodes: ReadonlyArray<PylonBezierNode>
}>

// Deterministic ring layout (golden-angle spiral) so nodes read as a network
// and the layout is stable between polls.
const nodeXY = (index: number, total: number): { x: number; y: number } => {
  const golden = 2.399963229728653
  const ringR = 24 + Math.min(16, total * 0.25)
  const r = ringR * (0.55 + 0.45 * Math.sqrt((index + 1) / Math.max(1, total)))
  const a = index * golden
  return { x: CX + Math.cos(a) * r, y: CY + Math.sin(a) * r * 0.7 }
}

// A bezier (quadratic) curve from a node to the center, bowed perpendicular to
// the chord so edges arc rather than run straight.
const edgePath = (x: number, y: number): string => {
  const mx = (x + CX) / 2
  const my = (y + CY) / 2
  const dx = CX - x
  const dy = CY - y
  const len = Math.hypot(dx, dy) || 1
  const bow = Math.min(10, len * 0.25)
  const cx = mx + (-dy / len) * bow
  const cy = my + (dx / len) * bow
  return `M ${x.toFixed(2)} ${y.toFixed(2)} Q ${cx.toFixed(2)} ${cy.toFixed(2)} ${CX} ${CY}`
}

export const pylonBezierGraph = (
  snapshot: PylonStatsSnapshot | null,
): PylonBezierGraph => {
  const online = pos(snapshot?.pylonsOnlineNow)
  const assignmentReady = pos(snapshot?.pylonsAssignmentReadyNow)
  const intensity = computeActivityIntensity(snapshot)

  const count = Math.min(MAX_NODES, online)
  const litCount = count > 0 ? Math.round((assignmentReady / online) * count) : 0

  const edges: Array<PylonBezierEdge> = []
  const nodes: Array<PylonBezierNode> = []
  for (let i = 0; i < count; i += 1) {
    const { x, y } = nodeXY(i, count)
    const lit = i < litCount
    edges.push({ d: edgePath(x, y), lit })
    nodes.push({
      cx: x,
      cy: y,
      lit,
      opacity: lit ? 0.55 + intensity * 0.45 : 0.4,
      radius: lit ? 0.85 : 0.6,
    })
  }
  return { edges, nodes }
}
