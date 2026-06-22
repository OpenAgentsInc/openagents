import { Data, Effect } from "effect"

import {
  WORLD_DELTA_SCHEMA_VERSION,
  decodeWorldDelta,
  decodeWorldSubscriptionPlan,
  sparseWorldPatchChangesOnly,
  worldRowKey,
  type WorldDelta,
  type WorldReadModel,
  type WorldRow,
  type WorldSubscriptionPlan,
  type WorldVector3,
} from "@openagentsinc/world-contract"

import {
  cursorForSequence,
  normalizeRegionRef,
  stableWorldRef,
} from "./protocol"

export const REGION_FEED_SPLIT_AVATAR_THRESHOLD = 96
export const REGION_FEED_SPLIT_ROWS_PER_SECOND_THRESHOLD = 960
export const DEFAULT_REGION_ENTER_RADIUS = 90
export const DEFAULT_REGION_DROP_RADIUS = 120
export const DEFAULT_REGION_NEAR_RADIUS = 32
export const DEFAULT_REGION_FAR_RADIUS = 120
export const DEFAULT_NEAR_UPDATE_MS = 100
export const DEFAULT_FAR_UPDATE_MS = 1_000
export const MAX_PUBLIC_INTEREST_RADIUS = 256

export class WorldSubscriptionPolicyError extends Data.TaggedError("WorldSubscriptionPolicyError")<{
  readonly reason: string
  readonly sourceRef: string
}> {}

export type WorldSubscriptionScope = WorldSubscriptionPlan["scope"]

export type WorldSubscriptionPlanRequest = Readonly<{
  regionRef: string
  scope?: string | null
  runRef?: string | null
  selectedEntityRef?: string | null
  selectedRefs?: ReadonlyArray<string>
  center?: Readonly<{
    x?: number
    y?: number
    z?: number
  }> | null
  resumeCursor?: string | null
  requestedRows?: ReadonlyArray<string>
}>

export type RegionFeedPolicy = Readonly<{
  kind: "single_region" | "split_near_far"
  avatarCount: number
  estimatedRowsPerSecond: number
  nearUpdateMs: number
  farUpdateMs: number
}>

export type SubscriptionInterestEntity = Readonly<{
  ref: string
  row: WorldRow
  position: WorldVector3
  stopped?: boolean
}>

export type SubscriptionInterestState = Readonly<{
  seenRefs: ReadonlyArray<string>
  tierByRef: Readonly<Record<string, "near" | "far">>
}>

export type SubscriptionInterestDelta = Readonly<{
  feedPolicy: RegionFeedPolicy
  fullRows: ReadonlyArray<WorldRow>
  liteRows: ReadonlyArray<WorldRow>
  prunedRefs: ReadonlyArray<string>
  settleRefs: ReadonlyArray<string>
  nearRefs: ReadonlyArray<string>
  farRefs: ReadonlyArray<string>
  updateMsByRef: Readonly<Record<string, number>>
  nextState: SubscriptionInterestState
}>

export const emptySubscriptionInterestState: SubscriptionInterestState = {
  seenRefs: [],
  tierByRef: {},
}

export const approveSubscriptionPlan = (
  request: WorldSubscriptionPlanRequest,
): Effect.Effect<WorldSubscriptionPlan, WorldSubscriptionPolicyError> =>
  Effect.sync(() => {
    rejectUnboundedAvatarOrEventRows(request)
    const scope = normalizeScope(request.scope, request.selectedEntityRef)
    const regionRef = normalizeRegionRef(request.regionRef)
    const center = normalizeCenter(request.center)
    const selectedRefs = [
      ...(request.selectedRefs ?? []),
      ...(request.selectedEntityRef === null || request.selectedEntityRef === undefined ? [] : [request.selectedEntityRef]),
    ].filter((ref, index, refs) => ref.length > 0 && refs.indexOf(ref) === index)

    return decodeWorldSubscriptionPlan({
      planRef: stableWorldRef("subscription.world", JSON.stringify({
        regionRef,
        scope,
        runRef: request.runRef ?? null,
        selectedEntityRef: request.selectedEntityRef ?? null,
        center,
        selectedRefs,
      })),
      regionRef,
      scope,
      ...(request.runRef === null || request.runRef === undefined || scope === "global" ? {} : { runRef: request.runRef }),
      ...(request.selectedEntityRef === null || request.selectedEntityRef === undefined
        ? {}
        : { selectedEntityRef: request.selectedEntityRef }),
      interest: {
        center,
        enterRadius: DEFAULT_REGION_ENTER_RADIUS,
        dropRadius: DEFAULT_REGION_DROP_RADIUS,
        nearRadius: DEFAULT_REGION_NEAR_RADIUS,
        farRadius: DEFAULT_REGION_FAR_RADIUS,
        selectedRefs,
      },
      nearUpdateMs: DEFAULT_NEAR_UPDATE_MS,
      farUpdateMs: DEFAULT_FAR_UPDATE_MS,
      ...(request.resumeCursor === null || request.resumeCursor === undefined || request.resumeCursor.length === 0
        ? {}
        : { resumeCursor: request.resumeCursor }),
    })
  })

