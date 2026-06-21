// Live Tassadar run scene element (#5118, epic #5112).
//
// A self-fetching custom element that, on connect, fetches the public-safe run
// summary from `GET /api/public/tassadar-run-summary` (no auth, #5114), maps it
// through the merged snapshot adapter (`tassadarRunVisualizationOptions`, #5113),
// and mounts the `oa-training-run` WebGL element with the resulting options —
// the real run, breathing.
//
// RECEIPT-FIRST: this element NEVER fabricates metrics. A just-launched / idle
// run (the endpoint's `emptyState.idle`) flows through the adapter as honest
// zeros and renders the empty/planned scene. A non-200 response or network
// failure renders a graceful, honest error message — never faked numbers.
//
// State machine: loading → ok (mount scene) | empty (idle honest scene) | error.
// The data-state attribute is exposed for tests and styling. Dark-only.
import type {
  TrainingRunNodeSelection,
  TrainingRunWorldItemSelection,
  TrainingRunVisualizationOptions,
  WasdMouseLookControllerOptions,
  WasdMouseLookDebugSnapshot,
} from '@openagentsinc/three-effect/core'
import {
  registerTrainingRunElement,
  trainingRunTagName,
} from '@openagentsinc/three-effect/foldkit'
import { define as defineCustomElement } from 'foldkit/customElement'
import type { Attribute, Html } from 'foldkit/html'

import {
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
  TASSADAR_PROOF_REPLAY_TAG,
} from './tassadarProofReplayElement'
import {
  type PublicTassadarSettlementRow,
  type TassadarRunBulletin,
  type TassadarRunPublicSummary,
  tassadarRunVisualizationOptions,
} from './tassadarRunSnapshot'
import {
  TASSADAR_ATTENTION_THROTTLE_MS,
  TASSADAR_AVATAR_POSITION_THROTTLE_MS,
  TASSADAR_REGION_BOUNDS,
  type TassadarLocalAvatarPosition,
  type TassadarPylonAttentionUpdate,
  type TassadarSpacetimeWorldSubscription,
  spacetimeConfigFromElement,
  startTassadarSpacetimeWorldSubscription,
} from './tassadarSpacetimeWorld'

export const TASSADAR_RUN_TAG = 'oa-tassadar-run'
export const TASSADAR_RUN_SUMMARY_ENDPOINT = '/api/public/tassadar-run-summary'
const TASSADAR_LOCAL_VIEWER_SESSION_KEY = 'openagents.tassadar.viewerName'
const TASSADAR_INITIAL_AVATAR_POSITION = {
  movementMode: 'idle',
  pitch: 0,
  positionX: 0,
  positionY: 0,
  positionZ: 5.6,
  yaw: 0,
} satisfies TassadarLocalAvatarPosition
const TASSADAR_WALK_METERS_PER_SECOND = 3.2
const TASSADAR_RUN_METERS_PER_SECOND = 5.6
const TASSADAR_AVATAR_KEEPALIVE_MS = 5_000
const TASSADAR_CHAT_MAX_CHARS = 280
const TASSADAR_LOCAL_CHAT_RADIUS_METERS = 8

export type TassadarRunDataState = 'loading' | 'ok' | 'empty' | 'error'
type TassadarRegionBounds = Readonly<{
  maxX: number
  maxY: number
  maxZ: number
  minX: number
  minY: number
  minZ: number
}>
export type TassadarRunProofLink = Readonly<{
  caveats: ReadonlyArray<string>
  href: string
  kind: string
  label: string
  ref: string
  sourceRefs: ReadonlyArray<string>
  state: string
}>

