import {
  type ChatWorldAvatarPositionRow,
  type ChatWorldAvatarRow,
  type ChatWorldGatewayStation,
  type ChatWorldInferenceEvent,
  type ChatWorldLocalChatMessageRow,
  type ChatWorldMultiplayerProjection,
  type ChatWorldMultiplayerRows,
  type ChatWorldPylonAttentionRow,
  type ChatWorldStationRow,
  DEFAULT_OA_CHARACTER_ID,
  chatWorldRegionRefForRun,
  projectChatWorldMultiplayer,
} from "./chat-world-multiplayer.js"
import type { ClientWorld } from "@openagentsinc/world-client"

export type ChatWorldRegionRow = Readonly<{
  regionRef: string
  runRef: string
  label: string
  minX: number
  minY: number
  minZ: number
  maxX: number
  maxY: number
  maxZ: number
  roadDirectionX: number
  roadDirectionY: number
  roadDirectionZ: number
  localOriginX: number
  localOriginY: number
  localOriginZ: number
  starterPylonSiteOffsetX: number
  starterPylonSiteOffsetY: number
  starterPylonSiteOffsetZ: number
  streetPrevRegionRef: string | null
  streetNextRegionRef: string | null
  proximityRadiusMeters: number
  avatarPositionMinIntervalMs: number
  staleAvatarPositionMs: number
}>

export type ChatWorldCloudflareRows = Readonly<{
  regions: ReadonlyArray<unknown>
  stations: ReadonlyArray<unknown>
  avatars: ReadonlyArray<unknown>
  positions: ReadonlyArray<unknown>
  messages: ReadonlyArray<unknown>
  attention: ReadonlyArray<unknown>
}>

export type ChatWorldCloudflareProjection = Readonly<{
  world: ChatWorldMultiplayerProjection
  regions: ReadonlyArray<ChatWorldRegionRow>
}>

export type ChatWorldDesktopAvatarIdentity = Readonly<{
  displayName: string
  pylonRef: string | null
  actorRef: string
}>

export type ChatWorldAvatarPositionWrite = Readonly<{
  regionRef: string
  positionX: number
  positionY: number
  positionZ: number
  yaw: number
  pitch: number
  movementMode: "idle" | "walking" | "running" | "ghost" | "inspecting"
  // The character this account is moving. Threaded to the world module so one
  // account can move many distinct characters; defaults to "main".
  characterId: string
}>

export type ChatWorldAvatarPositionPlan =
  | Readonly<{ ok: true; write: ChatWorldAvatarPositionWrite }>
  | Readonly<{ ok: false; reason: string }>

const DEFAULT_AVATAR_RATE_MS = 100
const MAX_AVATAR_MOVE_METERS_PER_SECOND = 14
const POSITION_EPSILON_METERS = 0.75

export const CHAT_WORLD_STARTER_REGION_CONTRACT = {
  avatarPositionMinIntervalMs: 100,
  bounds: {
    maxX: 160,
    maxY: 40,
    maxZ: 160,
    minX: -160,
    minY: 0,
    minZ: -160,
  },
  localOrigin: { x: 0, y: 0, z: 0 },
  proximityRadiusMeters: 12,
  roadDirection: { x: 0, y: 0, z: 1 },
  staleAvatarPositionMs: 20_000,
  starterPylonSiteOffset: { x: 24, y: 0, z: 0 },
  streetNextRegionRef: "region.run.tassadar.executor.20260615.street.next",
  streetPrevRegionRef: "region.run.tassadar.executor.20260615.street.prev",
} as const

const record = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object"
    ? value as Record<string, unknown>
    : {}

const text = (value: unknown): string =>
  typeof value === "string" ? value.trim() : ""

const optionalText = (value: unknown): string | null => {
  const out = text(value)
  return out.length > 0 ? out : null
}

const numberValue = (value: unknown): number | null => {
  const number =
    typeof value === "bigint"
      ? Number(value)
      : typeof value === "number"
        ? value
        : null
  return number !== null && Number.isFinite(number) ? number : null
}