export const subscriptionRequestFromUrl = (
  url: URL,
  regionRef: string,
): WorldSubscriptionPlanRequest => {
  const x = numberParam(url, "x")
  const y = numberParam(url, "y")
  const z = numberParam(url, "z")
  return {
    regionRef,
    scope: url.searchParams.get("scope"),
    runRef: url.searchParams.get("runRef"),
    selectedEntityRef: url.searchParams.get("selectedEntityRef"),
    selectedRefs: url.searchParams.getAll("selectedRef"),
    center: {
      ...(x === undefined ? {} : { x }),
      ...(y === undefined ? {} : { y }),
      ...(z === undefined ? {} : { z }),
    },
    resumeCursor: url.searchParams.get("cursor"),
    requestedRows: [
      ...url.searchParams.getAll("row"),
      ...url.searchParams.getAll("table"),
    ],
  }
}

export const planRegionFeedPolicy = (input: {
  readonly avatarCount: number
  readonly estimatedRowsPerSecond?: number
  readonly nearUpdateMs?: number
  readonly farUpdateMs?: number
}): RegionFeedPolicy => {
  const avatarCount = Math.max(0, Math.floor(input.avatarCount))
  const nearUpdateMs = input.nearUpdateMs ?? DEFAULT_NEAR_UPDATE_MS
  const farUpdateMs = input.farUpdateMs ?? DEFAULT_FAR_UPDATE_MS
  const estimatedRowsPerSecond = input.estimatedRowsPerSecond ?? avatarCount * (1_000 / nearUpdateMs)
  const shouldSplit =
    avatarCount > REGION_FEED_SPLIT_AVATAR_THRESHOLD ||
    estimatedRowsPerSecond > REGION_FEED_SPLIT_ROWS_PER_SECOND_THRESHOLD

  return {
    kind: shouldSplit ? "split_near_far" : "single_region",
    avatarCount,
    estimatedRowsPerSecond,
    nearUpdateMs,
    farUpdateMs,
  }
}

export const planSubscriptionInterestDelta = (input: {
  readonly plan: WorldSubscriptionPlan
  readonly previous: SubscriptionInterestState
  readonly entities: ReadonlyArray<SubscriptionInterestEntity>
}): SubscriptionInterestDelta => {
  const previousSeen = new Set(input.previous.seenRefs)
  const selectedRefs = new Set(input.plan.interest.selectedRefs.map(String))
  const avatarCount = input.entities.filter(entity => entity.row.kind === "agent_avatar" || entity.row.kind === "avatar_position").length
  const feedPolicy = planRegionFeedPolicy({
    avatarCount,
    nearUpdateMs: input.plan.nearUpdateMs,
    farUpdateMs: input.plan.farUpdateMs,
  })

  const active = input.entities
    .map(entity => {
      const wasSeen = previousSeen.has(entity.ref)
      const isSelected = selectedRefs.has(entity.ref)
      const distance = distance3(input.plan.interest.center, entity.position)
      const activeRadius = wasSeen ? input.plan.interest.dropRadius : input.plan.interest.enterRadius
      const inInterest = isSelected || distance <= activeRadius
      if (!inInterest) {
        return null
      }
      const tier = isSelected || distance <= input.plan.interest.nearRadius ? "near" as const : "far" as const
      return { entity, tier, wasSeen }
    })
    .filter((entry): entry is NonNullable<typeof entry> => entry !== null)

  const activeRefs = active.map(entry => entry.entity.ref)
  const fullRows = active.filter(entry => !entry.wasSeen).map(entry => entry.entity.row)
  const liteRows = active.filter(entry => entry.wasSeen).map(entry => entry.entity.row)
  const prunedRefs = input.previous.seenRefs.filter(ref => !activeRefs.includes(ref))
  const settleRefs = active
    .filter(entry => entry.entity.stopped === true)
    .map(entry => entry.entity.ref)
  const tierByRef = Object.fromEntries(active.map(entry => [entry.entity.ref, entry.tier]))
  const updateMsByRef = Object.fromEntries(active.map(entry => [
    entry.entity.ref,
    entry.tier === "near" ? input.plan.nearUpdateMs : input.plan.farUpdateMs,
  ]))

  return {
    feedPolicy,
    fullRows,
    liteRows,
    prunedRefs,
    settleRefs,
    nearRefs: active.filter(entry => entry.tier === "near").map(entry => entry.entity.ref),
    farRefs: active.filter(entry => entry.tier === "far").map(entry => entry.entity.ref),
    updateMsByRef,
    nextState: {
      seenRefs: activeRefs,
      tierByRef,
    },
  }
}

