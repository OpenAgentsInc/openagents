// Pure mapping from the live network snapshot (GET /api/public/pylon-stats) to
// the home-screen scene model: the center pylon's activity glow and the network
// graph's nodes. Kept free of Three.js so it is unit-testable and can be
// exercised headlessly "as new pylons join the network".
//
// Visual language: docs/autopilot-coder/2026-06-15-autopilot-home-network-visual-language.md
// (§2 activity glow, §3 network graph, §6 empty states). Tune the knobs HERE,
// not in the renderer.

// Minimal shape of PublicPylonStats we consume (the worker owns the full
// schema in apps/openagents.com/workers/api/src/public-pylon-stats.ts). All
// fields optional/nullable so a partial or unavailable snapshot is safe.
export type PylonStatsSnapshot = {
  readonly available?: boolean
  readonly status?: "live" | "unavailable"
  readonly asOfLabel?: string | null
  readonly pylonsOnlineNow?: number
  readonly pylonsSeen24h?: number
  readonly pylonsRegisteredTotal?: number
  readonly pylonsWalletReadyNow?: number
  readonly pylonsAssignmentReadyNow?: number
  readonly pylonSessionsOnlineNow?: number
  readonly sellablePylonsOnlineNow?: number
  readonly trainingAssignedContributors?: number
  readonly trainingAcceptedContributors?: number
  readonly trainingModelProgressContributors?: number
  readonly nip90MarketSettlementStats?: {
    readonly compute?: NipStream
    readonly data?: NipStream
    readonly labor?: NipStream
  } | null
  readonly recentPylons?: ReadonlyArray<RecentPylon>
}

type NipStream = {
  readonly jobsSettled24h?: number
  readonly satsSettled24h?: number
  readonly satsSettledTotal?: number
}

export type RecentPylon = {
  readonly nodeLabel?: string | null
  readonly nostrPubkeyShort?: string
  readonly runtimeState?: string | null
  readonly onlineNow?: boolean | null
  readonly walletReadyNow?: boolean | null
  readonly assignmentReadyNow?: boolean | null
  readonly cumulativeSettledSats?: number | null
  readonly lastHeartbeatAgeSeconds?: number | null
  readonly products?: ReadonlyArray<string> | null
}

const cleanLabel = (value: string | null | undefined): string => {
  const label = typeof value === "string" ? value.trim() : ""
  return label.length > 0 ? label : ""
}

export const recentPylonNetworkId = (
  pylon: RecentPylon,
  index: number,
): string => {
  const ref = cleanLabel(pylon.nostrPubkeyShort)
  return ref.length > 0 && ref !== "unknown" ? ref : `recent-pylon-${index + 1}`
}

export const recentPylonNetworkLabel = (
  pylon: RecentPylon,
  index: number,
): string => {
  const ref = recentPylonNetworkId(pylon, index)
  const label = cleanLabel(pylon.nodeLabel)
  return label.length > 0 && label.toLowerCase() !== "pylon" ? label : ref
}

// A graph node's visual tone (§3). blue = working, white = online idle,
// grey = seen-but-offline.
export type PylonNodeTone = "working" | "online" | "offline"

export type PylonNetworkNode = {
  readonly id: string
  readonly label: string
  readonly tone: PylonNodeTone
  // edge animates toward the center when the node is doing work
  readonly flowing: boolean
  readonly growth?: PylonNetworkNodeGrowth
}

export type PylonNetworkNodeGrowth = {
  readonly tier: number
  readonly scale: number
  readonly facets: number
  readonly brightness: number
  readonly settledSats: number
}

export type PylonNetworkScene = {
  // network "alive but no work" vs "busy" — drives the center pylon glow [0,1]
  readonly activityIntensity: number
  readonly dormant: boolean
  readonly onlineNow: number
  readonly sessionsOnlineNow: number
  readonly sellableOnlineNow: number
  readonly walletReadyNow: number
  readonly assignmentReadyNow: number
  readonly seen24h: number
  readonly registeredTotal: number
  readonly satsSettled24h: number
  readonly satsSettledTotal: number
  readonly trainingAssignedContributors: number
  readonly trainingAcceptedContributors: number
  readonly trainingProgressContributors: number
  readonly nodes: ReadonlyArray<PylonNetworkNode>
  readonly asOfLabel: string | null
}

const n = (value: number | null | undefined): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0 ? value : 0

// Soft saturating curve: responds immediately to the first unit of work and
// asymptotes to 1 rather than clipping. k sets the "half-bright" point.
const saturate = (x: number, k: number): number => {
  const v = n(x)
  return v <= 0 ? 0 : 1 - 1 / (1 + v / k)
}