const rowNumber = (
  row: Record<string, unknown>,
  key: string,
): number | null => numberValue(row[key])

const regionFromRow = (raw: unknown): ChatWorldRegionRow | null => {
  const row = record(raw)
  const regionRef = text(row.regionRef)
  const runRef = text(row.runRef)
  if (regionRef.length === 0 || runRef.length === 0) return null
  const minX = rowNumber(row, "minX")
  const minY = rowNumber(row, "minY")
  const minZ = rowNumber(row, "minZ")
  const maxX = rowNumber(row, "maxX")
  const maxY = rowNumber(row, "maxY")
  const maxZ = rowNumber(row, "maxZ")
  const proximityRadiusMeters = rowNumber(row, "proximityRadiusMeters")
  const avatarPositionMinIntervalMs = rowNumber(row, "avatarPositionMinIntervalMs")
  const staleAvatarPositionMs = rowNumber(row, "staleAvatarPositionMs")
  if (
    minX === null ||
    minY === null ||
    minZ === null ||
    maxX === null ||
    maxY === null ||
    maxZ === null ||
    proximityRadiusMeters === null ||
    avatarPositionMinIntervalMs === null
  ) {
    return null
  }
  const contract = CHAT_WORLD_STARTER_REGION_CONTRACT
  return {
    regionRef,
    runRef,
    label: text(row.label) || regionRef,
    minX,
    minY,
    minZ,
    maxX,
    maxY,
    maxZ,
    roadDirectionX: rowNumber(row, "roadDirectionX") ?? contract.roadDirection.x,
    roadDirectionY: rowNumber(row, "roadDirectionY") ?? contract.roadDirection.y,
    roadDirectionZ: rowNumber(row, "roadDirectionZ") ?? contract.roadDirection.z,
    localOriginX: rowNumber(row, "localOriginX") ?? contract.localOrigin.x,
    localOriginY: rowNumber(row, "localOriginY") ?? contract.localOrigin.y,
    localOriginZ: rowNumber(row, "localOriginZ") ?? contract.localOrigin.z,
    starterPylonSiteOffsetX:
      rowNumber(row, "starterPylonSiteOffsetX") ?? contract.starterPylonSiteOffset.x,
    starterPylonSiteOffsetY:
      rowNumber(row, "starterPylonSiteOffsetY") ?? contract.starterPylonSiteOffset.y,
    starterPylonSiteOffsetZ:
      rowNumber(row, "starterPylonSiteOffsetZ") ?? contract.starterPylonSiteOffset.z,
    streetPrevRegionRef:
      optionalText(row.streetPrevRegionRef) ?? contract.streetPrevRegionRef,
    streetNextRegionRef:
      optionalText(row.streetNextRegionRef) ?? contract.streetNextRegionRef,
    proximityRadiusMeters,
    avatarPositionMinIntervalMs,
    staleAvatarPositionMs: staleAvatarPositionMs ?? contract.staleAvatarPositionMs,
  }
}

const stationFromRow = (raw: unknown): ChatWorldStationRow | null => {
  const row = record(raw)
  const pylonRef = text(row.pylonRef)
  const runRef = text(row.runRef)
  const regionRef = text(row.regionRef)
  const x = rowNumber(row, "positionX")
  const y = rowNumber(row, "positionY")
  const z = rowNumber(row, "positionZ")
  if (
    pylonRef.length === 0 ||
    runRef.length === 0 ||
    regionRef.length === 0 ||
    x === null ||
    y === null ||
    z === null
  ) {
    return null
  }
  return {
    pylonRef,
    runRef,
    regionRef,
    x,
    y,
    z,
    label: text(row.label) || pylonRef,
  }
}

const avatarFromRow = (raw: unknown): ChatWorldAvatarRow | null => {
  const row = record(raw)
  const avatarRef = text(row.avatarRef)
  if (avatarRef.length === 0) return null
  return {
    avatarRef,
    displayName: text(row.displayName) || avatarRef,
    avatarKind: text(row.actorKind) || "agent",
    actorRef: text(row.actorRef) || avatarRef,
    colorHex: text(row.colorHex) || "#f5b73a",
  }
}