const HOST_STYLE =
  ':host{position:absolute;inset:0;display:block;background:#000;color:#f1efe8}' +
  '.mount{position:absolute;inset:0}' +
  '.overlay{position:absolute;inset:0;display:flex;align-items:center;justify-content:center;' +
  'padding:2rem;text-align:center;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;pointer-events:none}' +
  '.overlay p{margin:0;max-width:48ch;border:1px solid rgba(255,255,255,0.1);background:rgba(0,0,0,0.34);padding:0.8rem 1rem;font-size:0.95rem;line-height:1.6;color:rgba(241,239,232,0.64);backdrop-filter:blur(12px)}' +
  '.overlay .label{display:block;margin-bottom:0.4rem;font-size:0.7rem;letter-spacing:0.08em;' +
  'text-transform:uppercase;color:rgba(241,239,232,0.35)}' +
  '.status{position:absolute;top:0.9rem;left:1rem;right:1rem;z-index:3;display:flex;align-items:flex-start;' +
  'justify-content:space-between;gap:0.75rem;padding:0;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:#f1efe8;pointer-events:none}' +
  '.status dl{display:grid;grid-template-columns:repeat(5,minmax(0,auto));gap:0.55rem 1rem;margin:0;min-width:0}' +
  '.status div{min-width:0;text-shadow:0 1px 8px rgba(0,0,0,0.85)}.status dt{margin:0 0 0.18rem;font-size:0.58rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.34)}' +
  '.status dd{margin:0;max-width:min(18rem,24vw);overflow-wrap:anywhere;white-space:normal;font-size:0.72rem;line-height:1.25;color:rgba(255,255,255,0.76)}' +
  '.status .legend{display:flex;flex-wrap:wrap;justify-content:flex-end;gap:0.35rem 0.6rem;max-width:46rem;margin:0;padding:0;list-style:none;text-shadow:0 1px 8px rgba(0,0,0,0.85)}' +
  '.status .legend li{display:inline-flex;gap:0.32rem;align-items:baseline;min-width:0;font-size:0.62rem;line-height:1.25;color:rgba(255,255,255,0.58)}' +
  '.status .legend strong{font-weight:600;letter-spacing:0.06em;text-transform:uppercase;color:rgba(255,255,255,0.34)}.status .legend span{color:rgba(255,255,255,0.72)}' +
  '.status a{pointer-events:auto;align-self:flex-start;white-space:nowrap;color:rgba(255,255,255,0.86);font-size:0.72rem;text-underline-offset:0.2rem;text-shadow:0 1px 8px rgba(0,0,0,0.9)}.status a:hover{color:#fff}' +
  '@media (max-width:720px){.status{display:grid}.status dl{grid-template-columns:repeat(2,minmax(0,1fr))}.status dd{max-width:none}.status .legend{justify-content:flex-start;max-width:none}}' +
  '.selection{position:absolute;right:1rem;bottom:1rem;z-index:2;max-width:min(26rem,calc(100% - 2rem));' +
  'border:1px solid rgba(255,255,255,0.12);background:rgba(0,0,0,0.42);padding:0.75rem 0.875rem;' +
  'font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:#f1efe8;backdrop-filter:blur(14px);box-shadow:0 0.75rem 2rem rgba(0,0,0,0.28)}' +
  '.selection strong{display:block;margin-bottom:0.25rem;font-size:0.78rem;font-weight:600;color:rgba(255,255,255,0.88)}' +
  '.selection p{margin:0;font-size:0.72rem;line-height:1.45;color:rgba(255,255,255,0.55)}' +
  '.selection dl{display:grid;gap:0.45rem;margin:0.55rem 0 0}.selection dt{margin:0 0 0.12rem;font-size:0.58rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.35)}' +
  '.selection dd{margin:0;overflow-wrap:anywhere;font-size:0.7rem;line-height:1.4;color:rgba(255,255,255,0.66)}' +
  '.selection a{display:inline-flex;margin-top:0.55rem;font-size:0.72rem;color:rgba(255,255,255,0.86);text-underline-offset:0.18rem}' +
  '.selection a:hover{color:#fff}' +
  '.bulletin{position:absolute;left:1rem;top:5.8rem;z-index:2;width:min(28rem,calc(100% - 2rem));max-height:min(32rem,calc(100% - 7rem));overflow:auto;' +
  'border:1px solid rgba(142,246,255,0.22);background:rgba(5,8,8,0.78);padding:0.9rem 1rem;' +
  'font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:#f1efe8;backdrop-filter:blur(16px);box-shadow:0 0.9rem 2.4rem rgba(0,0,0,0.34);pointer-events:none}' +
  '.bulletin strong{display:block;margin-bottom:0.35rem;font-size:0.82rem;font-weight:700;color:rgba(255,255,255,0.92)}' +
  '.bulletin p{margin:0;font-size:0.72rem;line-height:1.55;color:rgba(255,255,255,0.68)}' +
  '.bulletin .headline{margin-bottom:0.55rem;color:rgba(142,246,255,0.86)}' +
  '.bulletin dl{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:0.55rem;margin:0.75rem 0}.bulletin dt{margin:0 0 0.12rem;font-size:0.56rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.36)}' +
  '.bulletin dd{margin:0;font-size:0.76rem;color:rgba(255,255,255,0.78)}.bulletin ol{display:grid;gap:0.45rem;margin:0.7rem 0 0;padding:0;list-style:none}' +
  '.bulletin li{border-left:2px solid rgba(142,246,255,0.35);padding-left:0.5rem}.bulletin li span{display:block;margin-bottom:0.12rem;font-size:0.58rem;letter-spacing:0.08em;text-transform:uppercase;color:rgba(255,255,255,0.38)}' +
  '.bulletin li p{color:rgba(255,255,255,0.62)}' +
  '.chat{position:absolute;left:1rem;bottom:1rem;z-index:2;width:min(24rem,calc(100% - 2rem));' +
  'font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:#f1efe8;pointer-events:auto}' +
  '.chat ol{display:grid;gap:0.35rem;max-height:9rem;overflow:hidden;margin:0 0 0.5rem;padding:0;list-style:none}' +
  '.chat li{border-left:2px solid rgba(255,255,255,0.22);padding-left:0.5rem;font-size:0.68rem;line-height:1.35;color:rgba(255,255,255,0.62);text-shadow:0 1px 8px rgba(0,0,0,0.8)}' +
  '.chat strong{display:inline;color:rgba(255,255,255,0.82);font-weight:600}.chat span{overflow-wrap:anywhere}' +
  '.chat form{display:grid;grid-template-columns:minmax(0,1fr) auto;gap:0.4rem;border:1px solid rgba(255,255,255,0.12);background:rgba(0,0,0,0.42);padding:0.45rem;backdrop-filter:blur(14px)}' +
  '.chat input{min-width:0;border:0;background:transparent;color:#fff;font:inherit;font-size:0.72rem;outline:none}' +
  '.chat input::placeholder{color:rgba(255,255,255,0.35)}.chat button{border:1px solid rgba(255,255,255,0.16);background:rgba(255,255,255,0.08);color:#fff;font:inherit;font-size:0.68rem;padding:0.28rem 0.5rem}' +
  '.chat button:disabled,.chat input:disabled{opacity:0.45}'

const isIdle = (summary: TassadarRunPublicSummary): boolean =>
  summary.emptyState?.idle === true

// Pure: classify a fetched summary into the data-state the scene should show.
// Idle → 'empty' (still renders the honest planned/zeroed scene). Otherwise 'ok'.
export const dataStateForSummary = (
  summary: TassadarRunPublicSummary,
): Exclude<TassadarRunDataState, 'loading' | 'error'> =>
  isIdle(summary) ? 'empty' : 'ok'

const publicTrainingRunHref = (summary: TassadarRunPublicSummary): string => {
  const runRef = summary.runRef ?? 'run.tassadar.executor.20260615'
  return `/api/public/training/runs/${encodeURIComponent(runRef)}`
}

const focusedTrainingRunHref = (
  summary: TassadarRunPublicSummary,
  ref: string,
): string =>
  `${publicTrainingRunHref(summary)}?focusRef=${encodeURIComponent(ref)}`

const isNexusPylonReceiptRef = (ref: string): boolean =>
  ref.startsWith('receipt.nexus.') ||
  ref.startsWith('receipt.nexus_') ||
  ref.startsWith('receipt.nexus-pylon.')

const nexusPylonReceiptHref = (ref: string): string =>
  `/api/public/nexus-pylon/receipts/${encodeURIComponent(ref)}`

const receiptHref = (ref: string): string =>
  isNexusPylonReceiptRef(ref)
    ? nexusPylonReceiptHref(ref)
    : `/api/forum/receipts/${encodeURIComponent(ref)}`

const firstRef = (
  refs: ReadonlyArray<string> | undefined,
): string | undefined =>
  Array.isArray(refs)
    ? refs.map(ref => ref.trim()).find(ref => ref.length > 0)
    : undefined

const publicRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  Array.isArray(refs)
    ? refs.map(ref => ref.trim()).filter(ref => ref.length > 0)
    : []

const metricNumber = (
  metric: { readonly value?: number } | undefined,
): number =>
  metric !== undefined &&
  typeof metric.value === 'number' &&
  Number.isFinite(metric.value)
    ? metric.value
    : 0

const finiteNonNegative = (value: number | undefined): number =>
  typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : 0

const textOrUnknown = (value: string | undefined): string => {
  const text = value?.trim()
  return text === undefined || text.length === 0 ? 'unknown' : text
}

const displayNumber = (value: number | undefined): string =>
  new Intl.NumberFormat('en-US').format(
    typeof value === 'number' && Number.isFinite(value) ? value : 0,
  )

export const sanitizeTassadarChatBody = (value: string): string | null => {
  const body = value.trim().replace(/\s+/g, ' ')
  if (body.length === 0) return null
  return body.length <= TASSADAR_CHAT_MAX_CHARS
    ? body
    : body.slice(0, TASSADAR_CHAT_MAX_CHARS)
}

const localViewerDisplayName = (): string => {
  const fallback = 'viewer'
  if (typeof sessionStorage === 'undefined') return fallback
  try {
    const existing = sessionStorage
      .getItem(TASSADAR_LOCAL_VIEWER_SESSION_KEY)
      ?.trim()
    if (existing !== undefined && existing.length > 0) return existing
    sessionStorage.setItem(TASSADAR_LOCAL_VIEWER_SESSION_KEY, fallback)
    return fallback
  } catch {
    return fallback
  }
}

type TassadarMovementKey = 'backward' | 'forward' | 'left' | 'right' | 'sprint'