const clamp01 = (x: number): number => Math.max(0, Math.min(1, x))

// Activity-intensity weights + half-bright points (§2). Tune here.
const ACTIVITY = {
  sessions: { weight: 1 / 3, k: 3 },
  nip90: { weight: 1 / 3, k: 5 },
  training: { weight: 1 / 3, k: 4 },
} as const

const sumNip90Jobs24h = (snapshot: PylonStatsSnapshot): number => {
  const m = snapshot.nip90MarketSettlementStats
  if (!m) return 0
  return n(m.compute?.jobsSettled24h) + n(m.data?.jobsSettled24h) + n(m.labor?.jobsSettled24h)
}

const sumNip90Sats = (
  snapshot: PylonStatsSnapshot,
  field: "satsSettled24h" | "satsSettledTotal",
): number => {
  const m = snapshot.nip90MarketSettlementStats
  if (!m) return 0
  return n(m.compute?.[field]) + n(m.data?.[field]) + n(m.labor?.[field])
}

export function computeActivityIntensity(snapshot: PylonStatsSnapshot): number {
  const sessions = saturate(n(snapshot.pylonSessionsOnlineNow), ACTIVITY.sessions.k)
  const nip90 = saturate(sumNip90Jobs24h(snapshot), ACTIVITY.nip90.k)
  const training = saturate(n(snapshot.trainingModelProgressContributors), ACTIVITY.training.k)
  return clamp01(
    sessions * ACTIVITY.sessions.weight +
      nip90 * ACTIVITY.nip90.weight +
      training * ACTIVITY.training.weight,
  )
}

const recentNodeTone = (pylon: RecentPylon): PylonNodeTone => {
  if (pylon.assignmentReadyNow === true) return "working"
  if (pylon.onlineNow === true) return "online"
  return "offline"
}

// Build the network graph nodes from concrete recentPylons only. Aggregate
// counts stay on the hub; per-node labels must correspond to a network row.
export function buildNetworkNodes(
  snapshot: PylonStatsSnapshot,
): PylonNetworkNode[] {
  const recent = snapshot.recentPylons ?? []
  return recent.map((pylon, index) => {
    const tone = recentNodeTone(pylon)
    return {
      id: recentPylonNetworkId(pylon, index),
      label: recentPylonNetworkLabel(pylon, index),
      tone,
      flowing: tone === "working",
    }
  })
}

export function projectPylonNetworkScene(
  snapshot: PylonStatsSnapshot | null,
): PylonNetworkScene {
  if (
    snapshot === null ||
    snapshot.available === false ||
    snapshot.status === "unavailable"
  ) {
    return {
      activityIntensity: 0,
      dormant: true,
      onlineNow: 0,
      sessionsOnlineNow: 0,
      sellableOnlineNow: 0,
      walletReadyNow: 0,
      assignmentReadyNow: 0,
      seen24h: 0,
      registeredTotal: 0,
      satsSettled24h: 0,
      satsSettledTotal: 0,
      trainingAssignedContributors: 0,
      trainingAcceptedContributors: 0,
      trainingProgressContributors: 0,
      nodes: [],
      asOfLabel: snapshot?.asOfLabel ?? null,
    }
  }
  const onlineNow = n(snapshot.pylonsOnlineNow)
  return {
    activityIntensity: computeActivityIntensity(snapshot),
    dormant: onlineNow === 0,
    onlineNow,
    sessionsOnlineNow: n(snapshot.pylonSessionsOnlineNow),
    sellableOnlineNow: n(snapshot.sellablePylonsOnlineNow),
    walletReadyNow: n(snapshot.pylonsWalletReadyNow),
    assignmentReadyNow: n(snapshot.pylonsAssignmentReadyNow),
    seen24h: n(snapshot.pylonsSeen24h),
    registeredTotal: n(snapshot.pylonsRegisteredTotal),
    satsSettled24h: sumNip90Sats(snapshot, "satsSettled24h"),
    satsSettledTotal: sumNip90Sats(snapshot, "satsSettledTotal"),
    trainingAssignedContributors: n(snapshot.trainingAssignedContributors),
    trainingAcceptedContributors: n(snapshot.trainingAcceptedContributors),
    trainingProgressContributors: n(snapshot.trainingModelProgressContributors),
    nodes: buildNetworkNodes(snapshot),
    asOfLabel: snapshot.asOfLabel ?? null,
  }
}