const positionFromRow = (raw: unknown): ChatWorldAvatarPositionRow | null => {
  const row = record(raw)
  const avatarRef = text(row.avatarRef)
  const regionRef = text(row.regionRef)
  const x = rowNumber(row, "positionX")
  const y = rowNumber(row, "positionY")
  const z = rowNumber(row, "positionZ")
  const lastSeenEpochMs = rowNumber(row, "lastSeenEpochMs")
  if (
    avatarRef.length === 0 ||
    regionRef.length === 0 ||
    x === null ||
    y === null ||
    z === null ||
    lastSeenEpochMs === null
  ) {
    return null
  }
  return {
    avatarRef,
    regionRef,
    x,
    y,
    z,
    yaw: rowNumber(row, "yaw") ?? 0,
    movementMode: text(row.movementMode) || "idle",
    lastSeenEpochMs,
    presenceFeed: text(row.presenceFeed) === "low" ? "low" : "high",
  }
}

const messageFromRow = (raw: unknown): ChatWorldLocalChatMessageRow | null => {
  const row = record(raw)
  const messageRef = text(row.messageRef)
  const avatarRef = text(row.speakerAvatarRef)
  const regionRef = text(row.regionRef)
  const expiresAtEpochMs = rowNumber(row, "expiresAtEpochMs")
  if (
    messageRef.length === 0 ||
    avatarRef.length === 0 ||
    regionRef.length === 0 ||
    expiresAtEpochMs === null
  ) {
    return null
  }
  return {
    messageRef,
    avatarRef,
    regionRef,
    text: text(row.body),
    radiusMeters: rowNumber(row, "radiusMeters") ?? 0,
    expiresAtEpochMs,
  }
}

const attentionFromRow = (raw: unknown): ChatWorldPylonAttentionRow | null => {
  const row = record(raw)
  const attentionRef = text(row.attentionRef)
  const avatarRef = text(row.avatarRef)
  const pylonRef = text(row.pylonRef)
  const expiresAtEpochMs = rowNumber(row, "expiresAtEpochMs")
  if (
    attentionRef.length === 0 ||
    avatarRef.length === 0 ||
    pylonRef.length === 0 ||
    expiresAtEpochMs === null
  ) {
    return null
  }
  return {
    attentionRef,
    avatarRef,
    pylonRef,
    attentionKind: text(row.attentionKind) || "nearby",
    expiresAtEpochMs,
  }
}

const compact = <T>(items: ReadonlyArray<T | null>): ReadonlyArray<T> =>
  items.filter((item): item is T => item !== null)

export const normalizeChatWorldCloudflareRows = (
  rows: ChatWorldCloudflareRows,
): { readonly regions: ReadonlyArray<ChatWorldRegionRow>; readonly rows: ChatWorldMultiplayerRows } => ({
  regions: compact(rows.regions.map(regionFromRow)),
  rows: {
    stations: compact(rows.stations.map(stationFromRow)),
    avatars: compact(rows.avatars.map(avatarFromRow)),
    positions: compact(rows.positions.map(positionFromRow)),
    messages: compact(rows.messages.map(messageFromRow)),
    attention: compact(rows.attention.map(attentionFromRow)),
  },
})

export const projectChatWorldCloudflareRows = (input: {
  readonly flagEnabled: boolean
  readonly runRef: string
  readonly rows: ChatWorldCloudflareRows | null
  readonly nowMs: number
  readonly worldUrl?: string
  readonly database?: string
  readonly localAvatarRef?: string | null
}): ChatWorldCloudflareProjection => {
  const multiplayerOptions = {
    ...(input.worldUrl !== undefined ? { worldUrl: input.worldUrl } : {}),
    ...(input.database !== undefined ? { database: input.database } : {}),
    ...(input.localAvatarRef !== undefined
      ? { localAvatarRef: input.localAvatarRef }
      : {}),
  }
  if (input.rows === null) {
    return {
      regions: [],
      world: projectChatWorldMultiplayer({
        flagEnabled: false,
        runRef: input.runRef,
        rows: null,
        nowMs: input.nowMs,
        ...multiplayerOptions,
      }),
    }
  }
  const normalized = normalizeChatWorldCloudflareRows(input.rows)
  return {
    regions: normalized.regions,
    world: projectChatWorldMultiplayer({
      flagEnabled: input.flagEnabled,
      runRef: input.runRef,
      rows: normalized.rows,
      nowMs: input.nowMs,
      ...multiplayerOptions,
    }),
  }
}