type TassadarMovementKeyState = Record<TassadarMovementKey, boolean>

type TassadarAvatarMovementSync = {
  activeAttentionPylonRef: string | null
  intervalId: number
  lastAttentionAt: number
  lastFrameAt: number
  lastSentAt: number
  lastSentSignature: string
  subscription: TassadarSpacetimeWorldSubscription
}

const emptyMovementKeys = (): TassadarMovementKeyState => ({
  backward: false,
  forward: false,
  left: false,
  right: false,
  sprint: false,
})

const movementKeyForCode = (code: string): TassadarMovementKey | undefined => {
  switch (code) {
    case 'ArrowUp':
    case 'KeyW':
      return 'forward'
    case 'ArrowDown':
    case 'KeyS':
      return 'backward'
    case 'ArrowLeft':
    case 'KeyA':
      return 'left'
    case 'ArrowRight':
    case 'KeyD':
      return 'right'
    case 'ShiftLeft':
    case 'ShiftRight':
      return 'sprint'
    default:
      return undefined
  }
}

const movementModeForKeys = (
  keys: TassadarMovementKeyState,
): TassadarLocalAvatarPosition['movementMode'] =>
  keys.forward || keys.backward || keys.left || keys.right
    ? keys.sprint
      ? 'running'
      : 'walking'
    : 'idle'

const avatarPositionSignature = (
  position: TassadarLocalAvatarPosition,
): string =>
  [
    position.positionX.toFixed(3),
    position.positionY.toFixed(3),
    position.positionZ.toFixed(3),
    position.yaw.toFixed(3),
    position.pitch.toFixed(3),
    position.movementMode,
  ].join(':')

const pylonRefFromProofLink = (
  summary: TassadarRunPublicSummary,
  proofLink: TassadarRunProofLink | null,
): string | undefined => {
  if (proofLink === null) return undefined
  const pylonRefs = new Set(
    (summary.realGradient?.leaderboardRows ?? [])
      .map(row => row.pylonRef?.trim())
      .filter((ref): ref is string => ref !== undefined && ref.length > 0),
  )
  return proofLink.sourceRefs.find(ref => pylonRefs.has(ref))
}

const clamp = (value: number, min: number, max: number): number =>
  Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : min

const regionBoundsFromSummary = (
  summary: TassadarRunPublicSummary,
): TassadarRegionBounds => {
  const bounds = summary.world?.worldRegions?.[0]?.bounds
  if (
    bounds === undefined ||
    !Number.isFinite(bounds.minX) ||
    !Number.isFinite(bounds.maxX) ||
    !Number.isFinite(bounds.minY) ||
    !Number.isFinite(bounds.maxY) ||
    !Number.isFinite(bounds.minZ) ||
    !Number.isFinite(bounds.maxZ) ||
    bounds.minX >= bounds.maxX ||
    bounds.minY >= bounds.maxY ||
    bounds.minZ >= bounds.maxZ
  ) {
    return TASSADAR_REGION_BOUNDS
  }
  return bounds
}

export const tassadarSupportHudItems = (
  summary: TassadarRunPublicSummary,
): ReadonlyArray<readonly [string, string]> => {
  const metrics = summary.metrics ?? {}
  const gradient = summary.realGradient ?? {}
  const observedDevices = finiteNonNegative(
    gradient.deviceRequirement?.observedDistinctContributorDevices,
  )
  const requiredDevices = finiteNonNegative(
    gradient.deviceRequirement?.requiredDistinctContributorDevices,
  )
  const registered = Math.max(
    metricNumber(metrics.assignedContributorCount),
    observedDevices,
    summary.world?.pylonStations?.length ?? 0,
  )
  const qualified = metricNumber(metrics.qualifiedContributorCount)
  const activeWindows = metricNumber(metrics.activeWindowCount)
  const blockerCount = publicRefs(gradient.externalAsk?.blockerRefs).length
  const receiptCount = Math.max(
    metricNumber(metrics.receiptRefCount),
    settlementRows(summary).filter(
      row => settlementReceiptRef(row) !== undefined,
    ).length,
  )
  const stationCount = summary.world?.pylonStations?.length ?? 0
  const avatarCount = summary.world?.avatarPositions?.length ?? 0
  const staleBound =
    typeof summary.staleness?.maxStalenessSeconds === 'number' &&
    Number.isFinite(summary.staleness.maxStalenessSeconds)
      ? `<= ${summary.staleness.maxStalenessSeconds}s`
      : 'unknown'

  return [
    ['registered', `${registered} pylons`],
    [
      'qualified',
      requiredDevices > 0
        ? `${observedDevices}/${requiredDevices} device gate`
        : `${qualified} qualified`,
    ],
    ['state synced', staleBound],
    ['active', `${activeWindows} windows`],
    ['sync reentry', `${blockerCount} blockers`],
    ['world rows', `${stationCount} stations / ${avatarCount} avatars`],
    ['proof refs', `${receiptCount} receipts`],
  ]
}

export const nextTassadarLocalAvatarPosition = (
  current: TassadarLocalAvatarPosition,
  keys: TassadarMovementKeyState,
  deltaMs: number,
  bounds: TassadarRegionBounds = TASSADAR_REGION_BOUNDS,
): TassadarLocalAvatarPosition => {
  const forward = (keys.forward ? 1 : 0) - (keys.backward ? 1 : 0)
  const strafe = (keys.right ? 1 : 0) - (keys.left ? 1 : 0)
  const moving = forward !== 0 || strafe !== 0
  const length = moving ? Math.hypot(forward, strafe) : 1
  const speed = keys.sprint
    ? TASSADAR_RUN_METERS_PER_SECOND
    : TASSADAR_WALK_METERS_PER_SECOND
  const distance = (Math.max(0, Math.min(deltaMs, 500)) / 1_000) * speed
  const yaw = Number.isFinite(current.yaw) ? current.yaw : 0
  const forwardX = Math.sin(yaw)
  const forwardZ = -Math.cos(yaw)
  const rightX = Math.cos(yaw)
  const rightZ = Math.sin(yaw)
  return {
    ...current,
    movementMode: movementModeForKeys(keys),
    positionX: clamp(
      current.positionX +
        ((forward / length) * forwardX + (strafe / length) * rightX) * distance,
      bounds.minX,
      bounds.maxX,
    ),
    positionY: clamp(current.positionY, bounds.minY, bounds.maxY),
    positionZ: clamp(
      current.positionZ +
        ((forward / length) * forwardZ + (strafe / length) * rightZ) * distance,
      bounds.minZ,
      bounds.maxZ,
    ),
  }
}

const generatedAtText = (summary: TassadarRunPublicSummary): string =>
  textOrUnknown(summary.generatedAt)

const settlementReceiptRef = (
  row: PublicTassadarSettlementRow,
): string | undefined => {
  const ref = row.receiptRef?.trim()
  return ref === undefined || ref === '' ? undefined : ref
}

const settlementRows = (
  summary: TassadarRunPublicSummary,
): ReadonlyArray<PublicTassadarSettlementRow> =>
  Array.isArray(summary.settlementRows) ? summary.settlementRows : []

