import { Schema as S } from "effect"

export const WORLD_CONTRACT_SCHEMA_VERSION = "openagents.world_contract.v1"
export const WORLD_DELTA_SCHEMA_VERSION = "openagents.world_delta.v1"
export const WORLD_READ_MODEL_SCHEMA_VERSION = "openagents.world_read_model.v1"

export const WorldSchemaVersion = S.Literal(WORLD_CONTRACT_SCHEMA_VERSION)
export type WorldSchemaVersion = typeof WorldSchemaVersion.Type

export const WorldRef = S.String.pipe(S.brand("WorldRef"))
export type WorldRef = typeof WorldRef.Type

export const WorldRegionRef = S.String.pipe(S.brand("WorldRegionRef"))
export type WorldRegionRef = typeof WorldRegionRef.Type

export const WorldAvatarRef = S.String.pipe(S.brand("WorldAvatarRef"))
export type WorldAvatarRef = typeof WorldAvatarRef.Type

export const WorldCharacterId = S.String.pipe(S.brand("WorldCharacterId"))
export type WorldCharacterId = typeof WorldCharacterId.Type

export const WorldPylonRef = S.String.pipe(S.brand("WorldPylonRef"))
export type WorldPylonRef = typeof WorldPylonRef.Type

export const WorldGatewayRef = S.String.pipe(S.brand("WorldGatewayRef"))
export type WorldGatewayRef = typeof WorldGatewayRef.Type

export const WorldRunRef = S.String.pipe(S.brand("WorldRunRef"))
export type WorldRunRef = typeof WorldRunRef.Type

export const WorldEntityRef = S.String.pipe(S.brand("WorldEntityRef"))
export type WorldEntityRef = typeof WorldEntityRef.Type

export const WorldEventRef = S.String.pipe(S.brand("WorldEventRef"))
export type WorldEventRef = typeof WorldEventRef.Type

export const WorldCursor = S.String.pipe(S.brand("WorldCursor"))
export type WorldCursor = typeof WorldCursor.Type

export const WorldIsoTimestamp = S.String.pipe(S.brand("WorldIsoTimestamp"))
export type WorldIsoTimestamp = typeof WorldIsoTimestamp.Type

export const WorldSequence = S.Int.pipe(S.brand("WorldSequence"))
export type WorldSequence = typeof WorldSequence.Type

export const WorldBoundedNumber = S.Number.pipe(S.brand("WorldBoundedNumber"))
export type WorldBoundedNumber = typeof WorldBoundedNumber.Type

export const WorldSourceRef = S.String.pipe(S.brand("WorldSourceRef"))
export type WorldSourceRef = typeof WorldSourceRef.Type

export const WorldActorClass = S.Literals([
  "browser",
  "agent",
  "service",
  "operator",
])
export type WorldActorClass = typeof WorldActorClass.Type

export const worldActorClasses: ReadonlyArray<WorldActorClass> = [
  "browser",
  "agent",
  "service",
  "operator",
]

export const WorldErrorTag = S.Literals([
  "validation",
  "auth",
  "redaction",
  "command",
  "storage",
  "cursor",
  "bridge",
])
export type WorldErrorTag = typeof WorldErrorTag.Type

export const worldErrorTags: ReadonlyArray<WorldErrorTag> = [
  "validation",
  "auth",
  "redaction",
  "command",
  "storage",
  "cursor",
  "bridge",
]

export const WorldCommandName = S.Literals([
  "join_region",
  "leave_region",
  "set_avatar_position",
  "focus_pylon",
  "clear_pylon_focus",
  "send_local_message",
  "send_pylon_message",
  "send_emote",
  "set_agent_intent",
  "upsert_training_run",
  "upsert_run_entity",
  "upsert_world_edge",
  "upsert_proof_ref",
  "upsert_settlement_ref",
  "append_world_event",
  "advance_projection_cursor",
  "record_bridge_health",
  "upsert_world_region",
  "upsert_pylon_station",
  "upsert_gateway_station",
  "record_system_world_message",
  "expire_interaction_rows",
])
export type WorldCommandName = typeof WorldCommandName.Type

export const browserWorldCommandNames: ReadonlyArray<WorldCommandName> = [
  "join_region",
  "leave_region",
  "set_avatar_position",
  "focus_pylon",
  "clear_pylon_focus",
  "send_local_message",
  "send_pylon_message",
  "send_emote",
  "set_agent_intent",
]