const epochMs = (value: string | undefined, fallback: number): number => {
  if (value === undefined) return fallback
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : fallback
}

const uniqueTexts = (values: ReadonlyArray<string | undefined>): ReadonlyArray<string> => {
  const out: string[] = []
  for (const value of values) {
    const trimmed = value?.trim() ?? ""
    if (trimmed.length > 0 && !out.includes(trimmed)) out.push(trimmed)
  }
  return out
}

const regionRunRef = (regionRef: string, fallback: string): string => {
  const prefix = "region."
  const suffix = ".street"
  return regionRef.startsWith(prefix) && regionRef.endsWith(suffix)
    ? regionRef.slice(prefix.length, -suffix.length)
    : fallback
}

export const projectChatWorldClientWorld = (input: {
  readonly flagEnabled: boolean
  readonly runRef: string
  readonly readModel: ClientWorld | null
  readonly nowMs: number
  readonly worldUrl?: string
  readonly database?: string
  readonly localAvatarRef?: string | null
}): ChatWorldCloudflareProjection => {
  const multiplayerOptions = {
    ...(input.worldUrl !== undefined ? { worldUrl: input.worldUrl } : {}),
    ...(input.database !== undefined ? { database: input.database } : {}),
    ...(input.localAvatarRef !== undefined
      ? { localAvatarRef: input.localAvatarRef }
      : {}),
  }
  if (input.flagEnabled !== true || input.readModel === null) {
    return projectChatWorldCloudflareRows({
      flagEnabled: false,
      runRef: input.runRef,
      rows: null,
      nowMs: input.nowMs,
      ...multiplayerOptions,
    })
  }

  const contract = CHAT_WORLD_STARTER_REGION_CONTRACT
  const readModelRegions = Object.values(input.readModel.regions)
  const regions: ReadonlyArray<ChatWorldRegionRow> = (readModelRegions.length > 0
    ? readModelRegions
    : [{
        bounds: {
          max: {
            x: contract.bounds.maxX,
            y: contract.bounds.maxY,
            z: contract.bounds.maxZ,
          },
          min: {
            x: contract.bounds.minX,
            y: contract.bounds.minY,
            z: contract.bounds.minZ,
          },
        },
        label: "Tassadar Street",
        origin: {
          x: contract.localOrigin.x,
          y: contract.localOrigin.y,
          z: contract.localOrigin.z,
        },
        proximityRadius: contract.proximityRadiusMeters,
        regionRef: input.readModel.regionRef,
        staleAvatarTtlMs: contract.staleAvatarPositionMs,
      }])
    .map(region => ({
      regionRef: region.regionRef,
      runRef: regionRunRef(region.regionRef, input.runRef),
      label: region.label,
      minX: region.bounds.min.x,
      minY: region.bounds.min.y,
      minZ: region.bounds.min.z,
      maxX: region.bounds.max.x,
      maxY: region.bounds.max.y,
      maxZ: region.bounds.max.z,
      roadDirectionX: contract.roadDirection.x,
      roadDirectionY: contract.roadDirection.y,
      roadDirectionZ: contract.roadDirection.z,
      localOriginX: region.origin.x,
      localOriginY: region.origin.y,
      localOriginZ: region.origin.z,
      starterPylonSiteOffsetX: contract.starterPylonSiteOffset.x,
      starterPylonSiteOffsetY: contract.starterPylonSiteOffset.y,
      starterPylonSiteOffsetZ: contract.starterPylonSiteOffset.z,
      streetPrevRegionRef: contract.streetPrevRegionRef,
      streetNextRegionRef: contract.streetNextRegionRef,
      proximityRadiusMeters: region.proximityRadius,
      avatarPositionMinIntervalMs: contract.avatarPositionMinIntervalMs,
      staleAvatarPositionMs: region.staleAvatarTtlMs,
    }))

  const rows: ChatWorldMultiplayerRows = {
    stations: Object.values(input.readModel.pylons).map(station => ({
      pylonRef: station.pylonRef,
      runRef: input.runRef,
      regionRef: station.regionRef,
      label: station.label,
      x: station.position.x,
      y: station.position.y,
      z: station.position.z,
    })),
    avatars: Object.values(input.readModel.avatars).map(avatar => ({
      avatarRef: avatar.avatarRef,
      actorRef: avatar.accountRef ?? avatar.avatarRef,
      avatarKind: avatar.avatarKind,
      displayName: avatar.label,
      colorHex: "#f5b73a",
    })),
    positions: Object.values(input.readModel.positions).map(position => ({
      avatarRef: position.avatarRef,
      regionRef: position.regionRef,
      x: position.position.x,
      y: position.position.y,
      z: position.position.z,
      yaw: position.rotationY,
      movementMode: position.animation === "run"
        ? "running"
        : position.animation === "walk"
          ? "walking"
          : "idle",
      lastSeenEpochMs: epochMs(position.observedAt, input.nowMs),
      presenceFeed: "high",
    })),
    messages: Object.values(input.readModel.chatMessages).map(message => ({
      messageRef: message.messageRef,
      avatarRef: message.avatarRef,
      regionRef: message.regionRef,
      text: message.text,
      radiusMeters: 12,
      expiresAtEpochMs: epochMs(message.expiresAt, input.nowMs + 60_000),
    })),
    attention: Object.values(input.readModel.intents).flatMap(intent => {
      if (intent.intent !== "focus_pylon" || intent.targetRef === undefined) return []
      return [{
        attentionRef: intent.intentRef,
        avatarRef: intent.avatarRef,
        pylonRef: intent.targetRef,
        attentionKind: "looking",
        expiresAtEpochMs: epochMs(intent.expiresAt, input.nowMs + 30_000),
      }]
    }),
  }

  const gateways: ReadonlyArray<ChatWorldGatewayStation> = Object.values(input.readModel.gateways)
    .filter(gateway => gateway.regionRef === input.readModel?.regionRef)
    .map(gateway => ({
      gatewayRef: gateway.gatewayRef,
      label: gateway.label,
      providerLabel: gateway.providerLabel,
      lane: gateway.lane,
      status: gateway.status,
      x: gateway.position.x,
      y: gateway.position.y,
      z: gateway.position.z,
    }))

  const inferenceEvents: ReadonlyArray<ChatWorldInferenceEvent> = Object.values(input.readModel.events)
    .flatMap(event => {
      if (event.inference === undefined) return []
      if (event.regionRef !== undefined && event.regionRef !== input.readModel?.regionRef) return []
      const gatewayWorker = event.inference.workers.find(worker => worker.workerKind === "gateway")
      return [{
        eventRef: event.eventRef,
        requestRef: event.inference.requestRef,
        receiptRef: event.inference.receiptRef,
        model: event.inference.model,
        route: event.inference.route,
        gatewayRef: gatewayWorker?.workerRef ?? null,
        workerRefs: event.inference.workers.map(worker => worker.workerRef),
        verification: event.inference.verification,
        costMsat: event.inference.costMsat,
        settled: event.inference.settled,
        sourceRefs: uniqueTexts([
          event.inference.receiptRef,
          ...event.sourceRefs,
          ...event.inference.sourceRefs,
          ...event.inference.workers.flatMap(worker => worker.sourceRefs),
        ]),
        generatedAt: event.createdAt,
      }]
    })

  return {
    regions,
    world: {
      ...projectChatWorldMultiplayer({
        flagEnabled: true,
        runRef: input.runRef,
        rows,
        nowMs: input.nowMs,
        ...multiplayerOptions,
      }),
      gateways,
      inferenceEvents,
    },
  }
}