const firstSettlementRow = (
  summary: TassadarRunPublicSummary,
): PublicTassadarSettlementRow | undefined =>
  settlementRows(summary).find(row => settlementReceiptRef(row) !== undefined)

const settlementRowForRef = (
  summary: TassadarRunPublicSummary,
  ref: string,
): PublicTassadarSettlementRow | undefined =>
  settlementRows(summary).find(row => settlementReceiptRef(row) === ref)

const proofDetail = (
  input: Readonly<{
    caveats?: ReadonlyArray<string>
    href: string
    kind: string
    label: string
    ref: string
    sourceRefs?: ReadonlyArray<string>
    state?: string
  }>,
): TassadarRunProofLink => ({
  caveats: publicRefs(input.caveats),
  href: input.href,
  kind: input.kind,
  label: input.label,
  ref: input.ref,
  sourceRefs: publicRefs(input.sourceRefs),
  state: input.state ?? 'linked',
})

const settlementProofDetail = (
  row: PublicTassadarSettlementRow,
): TassadarRunProofLink | null => {
  const ref = settlementReceiptRef(row)
  if (ref === undefined) return null
  const movementMode = row.movementMode ?? 'unknown'
  const state = row.state ?? 'unknown'
  const realBitcoinMoved = row.realBitcoinMoved === true
  const amount =
    typeof row.amountSats === 'number' && Number.isFinite(row.amountSats)
      ? `${row.amountSats} sats`
      : 'amount unknown'

  return proofDetail({
    caveats: [
      `Amount: ${amount}`,
      realBitcoinMoved
        ? 'Receipt claims real Bitcoin movement.'
        : 'Simulation-backed settlement record; this does not prove real Bitcoin moved.',
    ],
    href: row.apiUrl ?? nexusPylonReceiptHref(ref),
    kind: row.receiptKind ?? 'settlement_recorded',
    label: 'Settlement receipt',
    ref,
    sourceRefs: row.sourceRefs ?? [],
    state: `${state}; ${movementMode}; real bitcoin moved: ${
      realBitcoinMoved ? 'yes' : 'no'
    }`,
  })
}

const linkForRef = (
  summary: TassadarRunPublicSummary,
  label: string,
  ref: string | undefined,
): TassadarRunProofLink | null =>
  ref === undefined
    ? null
    : ref.startsWith('receipt.')
      ? settlementRowForRef(summary, ref) === undefined
        ? proofDetail({
            href: receiptHref(ref),
            kind: isNexusPylonReceiptRef(ref)
              ? 'nexus_pylon_receipt'
              : 'forum_receipt',
            label,
            ref,
            state: 'linked',
          })
        : settlementProofDetail(settlementRowForRef(summary, ref)!)
      : proofDetail({
          href: focusedTrainingRunHref(summary, ref),
          kind: 'training_ref',
          label,
          ref,
          state: 'linked',
        })

const replayPairForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
) =>
  summary.realGradient?.verifiedReplayPairs?.find(
    pair =>
      pair.workerRef === selection.id || pair.validatorRef === selection.id,
  )

const rejectedReplayPairForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
) =>
  summary.realGradient?.rejectedReplayPairs?.find(
    pair =>
      pair.workerRef === selection.id || pair.validatorRef === selection.id,
  )

const leaderboardRowForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
) =>
  summary.realGradient?.leaderboardRows?.find(
    row => row.pylonRef === selection.id,
  )

const worldPylonRefForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
): string | undefined => {
  const station = summary.world?.pylonStations?.find(
    row => selection.id === `station.${row.pylonRef}`,
  )
  if (station !== undefined) return station.pylonRef
  return summary.world?.agentAvatars?.find(
    row => row.avatarRef === selection.id,
  )?.homePylonRef
}

const pylonRefForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
): string | undefined =>
  leaderboardRowForSelection(summary, selection)?.pylonRef ??
  worldPylonRefForSelection(summary, selection)

export const pylonAttentionForAvatar = (
  summary: TassadarRunPublicSummary,
  position: TassadarLocalAvatarPosition,
  selectedPylonRef: string | null,
): TassadarPylonAttentionUpdate | null => {
  const stations = summary.world?.pylonStations ?? []
  if (stations.length === 0) return null
  const selectedStation =
    selectedPylonRef === null
      ? undefined
      : stations.find(station => station.pylonRef === selectedPylonRef)
  const nearest = stations
    .map(station => {
      const dx = station.position.x - position.positionX
      const dz = station.position.z - position.positionZ
      return { distanceMeters: Math.hypot(dx, dz), dx, dz, station }
    })
    .sort((left, right) => left.distanceMeters - right.distanceMeters)[0]
  const candidate =
    selectedStation === undefined
      ? nearest
      : {
          distanceMeters: Math.hypot(
            selectedStation.position.x - position.positionX,
            selectedStation.position.z - position.positionZ,
          ),
          dx: selectedStation.position.x - position.positionX,
          dz: selectedStation.position.z - position.positionZ,
          station: selectedStation,
        }
  if (candidate === undefined) return null
  const radius = Math.max(0.1, candidate.station.interactionRadiusMeters)
  const withinInteraction = candidate.distanceMeters <= radius
  const selected = selectedPylonRef === candidate.station.pylonRef
  if (!withinInteraction && !selected) return null
  const forwardX = Math.sin(position.yaw)
  const forwardZ = -Math.cos(position.yaw)
  const distance = Math.max(candidate.distanceMeters, 0.0001)
  const looking =
    withinInteraction &&
    (candidate.dx / distance) * forwardX +
      (candidate.dz / distance) * forwardZ >
      0.82
  return {
    attentionKind: selected ? 'inspecting' : looking ? 'looking' : 'nearby',
    distanceMeters: Number(candidate.distanceMeters.toFixed(3)),
    pylonRef: candidate.station.pylonRef,
    ...(selected ? { sourceEntityRef: candidate.station.pylonRef } : {}),
  }
}

const corpusTraceForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
): string | undefined =>
  publicRefs(summary.corpus?.traceRefs).find(ref => ref === selection.id)