export const serviceWorldCommandNames: ReadonlyArray<WorldCommandName> = [
  "upsert_training_run",
  "upsert_run_entity",
  "upsert_world_edge",
  "upsert_proof_ref",
  "upsert_settlement_ref",
  "append_world_event",
  "advance_projection_cursor",
  "record_bridge_health",
  "upsert_world_region",
  "upsert_pylon_station",
  "upsert_gateway_station",
  "record_system_world_message",
  "expire_interaction_rows",
]

export const WorldCommandReceiptStatus = S.Literals([
  "accepted",
  "applied",
  "duplicate",
  "stale",
  "rejected",
])
export type WorldCommandReceiptStatus = typeof WorldCommandReceiptStatus.Type

export const WorldDeltaKind = S.Literals([
  "snapshot",
  "update",
  "delete",
  "heartbeat",
  "diagnostic",
])
export type WorldDeltaKind = typeof WorldDeltaKind.Type

export const WorldRowKind = S.Literals([
  "world_region",
  "pylon_station",
  "gateway_station",
  "agent_avatar",
  "avatar_position",
  "local_chat_message",
  "chat_bubble",
  "local_emote",
  "agent_intent",
  "training_run",
  "run_entity",
  "world_edge",
  "proof_ref",
  "settlement_ref",
  "world_event",
  "projection_cursor",
  "bridge_health",
])
export type WorldRowKind = typeof WorldRowKind.Type

export const worldRowKinds: ReadonlyArray<WorldRowKind> = [
  "world_region",
  "pylon_station",
  "gateway_station",
  "agent_avatar",
  "avatar_position",
  "local_chat_message",
  "chat_bubble",
  "local_emote",
  "agent_intent",
  "training_run",
  "run_entity",
  "world_edge",
  "proof_ref",
  "settlement_ref",
  "world_event",
  "projection_cursor",
  "bridge_health",
]

export class WorldVector3 extends S.Class<WorldVector3>("WorldVector3")({
  x: WorldBoundedNumber,
  y: WorldBoundedNumber,
  z: WorldBoundedNumber,
}) {}
export type WorldVector3Encoded = typeof WorldVector3.Encoded

export class WorldRegionBounds extends S.Class<WorldRegionBounds>("WorldRegionBounds")({
  min: WorldVector3,
  max: WorldVector3,
}) {}

export class WorldStaleness extends S.Class<WorldStaleness>("WorldStaleness")({
  generatedAt: WorldIsoTimestamp,
  maxStalenessSeconds: S.Number,
  transitionRefs: S.Array(WorldSourceRef),
}) {}

export class WorldPublicSafety extends S.Class<WorldPublicSafety>("WorldPublicSafety")({
  publicProjectionAllowed: S.Boolean,
  sourceRefs: S.Array(WorldSourceRef),
  blockerRefs: S.Array(WorldSourceRef),
  caveatRefs: S.Array(WorldSourceRef),
}) {}