export const entitiesFromRows = (
  rows: ReadonlyArray<WorldRow>,
): ReadonlyArray<SubscriptionInterestEntity> =>
  rows.flatMap(row => {
    if (row.kind !== "avatar_position") {
      return []
    }
    return [{
      ref: worldRowKey(row),
      row,
      position: row.position,
      stopped: row.animation === "idle",
    }]
  })

export const deltaFromSubscriptionInterest = (input: {
  readonly regionRef: string
  readonly cursor: string
  readonly generatedAt: string
  readonly deltaRef: string
  readonly interest: SubscriptionInterestDelta
}): WorldDelta =>
  decodeWorldDelta({
    schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
    deltaRef: input.deltaRef,
    kind: input.interest.prunedRefs.length > 0 ? "delete" : "update",
    regionRef: input.regionRef,
    cursor: input.cursor,
    generatedAt: input.generatedAt,
    rows: [...input.interest.fullRows, ...input.interest.liteRows],
    ...(input.interest.prunedRefs.length === 0 ? {} : { deletedRefs: input.interest.prunedRefs }),
    patches: input.interest.settleRefs.map(ref => ({
      ref,
      movement: "settled",
    })),
  })

export const applySubscriptionDeltaToReadModel = (
  readModel: WorldReadModel,
  delta: WorldDelta,
): WorldReadModel => {
  const next = {
    ...readModel,
    cursor: delta.cursor,
    generatedAt: delta.generatedAt,
    regions: { ...readModel.regions },
    pylons: { ...readModel.pylons },
    gateways: { ...readModel.gateways },
    avatars: { ...readModel.avatars },
    positions: { ...readModel.positions },
    chatMessages: { ...readModel.chatMessages },
    chatBubbles: { ...readModel.chatBubbles },
    emotes: { ...readModel.emotes },
    intents: { ...readModel.intents },
    runs: { ...readModel.runs },
    entities: { ...readModel.entities },
    edges: { ...readModel.edges },
    proofRefs: { ...readModel.proofRefs },
    settlementRefs: { ...readModel.settlementRefs },
    events: { ...readModel.events },
    diagnostics: [...readModel.diagnostics],
  } as {
    schemaVersion: WorldReadModel["schemaVersion"]
    regionRef: WorldReadModel["regionRef"]
    cursor: WorldReadModel["cursor"]
    generatedAt: WorldReadModel["generatedAt"]
    regions: Record<string, WorldReadModel["regions"][string]>
    pylons: Record<string, WorldReadModel["pylons"][string]>
    gateways: Record<string, WorldReadModel["gateways"][string]>
    avatars: Record<string, WorldReadModel["avatars"][string]>
    positions: Record<string, WorldReadModel["positions"][string]>
    chatMessages: Record<string, WorldReadModel["chatMessages"][string]>
    chatBubbles: Record<string, WorldReadModel["chatBubbles"][string]>
    emotes: Record<string, WorldReadModel["emotes"][string]>
    intents: Record<string, WorldReadModel["intents"][string]>
    runs: Record<string, WorldReadModel["runs"][string]>
    entities: Record<string, WorldReadModel["entities"][string]>
    edges: Record<string, WorldReadModel["edges"][string]>
    proofRefs: Record<string, WorldReadModel["proofRefs"][string]>
    settlementRefs: Record<string, WorldReadModel["settlementRefs"][string]>
    events: Record<string, WorldReadModel["events"][string]>
    diagnostics: Array<WorldReadModel["diagnostics"][number]>
  }

  for (const row of delta.rows ?? []) {
    const key = worldRowKey(row)
    switch (row.kind) {
      case "world_region":
        next.regions[key] = row
        break
      case "pylon_station":
        next.pylons[key] = row
        break
      case "gateway_station":
        next.gateways[key] = row
        break
      case "agent_avatar":
        next.avatars[key] = row
        break
      case "avatar_position":
        next.positions[key] = row
        break
      case "local_chat_message":
        next.chatMessages[key] = row
        break
      case "chat_bubble":
        next.chatBubbles[key] = row
        break
      case "local_emote":
        next.emotes[key] = row
        break
      case "agent_intent":
        next.intents[key] = row
        break
      case "training_run":
        next.runs[key] = row
        break
      case "run_entity":
        next.entities[key] = row
        break
      case "world_edge":
        next.edges[key] = row
        break
      case "proof_ref":
        next.proofRefs[key] = row
        break
      case "settlement_ref":
        next.settlementRefs[key] = row
        break
      case "world_event":
        next.events[key] = row
        break
      case "projection_cursor":
      case "bridge_health":
        break
    }
  }

  for (const ref of delta.deletedRefs ?? []) {
    delete next.avatars[ref]
    delete next.gateways[ref]
    delete next.positions[ref]
    delete next.chatMessages[ref]
    delete next.chatBubbles[ref]
    delete next.emotes[ref]
    delete next.intents[ref]
    delete next.entities[ref]
    delete next.edges[ref]
    delete next.proofRefs[ref]
    delete next.settlementRefs[ref]
    delete next.events[ref]
  }

  if ((delta.patches ?? []).some(patch =>
    typeof patch === "object" &&
    patch !== null &&
    !sparseWorldPatchChangesOnly(patch as Record<string, unknown>)
  )) {
    throw new Error("Sparse world patches may not contain undefined fields.")
  }

  return next as WorldReadModel
}