export const proofLinkForSelection = (
  summary: TassadarRunPublicSummary,
  selection: TrainingRunNodeSelection,
): TassadarRunProofLink | null => {
  const settlement = settlementRowForRef(summary, selection.id)
  if (settlement !== undefined) {
    return settlementProofDetail(settlement)
  }

  const pair = replayPairForSelection(summary, selection)
  if (pair !== undefined) {
    return linkForRef(summary, 'Verified replay challenge', pair.challengeRef)
  }

  const rejectedPair = rejectedReplayPairForSelection(summary, selection)
  if (rejectedPair !== undefined) {
    return linkForRef(
      summary,
      'Rejected replay challenge',
      rejectedPair.challengeRef,
    )
  }

  const row = leaderboardRowForSelection(summary, selection)
  if (row !== undefined) {
    const contributorSettlement = settlementRows(summary).find(
      settlementRow => settlementRow.contributorRef === row.pylonRef,
    )
    if (contributorSettlement !== undefined) {
      return settlementProofDetail(contributorSettlement)
    }
    return linkForRef(summary, 'Pylon evidence', firstRef(row.sourceRefs))
  }

  const worldPylonRef = worldPylonRefForSelection(summary, selection)
  if (worldPylonRef !== undefined) {
    const row = summary.realGradient?.leaderboardRows?.find(
      row => row.pylonRef === worldPylonRef,
    )
    const contributorSettlement = settlementRows(summary).find(
      settlementRow => settlementRow.contributorRef === worldPylonRef,
    )
    if (contributorSettlement !== undefined) {
      return settlementProofDetail(contributorSettlement)
    }
    return linkForRef(
      summary,
      'Pylon evidence',
      firstRef(row?.sourceRefs) ?? worldPylonRef,
    )
  }

  const corpusTraceRef = corpusTraceForSelection(summary, selection)
  if (corpusTraceRef !== undefined) {
    return linkForRef(summary, 'Accepted trace corpus ref', corpusTraceRef)
  }

  if (selection.id === 'run') {
    return proofDetail({
      href: publicTrainingRunHref(summary),
      kind: 'training_run',
      label: 'Public run projection',
      ref: summary.runRef ?? 'run.tassadar.executor.20260615',
      state: summary.runState ?? 'unknown',
    })
  }

  if (selection.id === 'training_window' || selection.id === 'active') {
    return linkForRef(
      summary,
      'Training window',
      summary.windows?.[0]?.windowRef,
    )
  }

  if (selection.id === 'freivalds' || selection.role === 'proof') {
    return linkForRef(
      summary,
      'Verification proof',
      firstRef(summary.corpus?.verdictRefs) ??
        firstRef(summary.corpus?.traceRefs) ??
        firstRef(
          summary.realGradient?.closeoutRequirement?.freivaldsCommitmentRefs,
        ),
    )
  }

  if (selection.id === 'receipt' || selection.role === 'receipt') {
    const settlement = firstSettlementRow(summary)
    if (settlement !== undefined) {
      return settlementProofDetail(settlement)
    }
    return linkForRef(summary, 'Receipt', firstRef(summary.receiptRefs))
  }

  if (selection.id === 'settlement' || selection.role === 'rung') {
    const settlement = firstSettlementRow(summary)
    if (settlement !== undefined) {
      return settlementProofDetail(settlement)
    }
    return metricNumber(summary.metrics?.providerConfirmedSettledPayoutSats) > 0
      ? linkForRef(summary, 'Settlement receipt', firstRef(summary.receiptRefs))
      : null
  }

  return null
}

const isTrainingRunNodeSelection = (
  value: unknown,
): value is TrainingRunNodeSelection => {
  if (typeof value !== 'object' || value === null) {
    return false
  }
  const record = value as Record<string, unknown>
  return (
    typeof record.detail === 'string' &&
    typeof record.id === 'string' &&
    typeof record.label === 'string' &&
    typeof record.role === 'string' &&
    typeof record.status === 'string'
  )
}

const isTrainingRunWorldItemSelection = (
  value: unknown,
): value is TrainingRunWorldItemSelection => {
  if (typeof value !== 'object' || value === null) {
    return false
  }
  const record = value as Record<string, unknown>
  return (
    typeof record.detail === 'string' &&
    typeof record.id === 'string' &&
    record.kind === 'bulletin_board' &&
    typeof record.label === 'string' &&
    typeof record.status === 'string'
  )
}

const isWorldItemProximityDetail = (
  value: unknown,
): value is { readonly item: TrainingRunWorldItemSelection | null } => {
  if (typeof value !== 'object' || value === null) return false
  const item = (value as { readonly item?: unknown }).item
  return item === null || isTrainingRunWorldItemSelection(item)
}