export class WorldRegionRow extends S.Class<WorldRegionRow>("WorldRegionRow")({
  kind: S.Literal("world_region"),
  regionRef: WorldRegionRef,
  label: S.String,
  bounds: WorldRegionBounds,
  origin: WorldVector3,
  proximityRadius: S.Number,
  staleAvatarTtlMs: S.Int,
  updatedAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldPylonStationRow extends S.Class<WorldPylonStationRow>("WorldPylonStationRow")({
  kind: S.Literal("pylon_station"),
  pylonRef: WorldPylonRef,
  regionRef: WorldRegionRef,
  label: S.String,
  position: WorldVector3,
  status: S.Literals(["unknown", "online", "working", "offline", "blocked"]),
  updatedAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export const WorldGatewayLane = S.Literals([
  "vertex",
  "fireworks",
  "openrouter",
  "passthrough",
])
export type WorldGatewayLane = typeof WorldGatewayLane.Type

export class WorldGatewayStationRow extends S.Class<WorldGatewayStationRow>("WorldGatewayStationRow")({
  kind: S.Literal("gateway_station"),
  gatewayRef: WorldGatewayRef,
  regionRef: WorldRegionRef,
  lane: WorldGatewayLane,
  label: S.String,
  providerLabel: S.String,
  position: WorldVector3,
  status: S.Literals(["unknown", "online", "working", "offline", "blocked"]),
  updatedAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldAgentAvatarRow extends S.Class<WorldAgentAvatarRow>("WorldAgentAvatarRow")({
  kind: S.Literal("agent_avatar"),
  avatarRef: WorldAvatarRef,
  accountRef: S.optionalKey(WorldRef),
  characterId: WorldCharacterId,
  regionRef: WorldRegionRef,
  label: S.String,
  avatarKind: S.Literals(["human", "pylon_agent", "service_agent", "guest"]),
  updatedAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldAvatarPositionRow extends S.Class<WorldAvatarPositionRow>("WorldAvatarPositionRow")({
  kind: S.Literal("avatar_position"),
  avatarRef: WorldAvatarRef,
  regionRef: WorldRegionRef,
  position: WorldVector3,
  rotationY: S.Number,
  animation: S.Literals(["idle", "walk", "run", "emote", "unknown"]),
  observedAt: WorldIsoTimestamp,
  seq: S.optionalKey(WorldSequence),
  safety: WorldPublicSafety,
}) {}

export class WorldLocalChatMessageRow extends S.Class<WorldLocalChatMessageRow>("WorldLocalChatMessageRow")({
  kind: S.Literal("local_chat_message"),
  messageRef: WorldRef,
  regionRef: WorldRegionRef,
  avatarRef: WorldAvatarRef,
  channel: S.Literals(["local", "run", "pylon", "forum", "system"]),
  text: S.String,
  moderationState: S.Literals(["visible", "masked", "blocked", "muted"]),
  createdAt: WorldIsoTimestamp,
  expiresAt: S.optionalKey(WorldIsoTimestamp),
  safety: WorldPublicSafety,
}) {}

export class WorldChatBubbleRow extends S.Class<WorldChatBubbleRow>("WorldChatBubbleRow")({
  kind: S.Literal("chat_bubble"),
  bubbleRef: WorldRef,
  messageRef: WorldRef,
  regionRef: WorldRegionRef,
  avatarRef: WorldAvatarRef,
  text: S.String,
  createdAt: WorldIsoTimestamp,
  expiresAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldLocalEmoteRow extends S.Class<WorldLocalEmoteRow>("WorldLocalEmoteRow")({
  kind: S.Literal("local_emote"),
  emoteRef: WorldRef,
  avatarRef: WorldAvatarRef,
  regionRef: WorldRegionRef,
  emote: S.String,
  createdAt: WorldIsoTimestamp,
  expiresAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldAgentIntentRow extends S.Class<WorldAgentIntentRow>("WorldAgentIntentRow")({
  kind: S.Literal("agent_intent"),
  intentRef: WorldRef,
  avatarRef: WorldAvatarRef,
  regionRef: WorldRegionRef,
  intent: S.String,
  targetRef: S.optionalKey(WorldRef),
  createdAt: WorldIsoTimestamp,
  expiresAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldTrainingRunRow extends S.Class<WorldTrainingRunRow>("WorldTrainingRunRow")({
  kind: S.Literal("training_run"),
  runRef: WorldRunRef,
  label: S.String,
  state: S.Literals(["pending", "assigned", "tracing", "replaying", "accepted", "rejected", "settled", "blocked"]),
  updatedAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldRunEntityRow extends S.Class<WorldRunEntityRow>("WorldRunEntityRow")({
  kind: S.Literal("run_entity"),
  entityRef: WorldEntityRef,
  runRef: WorldRunRef,
  label: S.String,
  entityKind: S.String,
  updatedAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldEdgeRow extends S.Class<WorldEdgeRow>("WorldEdgeRow")({
  kind: S.Literal("world_edge"),
  edgeRef: WorldRef,
  fromRef: WorldRef,
  toRef: WorldRef,
  relation: S.String,
  updatedAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldProofRefRow extends S.Class<WorldProofRefRow>("WorldProofRefRow")({
  kind: S.Literal("proof_ref"),
  proofRef: WorldRef,
  runRef: WorldRunRef,
  label: S.String,
  url: S.String,
  updatedAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldSettlementRefRow extends S.Class<WorldSettlementRefRow>("WorldSettlementRefRow")({
  kind: S.Literal("settlement_ref"),
  settlementRef: WorldRef,
  runRef: WorldRunRef,
  label: S.String,
  amountSats: S.optionalKey(S.Number),
  updatedAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export const WorldInferenceWorkerKind = S.Literals([
  "coordinator",
  "pylon",
  "gateway",
  "coding_agent",
  "verifier",
])
export type WorldInferenceWorkerKind = typeof WorldInferenceWorkerKind.Type

export const WorldInferenceVerification = S.Literals([
  "none",
  "seeded",
  "test_passed",
  "exact_trace_replay",
  "failed",
  "unknown",
])
export type WorldInferenceVerification = typeof WorldInferenceVerification.Type

export class WorldInferenceWorkerRef extends S.Class<WorldInferenceWorkerRef>("WorldInferenceWorkerRef")({
  workerRef: WorldRef,
  workerKind: WorldInferenceWorkerKind,
  label: S.String,
  role: S.optionalKey(S.String),
  sourceRefs: S.Array(WorldSourceRef),
}) {}

export class WorldInferenceEventPayload extends S.Class<WorldInferenceEventPayload>("WorldInferenceEventPayload")({
  requestRef: WorldRef,
  receiptRef: WorldSourceRef,
  model: S.String,
  route: S.String,
  workers: S.Array(WorldInferenceWorkerRef),
  verification: WorldInferenceVerification,
  costMsat: S.optionalKey(S.Number),
  priceMsat: S.optionalKey(S.Number),
  settled: S.Boolean,
  sourceRefs: S.Array(WorldSourceRef),
}) {}

export class WorldEventRow extends S.Class<WorldEventRow>("WorldEventRow")({
  kind: S.Literal("world_event"),
  eventRef: WorldEventRef,
  regionRef: S.optionalKey(WorldRegionRef),
  runRef: S.optionalKey(WorldRunRef),
  eventKind: S.String,
  text: S.String,
  createdAt: WorldIsoTimestamp,
  sourceRefs: S.Array(WorldSourceRef),
  inference: S.optionalKey(WorldInferenceEventPayload),
  safety: WorldPublicSafety,
}) {}

export class WorldProjectionCursorRow extends S.Class<WorldProjectionCursorRow>("WorldProjectionCursorRow")({
  kind: S.Literal("projection_cursor"),
  cursorRef: WorldRef,
  sourceRef: WorldSourceRef,
  cursor: WorldCursor,
  observedAt: WorldIsoTimestamp,
  safety: WorldPublicSafety,
}) {}

export class WorldBridgeHealthRow extends S.Class<WorldBridgeHealthRow>("WorldBridgeHealthRow")({
  kind: S.Literal("bridge_health"),
  bridgeRef: WorldRef,
  sourceRef: WorldSourceRef,
  status: S.Literals(["current", "stale", "failed", "disabled"]),
  observedAt: WorldIsoTimestamp,
  lagSeconds: S.optionalKey(S.Number),
  diagnosticRefs: S.Array(WorldSourceRef),
  safety: WorldPublicSafety,
}) {}

export const WorldRow = S.Union([
  WorldRegionRow,
  WorldPylonStationRow,
  WorldGatewayStationRow,
  WorldAgentAvatarRow,
  WorldAvatarPositionRow,
  WorldLocalChatMessageRow,
  WorldChatBubbleRow,
  WorldLocalEmoteRow,
  WorldAgentIntentRow,
  WorldTrainingRunRow,
  WorldRunEntityRow,
  WorldEdgeRow,
  WorldProofRefRow,
  WorldSettlementRefRow,
  WorldEventRow,
  WorldProjectionCursorRow,
  WorldBridgeHealthRow,
])
export type WorldRow = typeof WorldRow.Type

export class WorldErrorEnvelope extends S.Class<WorldErrorEnvelope>("WorldErrorEnvelope")({
  tag: WorldErrorTag,
  message: S.String,
  retryable: S.Boolean,
  publicSafe: S.Boolean,
  sourceRefs: S.Array(WorldSourceRef),
}) {}

export class WorldDiagnostic extends S.Class<WorldDiagnostic>("WorldDiagnostic")({
  diagnosticRef: WorldRef,
  tag: WorldErrorTag,
  severity: S.Literals(["debug", "info", "warn", "error"]),
  message: S.String,
  observedAt: WorldIsoTimestamp,
  sourceRefs: S.Array(WorldSourceRef),
}) {}

export class WorldBridgePayload extends S.Class<WorldBridgePayload>("WorldBridgePayload")({
  payloadRef: WorldRef,
  sourceRef: WorldSourceRef,
  observedAt: WorldIsoTimestamp,
  rows: S.Array(WorldRow),
  cursor: S.optionalKey(WorldCursor),
}) {}

export class WorldInterestPlan extends S.Class<WorldInterestPlan>("WorldInterestPlan")({
  center: WorldVector3,
  enterRadius: S.Number,
  dropRadius: S.Number,
  nearRadius: S.Number,
  farRadius: S.Number,
  selectedRefs: S.Array(WorldRef),
}) {}

export class WorldSubscriptionPlan extends S.Class<WorldSubscriptionPlan>("WorldSubscriptionPlan")({
  planRef: WorldRef,
  regionRef: WorldRegionRef,
  scope: S.Literals(["global", "run", "region", "selected_entity"]),
  runRef: S.optionalKey(WorldRunRef),
  selectedEntityRef: S.optionalKey(WorldRef),
  interest: WorldInterestPlan,
  nearUpdateMs: S.Int,
  farUpdateMs: S.Int,
  resumeCursor: S.optionalKey(WorldCursor),
}) {}

export class WorldCommandEnvelope extends S.Class<WorldCommandEnvelope>("WorldCommandEnvelope")({
  schemaVersion: WorldSchemaVersion,
  commandRef: WorldRef,
  command: WorldCommandName,
  actorClass: WorldActorClass,
  actorRef: WorldRef,
  regionRef: S.optionalKey(WorldRegionRef),
  seq: S.optionalKey(WorldSequence),
  issuedAt: WorldIsoTimestamp,
  payload: S.Unknown,
}) {}

export class WorldCommandReceipt extends S.Class<WorldCommandReceipt>("WorldCommandReceipt")({
  receiptRef: WorldRef,
  commandRef: WorldRef,
  command: WorldCommandName,
  status: WorldCommandReceiptStatus,
  actorClass: WorldActorClass,
  acceptedSeq: S.optionalKey(WorldSequence),
  appliedSeq: S.optionalKey(WorldSequence),
  rejectedSeq: S.optionalKey(WorldSequence),
  observedAt: WorldIsoTimestamp,
  changedRefs: S.Array(WorldRef),
  error: S.optionalKey(WorldErrorEnvelope),
}) {}

export class WorldDelta extends S.Class<WorldDelta>("WorldDelta")({
  schemaVersion: S.Literal(WORLD_DELTA_SCHEMA_VERSION),
  deltaRef: WorldRef,
  kind: WorldDeltaKind,
  regionRef: WorldRegionRef,
  cursor: WorldCursor,
  generatedAt: WorldIsoTimestamp,
  rows: S.optionalKey(S.Array(WorldRow)),
  patches: S.optionalKey(S.Array(S.Unknown)),
  deletedRefs: S.optionalKey(S.Array(WorldRef)),
  receipt: S.optionalKey(WorldCommandReceipt),
  diagnostic: S.optionalKey(WorldDiagnostic),
}) {}

export class WorldReadModel extends S.Class<WorldReadModel>("WorldReadModel")({
  schemaVersion: S.Literal(WORLD_READ_MODEL_SCHEMA_VERSION),
  regionRef: WorldRegionRef,
  cursor: WorldCursor,
  generatedAt: WorldIsoTimestamp,
  regions: S.Record(S.String, WorldRegionRow),
  pylons: S.Record(S.String, WorldPylonStationRow),
  gateways: S.Record(S.String, WorldGatewayStationRow),
  avatars: S.Record(S.String, WorldAgentAvatarRow),
  positions: S.Record(S.String, WorldAvatarPositionRow),
  chatMessages: S.Record(S.String, WorldLocalChatMessageRow),
  chatBubbles: S.Record(S.String, WorldChatBubbleRow),
  emotes: S.Record(S.String, WorldLocalEmoteRow),
  intents: S.Record(S.String, WorldAgentIntentRow),
  runs: S.Record(S.String, WorldTrainingRunRow),
  entities: S.Record(S.String, WorldRunEntityRow),
  edges: S.Record(S.String, WorldEdgeRow),
  proofRefs: S.Record(S.String, WorldProofRefRow),
  settlementRefs: S.Record(S.String, WorldSettlementRefRow),
  events: S.Record(S.String, WorldEventRow),
  diagnostics: S.Array(WorldDiagnostic),
}) {}
export type ClientWorld = WorldReadModel

export const decodeWorldRow = S.decodeUnknownSync(WorldRow)
export const decodeWorldCommandEnvelope = S.decodeUnknownSync(WorldCommandEnvelope)
export const decodeWorldCommandReceipt = S.decodeUnknownSync(WorldCommandReceipt)
export const decodeWorldDelta = S.decodeUnknownSync(WorldDelta)
export const decodeWorldReadModel = S.decodeUnknownSync(WorldReadModel)
export const decodeWorldSubscriptionPlan = S.decodeUnknownSync(WorldSubscriptionPlan)
export const decodeWorldBridgePayload = S.decodeUnknownSync(WorldBridgePayload)
export const decodeWorldErrorEnvelope = S.decodeUnknownSync(WorldErrorEnvelope)

export const sanitizeWorldCharacterId = (input: string): string => {
  const normalized = input.trim().toLowerCase().replace(/[^a-z0-9_-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^[-_]+|[-_]+$/g, "")
  return normalized.length > 0 ? normalized.slice(0, 48) : "guest"
}

export const worldAvatarRefForCharacter = (
  accountRef: string,
  characterId: string,
): string => `avatar.${sanitizeWorldCharacterId(accountRef)}.${sanitizeWorldCharacterId(characterId)}`

export const worldRowKey = (row: WorldRow): string => {
  switch (row.kind) {
    case "world_region":
      return row.regionRef
    case "pylon_station":
      return row.pylonRef
    case "gateway_station":
      return row.gatewayRef
    case "agent_avatar":
    case "avatar_position":
      return row.avatarRef
    case "local_chat_message":
      return row.messageRef
    case "chat_bubble":
      return row.bubbleRef
    case "local_emote":
      return row.emoteRef
    case "agent_intent":
      return row.intentRef
    case "training_run":
      return row.runRef
    case "run_entity":
      return row.entityRef
    case "world_edge":
      return row.edgeRef
    case "proof_ref":
      return row.proofRef
    case "settlement_ref":
      return row.settlementRef
    case "world_event":
      return row.eventRef
    case "projection_cursor":
      return row.cursorRef
    case "bridge_health":
      return row.bridgeRef
  }
}

export const deterministicWorldEventRef = (
  sourceRef: string,
  eventKind: string,
  sequence: number,
): string =>
  `world_event.${sanitizeWorldCharacterId(sourceRef)}.${sanitizeWorldCharacterId(eventKind)}.${sequence}`

export const isBrowserWorldCommand = (command: WorldCommandName): boolean =>
  browserWorldCommandNames.includes(command)

export const isServiceWorldCommand = (command: WorldCommandName): boolean =>
  serviceWorldCommandNames.includes(command)

export const assertWorldCommandActorAllowed = (
  envelope: WorldCommandEnvelope,
): WorldCommandEnvelope => {
  if (isServiceWorldCommand(envelope.command) && envelope.actorClass !== "service") {
    throw new Error("World command actor class cannot invoke service-only command")
  }
  if (isBrowserWorldCommand(envelope.command) && envelope.actorClass === "service") {
    throw new Error("World service actor cannot invoke browser interaction command")
  }
  return envelope
}

export const worldVectorInsideBounds = (
  position: WorldVector3,
  bounds: WorldRegionBounds,
): boolean =>
  position.x >= bounds.min.x &&
  position.x <= bounds.max.x &&
  position.y >= bounds.min.y &&
  position.y <= bounds.max.y &&
  position.z >= bounds.min.z &&
  position.z <= bounds.max.z

const unsafePublicTextPatterns = [
  /raw_prompt/i,
  /raw_shell_log/i,
  /provider_payload/i,
  /secret/i,
  /\/Users\//,
  /sk-[a-z0-9_-]+/i,
]

export const worldTextHasUnsafeMaterial = (text: string): boolean =>
  unsafePublicTextPatterns.some((pattern) => pattern.test(text))

export const assertWorldPublicSafety = (row: WorldRow): WorldRow => {
  if (!row.safety.publicProjectionAllowed) {
    throw new Error("World row is not public projection safe")
  }
  if (row.safety.sourceRefs.length === 0) {
    throw new Error("World row requires at least one public source ref")
  }
  const asJson = JSON.stringify(row)
  if (worldTextHasUnsafeMaterial(asJson)) {
    throw new Error("World row contains raw/private material")
  }
  return row
}

export const sparseWorldPatchChangesOnly = (
  patch: Readonly<Record<string, unknown>>,
): boolean =>
  Object.values(patch).every((value) => value !== undefined)