export const subscriptionInterestStateFromAttachment = (input: {
  readonly seenRefs?: ReadonlyArray<string>
  readonly tierByRef?: Readonly<Record<string, "near" | "far">>
}): SubscriptionInterestState => ({
  seenRefs: [...(input.seenRefs ?? [])],
  tierByRef: { ...(input.tierByRef ?? {}) },
})

export const defaultSubscriptionCursor = (regionRef: string): string =>
  cursorForSequence(regionRef, 0)

const rejectUnboundedAvatarOrEventRows = (request: WorldSubscriptionPlanRequest): void => {
  const requestedRows = new Set((request.requestedRows ?? []).map(row => row.toLowerCase()))
  const asksForGlobalHotRows = normalizeScope(request.scope, request.selectedEntityRef) === "global" &&
    (requestedRows.has("avatar_position") ||
      requestedRows.has("agent_avatar") ||
      requestedRows.has("world_event") ||
      requestedRows.has("event"))
  const center = normalizeCenter(request.center)
  const hasBound = Number.isFinite(center.x) && Number.isFinite(center.y) && Number.isFinite(center.z)

  if (asksForGlobalHotRows || !hasBound) {
    throw new WorldSubscriptionPolicyError({
      reason: "Unbounded global avatar/event subscriptions are not allowed; use a server-approved region/run/selected-entity interest plan.",
      sourceRef: "subscription.world.policy",
    })
  }
}

const normalizeScope = (
  input: string | null | undefined,
  selectedEntityRef: string | null | undefined,
): WorldSubscriptionScope => {
  if (selectedEntityRef !== null && selectedEntityRef !== undefined && selectedEntityRef.length > 0) {
    return "selected_entity"
  }
  if (input === "global" || input === "run" || input === "region" || input === "selected_entity") {
    return input
  }
  return "region"
}

const normalizeCenter = (
  input: WorldSubscriptionPlanRequest["center"],
): WorldVector3 => ({
  x: boundedNumber(input?.x ?? 0),
  y: boundedNumber(input?.y ?? 0),
  z: boundedNumber(input?.z ?? 0),
}) as WorldVector3

const boundedNumber = (value: unknown): number => {
  const number = typeof value === "number" && Number.isFinite(value) ? value : 0
  return Math.max(-MAX_PUBLIC_INTEREST_RADIUS, Math.min(MAX_PUBLIC_INTEREST_RADIUS, number))
}

const numberParam = (url: URL, key: string): number | undefined => {
  const raw = url.searchParams.get(key)
  if (raw === null) {
    return undefined
  }
  const value = Number(raw)
  return Number.isFinite(value) ? value : undefined
}

const distance3 = (a: WorldVector3, b: WorldVector3): number => {
  const dx = a.x - b.x
  const dy = a.y - b.y
  const dz = a.z - b.z
  return Math.sqrt(dx * dx + dy * dy + dz * dz)
}
