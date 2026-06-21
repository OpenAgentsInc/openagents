import {
  type ChatWorldAvatarPositionRow,
  type ChatWorldAvatarRow,
  type ChatWorldLocalChatMessageRow,
  type ChatWorldMultiplayerProjection,
  type ChatWorldMultiplayerRows,
  type ChatWorldPylonAttentionRow,
  type ChatWorldStationRow,
  chatWorldRegionRefForRun,
  projectChatWorldMultiplayer,
} from "./chat-world-multiplayer"

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
  proximityRadiusMeters: number
  avatarPositionMinIntervalMs: number
}>

export type ChatWorldSpacetimeRows = Readonly<{
  regions: ReadonlyArray<unknown>
  stations: ReadonlyArray<unknown>
  avatars: ReadonlyArray<unknown>
  positions: ReadonlyArray<unknown>
  messages: ReadonlyArray<unknown>
  attention: ReadonlyArray<unknown>
}>

export type ChatWorldSpacetimeProjection = Readonly<{
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
}>

export type ChatWorldAvatarPositionPlan =
  | Readonly<{ ok: true; write: ChatWorldAvatarPositionWrite }>
  | Readonly<{ ok: false; reason: string }>

const DEFAULT_AVATAR_RATE_MS = 100
const MAX_AVATAR_MOVE_METERS_PER_SECOND = 14
const POSITION_EPSILON_METERS = 0.75

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
    proximityRadiusMeters,
    avatarPositionMinIntervalMs,
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

export const normalizeChatWorldSpacetimeRows = (
  rows: ChatWorldSpacetimeRows,
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

export const projectChatWorldSpacetimeRows = (input: {
  readonly flagEnabled: boolean
  readonly runRef: string
  readonly rows: ChatWorldSpacetimeRows | null
  readonly nowMs: number
  readonly worldUrl?: string
  readonly database?: string
}): ChatWorldSpacetimeProjection => {
  if (input.rows === null) {
    return {
      regions: [],
      world: projectChatWorldMultiplayer({
        flagEnabled: false,
        runRef: input.runRef,
        rows: null,
        nowMs: input.nowMs,
        worldUrl: input.worldUrl,
        database: input.database,
      }),
    }
  }
  const normalized = normalizeChatWorldSpacetimeRows(input.rows)
  return {
    regions: normalized.regions,
    world: projectChatWorldMultiplayer({
      flagEnabled: input.flagEnabled,
      runRef: input.runRef,
      rows: normalized.rows,
      nowMs: input.nowMs,
      worldUrl: input.worldUrl,
      database: input.database,
    }),
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