export const chatWorldDesktopAvatarIdentity = (input: {
  readonly pylonRef?: string | null
  readonly nodeLabel?: string | null
  readonly fallbackActorRef?: string | null
}): ChatWorldDesktopAvatarIdentity => {
  const pylonRef = optionalText(input.pylonRef)
  const nodeLabel = optionalText(input.nodeLabel)
  const actorRef = pylonRef ?? optionalText(input.fallbackActorRef) ?? "desktop.local"
  return {
    pylonRef,
    actorRef,
    displayName: nodeLabel ?? (pylonRef === null ? "Autopilot Desktop" : `Pylon ${pylonRef}`),
  }
}

const movementModes = new Set([
  "idle",
  "walking",
  "running",
  "ghost",
  "inspecting",
])

const insideRegion = (
  region: ChatWorldRegionRow,
  x: number,
  y: number,
  z: number,
): boolean =>
  x >= region.minX &&
  x <= region.maxX &&
  y >= region.minY &&
  y <= region.maxY &&
  z >= region.minZ &&
  z <= region.maxZ

export const planChatWorldAvatarPositionWrite = (input: {
  readonly region: ChatWorldRegionRow | null
  readonly previous: ChatWorldAvatarPositionRow | null
  readonly nowMs: number
  readonly x: number
  readonly y: number
  readonly z: number
  readonly yaw?: number
  readonly pitch?: number
  readonly movementMode?: string
  readonly characterId?: string
}): ChatWorldAvatarPositionPlan => {
  const region = input.region
  if (region === null) return { ok: false, reason: "region unavailable" }
  const yaw = input.yaw ?? 0
  const pitch = input.pitch ?? 0
  if (
    !Number.isFinite(input.x) ||
    !Number.isFinite(input.y) ||
    !Number.isFinite(input.z) ||
    !Number.isFinite(yaw) ||
    !Number.isFinite(pitch) ||
    !Number.isFinite(input.nowMs)
  ) {
    return { ok: false, reason: "position is not finite" }
  }
  if (!insideRegion(region, input.x, input.y, input.z)) {
    return { ok: false, reason: "position outside region bounds" }
  }
  const movementMode = input.movementMode ?? "idle"
  if (!movementModes.has(movementMode)) {
    return { ok: false, reason: "movement mode unavailable" }
  }
  const minIntervalMs = Math.max(
    DEFAULT_AVATAR_RATE_MS,
    region.avatarPositionMinIntervalMs,
  )
  const previous = input.previous
  if (previous !== null) {
    if (previous.regionRef !== region.regionRef) {
      return { ok: false, reason: "avatar must join region before moving" }
    }
    const elapsedMs = input.nowMs - previous.lastSeenEpochMs
    if (elapsedMs < minIntervalMs) {
      return { ok: false, reason: "position update rate limited" }
    }
    const dx = input.x - previous.x
    const dy = input.y - previous.y
    const dz = input.z - previous.z
    const distance = Math.sqrt(dx * dx + dy * dy + dz * dz)
    const allowed =
      MAX_AVATAR_MOVE_METERS_PER_SECOND * (elapsedMs / 1_000) +
      POSITION_EPSILON_METERS
    if (distance > allowed) {
      return { ok: false, reason: "position jump exceeds movement limit" }
    }
  }
  return {
    ok: true,
    write: {
      regionRef: region.regionRef,
      positionX: Number(input.x.toFixed(3)),
      positionY: Number(input.y.toFixed(3)),
      positionZ: Number(input.z.toFixed(3)),
      yaw: Number(yaw.toFixed(3)),
      pitch: Number(pitch.toFixed(3)),
      movementMode: movementMode as ChatWorldAvatarPositionWrite["movementMode"],
      characterId: input.characterId ?? DEFAULT_OA_CHARACTER_ID,
    },
  }
}

export const defaultChatWorldRegionForRun = (
  regions: ReadonlyArray<ChatWorldRegionRow>,
  runRef: string,
): ChatWorldRegionRow | null => {
  const regionRef = chatWorldRegionRefForRun(runRef)
  return regions.find((region) => region.regionRef === regionRef) ?? null
}