const makeClass = (): CustomElementConstructor =>
  class extends HTMLElement {
    #shadow: ShadowRoot | null = null
    #abort: AbortController | null = null
    #spacetimeWorld: TassadarSpacetimeWorldSubscription | null = null
    #avatarMovement: TassadarAvatarMovementSync | null = null
    #movementKeys = emptyMovementKeys()
    #localAvatarPosition: TassadarLocalAvatarPosition = {
      ...TASSADAR_INITIAL_AVATAR_POSITION,
    }
    #selectedPylonRef: string | null = null
    #nearWorldItemId: string | null = null
    #summary: TassadarRunPublicSummary | null = null
    #mouselookDebugCount = 0

    connectedCallback(): void {
      const shadow = this.shadowRoot ?? this.attachShadow({ mode: 'open' })
      this.#shadow = shadow
      this.#refresh()
    }

    disconnectedCallback(): void {
      this.#abort?.abort()
      this.#abort = null
      this.#disconnectSpacetimeWorld()
      this.#stopAvatarMovement()
      this.#shadow?.replaceChildren()
    }

    #refresh(): void {
      this.#abort?.abort()
      this.#disconnectSpacetimeWorld()
      this.#stopAvatarMovement()
      this.#renderLoading()
      this.#abort = new AbortController()
      void this.#load(this.#abort.signal)
    }

    async #load(signal: AbortSignal): Promise<void> {
      try {
        const response = await fetch(TASSADAR_RUN_SUMMARY_ENDPOINT, {
          headers: { accept: 'application/json' },
          signal,
        })
        if (signal.aborted) return
        if (!response.ok) {
          this.#renderError(
            `Run summary unavailable (HTTP ${response.status}). The live ` +
              'projection is not reachable right now.',
          )
          return
        }
        const summary = (await response.json()) as TassadarRunPublicSummary
        if (signal.aborted) return
        this.#renderScene(summary)
      } catch (error) {
        if (signal.aborted) return
        this.#renderError(
          'Could not load the live run summary. The projection endpoint did ' +
            'not respond.',
        )
        // Keep the failure visible to operators without faking any metric.
        void error
      }
    }

    #base(): { shadow: ShadowRoot; mount: HTMLDivElement } | null {
      const shadow = this.#shadow
      if (shadow === null) return null
      shadow.replaceChildren()
      const style = document.createElement('style')
      style.textContent = HOST_STYLE
      const mount = document.createElement('div')
      mount.className = 'mount'
      shadow.append(style, mount)
      return { shadow, mount }
    }

    #renderLoading(): void {
      const base = this.#base()
      if (base === null) return
      this.setAttribute('data-state', 'loading')
      base.mount.append(
        this.#overlay('Live data', 'Loading the live Tassadar run projection…'),
      )
    }

    #renderError(message: string): void {
      const base = this.#base()
      if (base === null) return
      this.setAttribute('data-state', 'error')
      this.removeAttribute('data-spacetime-state')
      base.mount.append(this.#overlay('Live data — error', message))
    }

    // Receipt-first: idle summaries still render the real (zeroed) scene; we do
    // not substitute placeholder numbers. Only the data-state differs so callers
    // and tests can distinguish a just-launched run from a populated one.
    #renderScene(summary: TassadarRunPublicSummary): void {
      const base = this.#base()
      if (base === null) return
      this.#summary = summary
      this.setAttribute('data-state', dataStateForSummary(summary))
      this.setAttribute('data-pointer-lock', 'released')
      registerTrainingRunElement()
      const run = document.createElement(trainingRunTagName) as HTMLElement & {
        visualization?: unknown
      }
      run.style.position = 'absolute'
      run.style.inset = '0'
      // The training-run element reads its `visualization` property reactively.
      const visualization: TrainingRunVisualizationOptions = {
        ...tassadarRunVisualizationOptions(summary),
        cameraMode: 'perspective_walk',
        controller: 'wasd_mouselook',
        walkController: this.#walkControllerOptions(),
      }
      run.visualization = visualization
      run.addEventListener('node-selected', event => {
        const detail = (event as CustomEvent<unknown>).detail
        if (!isTrainingRunNodeSelection(detail)) return
        const proofLink = proofLinkForSelection(
          this.#summary ?? summary,
          detail,
        )
        const activeSummary = this.#summary ?? summary
        this.#selectedPylonRef =
          pylonRefForSelection(activeSummary, detail) ??
          pylonRefFromProofLink(activeSummary, proofLink) ??
          null
        this.#renderSelection(base.mount, detail, proofLink)
        this.#renderChat(base.mount, activeSummary)
      })
      run.addEventListener('world-item-proximity-changed', event => {
        const detail = (event as CustomEvent<unknown>).detail
        if (!isWorldItemProximityDetail(detail)) return
        this.#nearWorldItemId = detail.item?.id ?? null
        this.#renderBulletin(base.mount, this.#summary ?? summary, detail.item)
      })
      base.mount.append(run)
      this.#renderStatus(base.mount, summary)
      this.#renderChat(base.mount, summary)
      this.#connectSpacetimeWorld(summary, run, base.mount)
    }

    #disconnectSpacetimeWorld(): void {
      this.#stopAvatarMovement()
      this.#spacetimeWorld?.disconnect()
      this.#spacetimeWorld = null
      this.removeAttribute('data-spacetime-state')
    }

    #connectSpacetimeWorld(
      summary: TassadarRunPublicSummary,
      run: HTMLElement & { visualization?: unknown },
      mount: HTMLDivElement,
    ): void {
      const config = spacetimeConfigFromElement(this)
      if (config === null) return
      const signal = this.#abort?.signal
      this.setAttribute('data-spacetime-state', 'connecting')
      void startTassadarSpacetimeWorldSubscription({
        baseSummary: summary,
        config,
        displayName: localViewerDisplayName(),
        onError: () => {
          if (!this.isConnected || signal?.aborted === true) return
          this.setAttribute('data-spacetime-state', 'error')
        },
        onSummary: nextSummary => {
          if (!this.isConnected || signal?.aborted === true) return
          this.#summary = nextSummary
          this.setAttribute('data-spacetime-state', 'connected')
          run.visualization = {
            ...tassadarRunVisualizationOptions(nextSummary),
            cameraMode: 'perspective_walk',
            controller: 'wasd_mouselook',
            walkController: this.#walkControllerOptions(),
          }
          this.#renderStatus(mount, nextSummary)
          this.#renderBulletinForCurrentProximity(mount, nextSummary)
          this.#renderChat(mount, nextSummary)
        },
      })
        .then(subscription => {
          if (!this.isConnected || signal?.aborted === true) {
            subscription.disconnect()
            return
          }
          this.#spacetimeWorld = subscription
          this.#startAvatarMovement(subscription)
          this.#renderChat(mount, this.#summary ?? summary)
        })
        .catch(() => {
          if (!this.isConnected || signal?.aborted === true) return
          this.setAttribute('data-spacetime-state', 'error')
        })
    }

    #startAvatarMovement(
      subscription: TassadarSpacetimeWorldSubscription,
    ): void {
      this.#stopAvatarMovement()
      this.#movementKeys = emptyMovementKeys()
      this.#localAvatarPosition = { ...TASSADAR_INITIAL_AVATAR_POSITION }
      const now = 0
      this.#avatarMovement = {
        activeAttentionPylonRef: null,
        intervalId: window.setInterval(() => {
          const current = this.#avatarMovement
          this.#tickAvatarMovement(
            current === null ? 0 : current.lastFrameAt + 100,
          )
        }, 100),
        lastAttentionAt: 0,
        lastFrameAt: now,
        lastSentAt: 0,
        lastSentSignature: '',
        subscription,
      }
      window.addEventListener('keydown', this.#handleMovementKeyDown, {
        passive: true,
      })
      window.addEventListener('keyup', this.#handleMovementKeyUp, {
        passive: true,
      })
      this.#syncLocalAvatar(now, true)
      this.#syncPylonAttention(now, true)
    }

    #stopAvatarMovement(): void {
      const movement = this.#avatarMovement
      if (movement === null) return
      window.clearInterval(movement.intervalId)
      window.removeEventListener('keydown', this.#handleMovementKeyDown)
      window.removeEventListener('keyup', this.#handleMovementKeyUp)
      if (movement.activeAttentionPylonRef !== null) {
        movement.subscription.clearPylonFocus(movement.activeAttentionPylonRef)
      }
      this.#avatarMovement = null
      this.#movementKeys = emptyMovementKeys()
    }

    #handleMovementKeyDown = (event: KeyboardEvent): void => {
      const key = movementKeyForCode(event.code)
      if (key === undefined) return
      this.#movementKeys[key] = true
    }

    #handleMovementKeyUp = (event: KeyboardEvent): void => {
      const key = movementKeyForCode(event.code)
      if (key === undefined) return
      this.#movementKeys[key] = false
    }

    #tickAvatarMovement(now: number): void {
      const movement = this.#avatarMovement
      if (movement === null) return
      const deltaMs = Math.max(0, now - movement.lastFrameAt)
      movement.lastFrameAt = now
      this.#localAvatarPosition = nextTassadarLocalAvatarPosition(
        this.#localAvatarPosition,
        this.#movementKeys,
        deltaMs,
        regionBoundsFromSummary(this.#summary ?? {}),
      )
      this.#syncLocalAvatar(now, false)
      this.#syncPylonAttention(now, false)
    }

    #syncLocalAvatar(now: number, force: boolean): void {
      const movement = this.#avatarMovement
      if (movement === null) return
      const signature = avatarPositionSignature(this.#localAvatarPosition)
      const changed = signature !== movement.lastSentSignature
      const throttled =
        !force &&
        now < movement.lastSentAt + TASSADAR_AVATAR_POSITION_THROTTLE_MS
      const keepaliveDue =
        now >= movement.lastSentAt + TASSADAR_AVATAR_KEEPALIVE_MS
      if (throttled || (!changed && !keepaliveDue && !force)) return
      movement.subscription.updateLocalAvatar(this.#localAvatarPosition)
      movement.lastSentAt = now
      movement.lastSentSignature = signature
      this.setAttribute(
        'data-avatar-sync',
        this.#localAvatarPosition.movementMode,
      )
    }

    #syncPylonAttention(now: number, force: boolean): void {
      const movement = this.#avatarMovement
      if (movement === null) return
      if (
        !force &&
        now < movement.lastAttentionAt + TASSADAR_ATTENTION_THROTTLE_MS
      ) {
        return
      }
      const nextAttention = pylonAttentionForAvatar(
        this.#summary ?? {},
        this.#localAvatarPosition,
        this.#selectedPylonRef,
      )
      if (nextAttention === null && movement.activeAttentionPylonRef !== null) {
        movement.subscription.clearPylonFocus(movement.activeAttentionPylonRef)
        movement.activeAttentionPylonRef = null
        movement.lastAttentionAt = now
        this.removeAttribute('data-pylon-attention')
        return
      }
      if (nextAttention === null) return
      if (
        movement.activeAttentionPylonRef !== null &&
        movement.activeAttentionPylonRef !== nextAttention.pylonRef
      ) {
        movement.subscription.clearPylonFocus(movement.activeAttentionPylonRef)
      }
      movement.subscription.focusPylon(nextAttention)
      movement.activeAttentionPylonRef = nextAttention.pylonRef
      movement.lastAttentionAt = now
      this.setAttribute(
        'data-pylon-attention',
        `${nextAttention.pylonRef}:${nextAttention.attentionKind}`,
      )
    }

    #walkControllerOptions(): WasdMouseLookControllerOptions {
      const bounds = regionBoundsFromSummary(this.#summary ?? {})
      return {
        bounds: {
          minX: bounds.minX,
          maxX: bounds.maxX,
          minZ: bounds.minZ,
          maxZ: bounds.maxZ,
        },
        eyeHeight: 1.65,
        initialPosition: [
          TASSADAR_INITIAL_AVATAR_POSITION.positionX,
          1.65,
          TASSADAR_INITIAL_AVATAR_POSITION.positionZ,
        ],
        movementSpeed: 4.5,
        sprintMultiplier: 1.8,
        debug: this.#recordMouselookDebug,
        onLockChange: locked => {
          this.setAttribute('data-pointer-lock', locked ? 'locked' : 'released')
        },
      }
    }

    #recordMouselookDebug = (snapshot: WasdMouseLookDebugSnapshot): void => {
      this.#mouselookDebugCount += 1
      this.setAttribute(
        'data-mouselook-count',
        String(this.#mouselookDebugCount),
      )
      this.setAttribute('data-mouselook-event', snapshot.event)
      this.setAttribute('data-mouselook-source', snapshot.source)
      this.setAttribute('data-mouselook-locked', String(snapshot.locked))
      this.setAttribute('data-mouselook-applied', String(snapshot.applied))
      this.#localAvatarPosition = {
        ...this.#localAvatarPosition,
        pitch: snapshot.pitch,
        yaw: snapshot.yaw,
      }
      this.setAttribute(
        'data-mouselook-delta',
        `${snapshot.movementX},${snapshot.movementY}`,
      )
      this.setAttribute('data-mouselook-yaw', snapshot.yaw.toFixed(5))
      this.setAttribute('data-mouselook-pitch', snapshot.pitch.toFixed(5))
      if (snapshot.reason === undefined) {
        this.removeAttribute('data-mouselook-reason')
      } else {
        this.setAttribute('data-mouselook-reason', snapshot.reason)
      }
      if (snapshot.movementX !== 0 || snapshot.movementY !== 0) {
        this.setAttribute(
          'data-mouselook-last-nonzero',
          `${snapshot.movementX},${snapshot.movementY}`,
        )
      }
      if (typeof console === 'undefined') return
      const motionEvent =
        snapshot.event === 'mousemove' ||
        snapshot.event === 'pointermove' ||
        snapshot.event === 'pointerrawupdate'
      if (
        !motionEvent ||
        this.#mouselookDebugCount <= 60 ||
        this.#mouselookDebugCount % 60 === 0
      ) {
        console.warn('[tassadar:mouselook]', snapshot)
      }
    }

    #renderChat(
      mount: HTMLDivElement,
      summary: TassadarRunPublicSummary,
    ): void {
      const existingBody =
        (mount.querySelector('.chat input') as HTMLInputElement | null)
          ?.value ?? ''
      mount.querySelector('.chat')?.remove()
      const messages = (summary.world?.localChatMessages ?? [])
        .filter(message => message.moderationState === 'visible')
        .slice(-5)
      if (this.#spacetimeWorld === null && messages.length === 0) return
      const panel = document.createElement('aside')
      panel.className = 'chat'
      panel.setAttribute('aria-label', 'Nearby chat')

      if (messages.length > 0) {
        const names = new Map(
          (summary.world?.agentAvatars ?? []).map(avatar => [
            avatar.avatarRef,
            avatar.displayName,
          ]),
        )
        const list = document.createElement('ol')
        messages.forEach(message => {
          const item = document.createElement('li')
          const speaker = document.createElement('strong')
          speaker.textContent =
            names.get(message.speakerAvatarRef) ??
            message.speakerAvatarRef.split('.').slice(-1)[0] ??
            'avatar'
          const body = document.createElement('span')
          body.textContent = ` ${message.body}`
          item.append(speaker, body)
          list.append(item)
        })
        panel.append(list)
      }

      const form = document.createElement('form')
      const input = document.createElement('input')
      input.name = 'body'
      input.maxLength = TASSADAR_CHAT_MAX_CHARS
      input.value = existingBody
      input.placeholder =
        this.#selectedPylonRef === null ? 'Nearby message' : 'Message pylon'
      input.autocomplete = 'off'
      input.disabled = this.#spacetimeWorld === null
      const button = document.createElement('button')
      button.type = 'submit'
      button.textContent = 'Send'
      button.disabled = this.#spacetimeWorld === null
      form.addEventListener('submit', event => {
        event.preventDefault()
        this.#sendChatMessage(input)
      })
      form.append(input, button)
      panel.append(form)
      mount.append(panel)
    }

    #sendChatMessage(input: HTMLInputElement): void {
      const subscription = this.#spacetimeWorld
      if (subscription === null) {
        this.setAttribute('data-chat-state', 'unavailable')
        return
      }
      const body = sanitizeTassadarChatBody(input.value)
      if (body === null) {
        this.setAttribute('data-chat-state', 'empty')
        return
      }
      if (this.#selectedPylonRef === null) {
        subscription.sendLocalMessage({
          body,
          radiusMeters: TASSADAR_LOCAL_CHAT_RADIUS_METERS,
        })
      } else {
        subscription.sendPylonMessage({
          body,
          pylonRef: this.#selectedPylonRef,
        })
      }
      input.value = ''
      this.setAttribute(
        'data-chat-state',
        this.#selectedPylonRef === null ? 'local_sent' : 'pylon_sent',
      )
    }

    #renderStatus(
      mount: HTMLDivElement,
      summary: TassadarRunPublicSummary,
    ): void {
      mount.querySelector('.status')?.remove()
      const panel = document.createElement('aside')
      panel.className = 'status'
      panel.setAttribute('aria-label', 'Live Tassadar snapshot status')
      const list = document.createElement('dl')
      const rows: ReadonlyArray<readonly [string, string]> = [
        ['Run', textOrUnknown(summary.runRef)],
        ['State', textOrUnknown(summary.runState)],
        ['Updated', generatedAtText(summary)],
      ]
      for (const [label, value] of rows) {
        const item = document.createElement('div')
        const term = document.createElement('dt')
        term.textContent = label
        const detail = document.createElement('dd')
        detail.textContent = value
        item.append(term, detail)
        list.append(item)
      }
      const legend = document.createElement('ul')
      legend.className = 'legend'
      for (const [label, value] of tassadarSupportHudItems(summary)) {
        const item = document.createElement('li')
        const term = document.createElement('strong')
        term.textContent = label
        const detail = document.createElement('span')
        detail.textContent = value
        item.append(term, detail)
        legend.append(item)
      }
      const replayLink = document.createElement('a')
      replayLink.href = `/tassadar/replay/${FIRST_REAL_SETTLEMENT_REPLAY_SLUG}`
      replayLink.setAttribute(
        'data-tassadar-replay-link',
        'first-real-settlement',
      )
      replayLink.textContent = 'Replay first settlement'
      replayLink.setAttribute(
        'aria-label',
        'Replay first real Tassadar settlement',
      )
      replayLink.setAttribute('data-replay-element', TASSADAR_PROOF_REPLAY_TAG)
      panel.append(list, legend, replayLink)
      mount.append(panel)
    }

    #renderBulletinForCurrentProximity(
      mount: HTMLDivElement,
      summary: TassadarRunPublicSummary,
    ): void {
      if (this.#nearWorldItemId === null) {
        this.#renderBulletin(mount, summary, null)
        return
      }
      this.#renderBulletin(mount, summary, {
        detail: summary.bulletin?.summary ?? '',
        id: this.#nearWorldItemId,
        kind: 'bulletin_board',
        label: summary.bulletin?.title ?? 'Tassadar board',
        status: 'active',
        sourceRefs: summary.bulletin?.sourceRefs ?? [],
      })
    }

    #renderBulletin(
      mount: HTMLDivElement,
      summary: TassadarRunPublicSummary,
      item: TrainingRunWorldItemSelection | null,
    ): void {
      mount.querySelector('.bulletin')?.remove()
      if (item === null || item.id !== 'bulletin.tassadar.run') return
      const bulletin = summary.bulletin
      if (bulletin === undefined) return
      const panel = document.createElement('aside')
      panel.className = 'bulletin'
      panel.setAttribute('aria-label', 'Tassadar run bulletin')
      panel.setAttribute('data-world-item', item.id)

      const title = document.createElement('strong')
      title.textContent = textOrUnknown(bulletin.title)
      const headline = document.createElement('p')
      headline.className = 'headline'
      headline.textContent = textOrUnknown(bulletin.headline)
      const body = document.createElement('p')
      body.textContent = textOrUnknown(bulletin.summary)
      panel.append(title, headline, body)

      this.#appendBulletinMetrics(panel, bulletin)
      this.#appendBulletinActivity(panel, bulletin)
      mount.append(panel)
    }

    #appendBulletinMetrics(
      panel: HTMLElement,
      bulletin: TassadarRunBulletin,
    ): void {
      const metrics = bulletin.metrics
      if (metrics === undefined) return
      const rows: ReadonlyArray<readonly [string, string]> = [
        ['pylons', displayNumber(metrics.totalPylonCount)],
        ['active', displayNumber(metrics.activePylonCount)],
        ['sats', displayNumber(metrics.settledSats)],
        ['windows', displayNumber(metrics.activeWindowCount)],
        ['traces', displayNumber(metrics.acceptedTraceCount)],
        ['verified', displayNumber(metrics.verifiedWorkCount)],
      ]
      const list = document.createElement('dl')
      for (const [label, value] of rows) {
        const item = document.createElement('div')
        const term = document.createElement('dt')
        term.textContent = label
        const detail = document.createElement('dd')
        detail.textContent = value
        item.append(term, detail)
        list.append(item)
      }
      panel.append(list)
    }

    #appendBulletinActivity(
      panel: HTMLElement,
      bulletin: TassadarRunBulletin,
    ): void {
      const activity = bulletin.latestActivity ?? []
      if (activity.length === 0) return
      const list = document.createElement('ol')
      for (const event of activity.slice(0, 3)) {
        const item = document.createElement('li')
        const label = document.createElement('span')
        label.textContent = textOrUnknown(event.label)
        const text = document.createElement('p')
        text.textContent = textOrUnknown(event.text)
        item.append(label, text)
        list.append(item)
      }
      panel.append(list)
    }

    #renderSelection(
      mount: HTMLDivElement,
      selection: TrainingRunNodeSelection,
      proofLink: TassadarRunProofLink | null,
    ): void {
      mount.querySelector('.selection')?.remove()
      const panel = document.createElement('aside')
      panel.className = 'selection'
      panel.setAttribute(
        'data-proof-state',
        proofLink === null ? 'unlinked' : 'linked',
      )
      const title = document.createElement('strong')
      title.textContent = selection.label
      const detail = document.createElement('p')
      detail.textContent =
        proofLink === null
          ? `${selection.detail}. No public proof ref is linked yet.`
          : `${proofLink.label}`
      panel.append(title, detail)
      if (proofLink !== null) {
        const list = document.createElement('dl')
        const rows: ReadonlyArray<readonly [string, string]> = [
          ['Kind', proofLink.kind],
          ['State', proofLink.state],
          ['Ref', proofLink.ref],
          ['Route', proofLink.href],
          [
            'Caveats',
            proofLink.caveats.length === 0
              ? 'none'
              : proofLink.caveats.join(' | '),
          ],
          [
            'Source refs',
            proofLink.sourceRefs.length === 0
              ? 'none'
              : proofLink.sourceRefs.slice(0, 6).join(' | '),
          ],
        ]
        for (const [term, value] of rows) {
          const item = document.createElement('div')
          const dt = document.createElement('dt')
          dt.textContent = term
          const dd = document.createElement('dd')
          dd.textContent = value
          item.append(dt, dd)
          list.append(item)
        }
        const link = document.createElement('a')
        link.href = proofLink.href
        link.target = '_blank'
        link.rel = 'noopener noreferrer'
        link.textContent = 'Open proof'
        panel.append(list, link)
      }
      mount.append(panel)
    }

    #overlay(label: string, message: string): HTMLDivElement {
      const overlay = document.createElement('div')
      overlay.className = 'overlay'
      const text = document.createElement('p')
      const labelEl = document.createElement('span')
      labelEl.className = 'label'
      labelEl.textContent = label
      text.append(labelEl, document.createTextNode(message))
      overlay.append(text)
      return overlay
    }
  }

const register = (): void => {
  if (typeof customElements === 'undefined') return
  if (typeof HTMLElement === 'undefined') return
  if (customElements.get(TASSADAR_RUN_TAG) !== undefined) return
  customElements.define(TASSADAR_RUN_TAG, makeClass())
}

const element = defineCustomElement({
  events: {},
  properties: {},
  tag: TASSADAR_RUN_TAG,
})

// Foldkit view helper: renders the self-fetching `<oa-tassadar-run>` element.
export const tassadarRunView = <Message>(
  attributes: ReadonlyArray<Attribute<Message>> = [],
): Html => {
  register()
  return element.withMessage<Message>()(attributes, [])
}
