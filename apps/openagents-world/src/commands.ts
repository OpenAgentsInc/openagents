import { Effect } from "effect"

import {
  WORLD_DELTA_SCHEMA_VERSION,
  assertWorldCommandActorAllowed,
  decodeWorldCommandEnvelope,
  decodeWorldCommandReceipt,
  decodeWorldDelta,
  decodeWorldErrorEnvelope,
  decodeWorldRow,
  worldAvatarRefForCharacter,
  type WorldCommandEnvelope,
  type WorldCommandName,
  type WorldCommandReceipt,
  type WorldDelta,
  type WorldRef,
  type WorldRow,
} from "@openagentsinc/world-contract"

import {
  cursorForSequence,
  makeDiagnostic,
  stableWorldRef,
} from "./protocol"

export type WorldHotAvatar = Readonly<{
  avatarRef: string
  actorRef: string
  characterId: string
  label: string
  regionRef: string
  joinedAt: string
  lastSeq: number
  lastPoseAt: string | null
  position: Readonly<{ x: number; y: number; z: number }> | null
}>

export type WorldHotState = Readonly<{
  regionRef: string
  sequence: number
  minReplaySeq: number
  avatars: Readonly<Record<string, WorldHotAvatar>>
  focusByActor: Readonly<Record<string, string>>
  expiringRefs: Readonly<Record<string, WorldExpiringRef>>
  lastChatAtByActor: Readonly<Record<string, string>>
  lastEmoteAtByActor: Readonly<Record<string, string>>
  lastIntentAtByActor: Readonly<Record<string, string>>
}>

export type WorldExpiringRef = Readonly<{
  ref: string
  kind: "avatar" | "chat" | "emote" | "focus" | "intent"
  expiresAt: string
}>

export type WorldCommandApplyResult = Readonly<{
  state: WorldHotState
  receipt: WorldCommandReceipt
  delta: WorldDelta
}>

type CommandFailure = Readonly<{
  reason: string
  tag?: "validation" | "auth" | "command"
  retryable?: boolean
}>

const poseCadenceMs = 50
const chatCadenceMs = 1000
const emoteCadenceMs = 500
const intentCadenceMs = 250
const maxPoseSpeedPerSecond = 24
const maxMessageLength = 240
const maxIntentLength = 120

export const makeEmptyHotState = (regionRef: string): WorldHotState => ({
  regionRef,
  sequence: 0,
  minReplaySeq: 0,
  avatars: {},
  focusByActor: {},
  expiringRefs: {},
  lastChatAtByActor: {},
  lastEmoteAtByActor: {},
  lastIntentAtByActor: {},
})

export const applyWorldCommand = (
  state: WorldHotState,
  input: unknown,
  observedAt: string,
): Effect.Effect<WorldCommandApplyResult> =>
  Effect.sync(() => {
    const envelope = decodeWorldCommandEnvelope(input)
    try {
      assertWorldCommandActorAllowed(envelope)
    } catch (error) {
      return commandOutcomeToResult(state, envelope, {
        ok: false,
        failure: {
          tag: "auth",
          reason: error instanceof Error ? error.message : String(error),
        },
      }, observedAt)
    }
    const outcome = evaluateWorldCommand(state, envelope, observedAt)
    return commandOutcomeToResult(state, envelope, outcome, observedAt)
  })

const evaluateWorldCommand = (
  state: WorldHotState,
  envelope: WorldCommandEnvelope,
  observedAt: string,
):
  | { ok: true; state: WorldHotState; rows: ReadonlyArray<WorldRow>; deletedRefs?: ReadonlyArray<string> }
  | { ok: false; failure: CommandFailure } => {
  if (envelope.actorClass === "service") {
    return {
      ok: false,
      failure: {
        tag: "auth",
        reason: "Service actors cannot use browser hot-presence commands.",
      },
    }
  }

  if (state.regionRef !== (envelope.regionRef ?? state.regionRef)) {
    return {
      ok: false,
      failure: {
        reason: "Command region does not match this Region Durable Object.",
      },
    }
  }

  switch (envelope.command) {
    case "join_region":
      return joinRegion(state, envelope, observedAt)
    case "leave_region":
      return leaveRegion(state, envelope)
    case "set_avatar_position":
      return setAvatarPosition(state, envelope, observedAt)
    case "focus_pylon":
      return focusPylon(state, envelope, observedAt)
    case "clear_pylon_focus":
      return clearPylonFocus(state, envelope)
    case "send_local_message":
    case "send_pylon_message":
      return sendMessage(state, envelope, observedAt)
    case "send_emote":
      return sendEmote(state, envelope, observedAt)
    case "set_agent_intent":
      return setAgentIntent(state, envelope, observedAt)
    default:
      return {
        ok: false,
        failure: {
          tag: "auth",
          reason: `${envelope.command} is service-only and cannot be written by browser actors.`,
        },
      }
  }
}

const commandOutcomeToResult = (
  previousState: WorldHotState,
  envelope: WorldCommandEnvelope,
  outcome:
    | { ok: true; state: WorldHotState; rows: ReadonlyArray<WorldRow>; deletedRefs?: ReadonlyArray<string> }
    | { ok: false; failure: CommandFailure },
  observedAt: string,
): WorldCommandApplyResult => {
  const acceptedSeq = envelope.seq
  const sequence = outcome.ok ? previousState.sequence + 1 : previousState.sequence
  const cursor = cursorForSequence(previousState.regionRef, sequence)
  const receipt = decodeWorldCommandReceipt({
    receiptRef: stableWorldRef("receipt.world_command", `${envelope.commandRef}:${observedAt}`),
    commandRef: envelope.commandRef,
    command: envelope.command,
    status: outcome.ok ? "applied" : "rejected",
    actorClass: envelope.actorClass,
    ...(acceptedSeq === undefined ? {} : { acceptedSeq }),
    ...(outcome.ok && acceptedSeq !== undefined ? { appliedSeq: acceptedSeq } : {}),
    ...(!outcome.ok && acceptedSeq !== undefined ? { rejectedSeq: acceptedSeq } : {}),
    observedAt,
    changedRefs: outcome.ok ? outcome.rows.map(rowRef) : [],
    ...(!outcome.ok
      ? {
          error: decodeWorldErrorEnvelope({
            tag: outcome.failure.tag ?? "command",
            message: outcome.failure.reason,
            retryable: outcome.failure.retryable ?? false,
            publicSafe: true,
            sourceRefs: [envelope.commandRef],
          }),
        }
      : {}),
  })
  const nextState = outcome.ok
    ? {
        ...outcome.state,
        sequence,
        minReplaySeq: Math.max(0, sequence - 256),
      }
    : previousState

  const delta = decodeWorldDelta({
    schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
    deltaRef: stableWorldRef("delta.world.command", `${envelope.commandRef}:${cursor}`),
    kind: outcome.ok && outcome.deletedRefs !== undefined && outcome.deletedRefs.length > 0
      ? "delete"
      : outcome.ok
        ? "update"
        : "diagnostic",
    regionRef: previousState.regionRef,
    cursor,
    generatedAt: observedAt,
    ...(outcome.ok ? { rows: outcome.rows } : {}),
    ...(outcome.ok && outcome.deletedRefs !== undefined ? { deletedRefs: outcome.deletedRefs } : {}),
    receipt,
    ...(!outcome.ok
      ? {
          diagnostic: makeDiagnostic({
            tag: outcome.failure.tag ?? "command",
            severity: "warn",
            message: outcome.failure.reason,
            observedAt,
            sourceRefs: [envelope.commandRef],
          }),
        }
      : {}),
  })

  return { state: nextState, receipt, delta }
}

const joinRegion = (
  state: WorldHotState,
  envelope: WorldCommandEnvelope,
  observedAt: string,
) => {
  const payload = objectPayload(envelope.payload)
  const characterId = stringField(payload, "characterId") ?? sanitizeCommandPart(envelope.actorRef)
  const avatarRef = stringField(payload, "avatarRef") ?? worldAvatarRefForCharacter(envelope.actorRef, characterId)
  const label = plainText(stringField(payload, "label") ?? characterId, 48)
  const avatar: WorldHotAvatar = {
    avatarRef,
    actorRef: envelope.actorRef,
    characterId,
    label,
    regionRef: state.regionRef,
    joinedAt: observedAt,
    lastSeq: envelope.seq ?? 0,
    lastPoseAt: null,
    position: null,
  }
  const row = decodeWorldRow({
    kind: "agent_avatar",
    avatarRef,
    accountRef: envelope.actorRef,
    characterId,
    regionRef: state.regionRef,
    label,
    avatarKind: "guest",
    updatedAt: observedAt,
    safety: publicSafety(envelope.commandRef),
  })
  return {
    ok: true as const,
    state: {
      ...state,
      avatars: { ...state.avatars, [avatarRef]: avatar },
      expiringRefs: {
        ...state.expiringRefs,
        [avatarRef]: expires(avatarRef, "avatar", observedAt, 30000),
      },
    },
    rows: [row],
  }
}

const leaveRegion = (state: WorldHotState, envelope: WorldCommandEnvelope) => {
  const payload = objectPayload(envelope.payload)
  const avatarRef = stringField(payload, "avatarRef") ?? avatarRefForActor(state, envelope.actorRef)
  if (avatarRef === null) {
    return reject("Actor has no joined avatar in this region.")
  }
  const { [avatarRef]: _removed, ...avatars } = state.avatars
  return {
    ok: true as const,
    state: {
      ...state,
      avatars,
      expiringRefs: omitKey(state.expiringRefs, avatarRef),
      focusByActor: omitKey(state.focusByActor, envelope.actorRef),
    },
    rows: [],
    deletedRefs: [avatarRef],
  }
}

const setAvatarPosition = (
  state: WorldHotState,
  envelope: WorldCommandEnvelope,
  observedAt: string,
) => {
  const payload = objectPayload(envelope.payload)
  const avatarRef = stringField(payload, "avatarRef") ?? avatarRefForActor(state, envelope.actorRef)
  if (avatarRef === null) {
    return reject("Actor must join the region before moving.")
  }
  const avatar = state.avatars[avatarRef]
  if (avatar === undefined) {
    return reject("Avatar is not joined in this region.")
  }
  const seqFailure = validateSeq(avatar.lastSeq, envelope.seq)
  if (seqFailure !== null) {
    return reject(seqFailure)
  }
  const position = vectorPayload(payload.position)
  if (position === null || !insideStarterBounds(position)) {
    return reject("Avatar position is outside the starter region bounds.")
  }
  if (avatar.lastPoseAt !== null && avatar.position !== null) {
    const elapsedMs = Date.parse(observedAt) - Date.parse(avatar.lastPoseAt)
    if (elapsedMs >= 0 && elapsedMs < poseCadenceMs) {
      return reject("Avatar pose cadence exceeded.")
    }
    if (elapsedMs > 0) {
      const speed = distance(avatar.position, position) / (elapsedMs / 1000)
      if (speed > maxPoseSpeedPerSecond) {
        return reject("Avatar pose velocity exceeded.")
      }
    }
  }
  const row = decodeWorldRow({
    kind: "avatar_position",
    avatarRef,
    regionRef: state.regionRef,
    position,
    rotationY: finiteNumber(payload.rotationY) ?? 0,
    animation: animation(payload.animation),
    seq: envelope.seq,
    observedAt,
    safety: publicSafety(envelope.commandRef),
  })
  return {
    ok: true as const,
    state: {
      ...state,
      avatars: {
        ...state.avatars,
        [avatarRef]: {
          ...avatar,
          lastSeq: envelope.seq ?? avatar.lastSeq,
          lastPoseAt: observedAt,
          position,
        },
      },
      expiringRefs: {
        ...state.expiringRefs,
        [avatarRef]: expires(avatarRef, "avatar", observedAt, 30000),
      },
    },
    rows: [row],
  }
}

const focusPylon = (
  state: WorldHotState,
  envelope: WorldCommandEnvelope,
  observedAt: string,
) => {
  const pylonRef = stringField(objectPayload(envelope.payload), "pylonRef")
  if (pylonRef === null || !pylonRef.startsWith("pylon.")) {
    return reject("Pylon focus requires a visible pylon ref.")
  }
  const intentRef = stableWorldRef("intent.world.focus", `${envelope.actorRef}:${pylonRef}`)
  const row = decodeWorldRow({
    kind: "agent_intent",
    intentRef,
    avatarRef: avatarRefForActor(state, envelope.actorRef) ?? envelope.actorRef,
    regionRef: state.regionRef,
    intent: "focus_pylon",
    targetRef: pylonRef,
    createdAt: observedAt,
    expiresAt: new Date(Date.parse(observedAt) + 30000).toISOString(),
    safety: publicSafety(envelope.commandRef),
  })
  return {
    ok: true as const,
    state: {
      ...state,
      focusByActor: { ...state.focusByActor, [envelope.actorRef]: pylonRef },
      expiringRefs: {
        ...state.expiringRefs,
        [intentRef]: expires(intentRef, "focus", observedAt, 30000),
      },
    },
    rows: [row],
  }
}

const clearPylonFocus = (state: WorldHotState, envelope: WorldCommandEnvelope) => ({
  ok: true as const,
  state: {
    ...state,
    focusByActor: omitKey(state.focusByActor, envelope.actorRef),
    expiringRefs: omitKey(state.expiringRefs, stableWorldRef("intent.world.focus", envelope.actorRef)),
  },
  rows: [],
  deletedRefs: [stableWorldRef("intent.world.focus", envelope.actorRef)],
})

const sendMessage = (
  state: WorldHotState,
  envelope: WorldCommandEnvelope,
  observedAt: string,
) => {
  const cadenceFailure = validateCadence(state.lastChatAtByActor[envelope.actorRef], observedAt, chatCadenceMs, "Chat")
  if (cadenceFailure !== null) {
    return reject(cadenceFailure)
  }
  const text = plainText(stringField(objectPayload(envelope.payload), "text") ?? "", maxMessageLength)
  if (text.length === 0) {
    return reject("Chat message must be non-empty plain text.")
  }
  const messageRef = stableWorldRef("message.world.local", `${envelope.commandRef}:${text}`)
  const row = decodeWorldRow({
    kind: "local_chat_message",
    messageRef,
    regionRef: state.regionRef,
    avatarRef: avatarRefForActor(state, envelope.actorRef) ?? envelope.actorRef,
    channel: envelope.command === "send_pylon_message" ? "pylon" : "local",
    text,
    moderationState: "visible",
    createdAt: observedAt,
    expiresAt: new Date(Date.parse(observedAt) + 60000).toISOString(),
    safety: publicSafety(envelope.commandRef),
  })
  return {
    ok: true as const,
    state: {
      ...state,
      lastChatAtByActor: { ...state.lastChatAtByActor, [envelope.actorRef]: observedAt },
      expiringRefs: {
        ...state.expiringRefs,
        [messageRef]: expires(messageRef, "chat", observedAt, 60000),
      },
    },
    rows: [row],
  }
}

const sendEmote = (
  state: WorldHotState,
  envelope: WorldCommandEnvelope,
  observedAt: string,
) => {
  const cadenceFailure = validateCadence(state.lastEmoteAtByActor[envelope.actorRef], observedAt, emoteCadenceMs, "Emote")
  if (cadenceFailure !== null) {
    return reject(cadenceFailure)
  }
  const emote = plainText(stringField(objectPayload(envelope.payload), "emote") ?? "", 32)
  if (emote.length === 0) {
    return reject("Emote must be non-empty plain text.")
  }
  const emoteRef = stableWorldRef("emote.world.local", `${envelope.commandRef}:${emote}`)
  const row = decodeWorldRow({
    kind: "local_emote",
    emoteRef,
    regionRef: state.regionRef,
    avatarRef: avatarRefForActor(state, envelope.actorRef) ?? envelope.actorRef,
    emote,
    createdAt: observedAt,
    expiresAt: new Date(Date.parse(observedAt) + 10000).toISOString(),
    safety: publicSafety(envelope.commandRef),
  })
  return {
    ok: true as const,
    state: {
      ...state,
      lastEmoteAtByActor: { ...state.lastEmoteAtByActor, [envelope.actorRef]: observedAt },
      expiringRefs: {
        ...state.expiringRefs,
        [emoteRef]: expires(emoteRef, "emote", observedAt, 10000),
      },
    },
    rows: [row],
  }
}

const setAgentIntent = (
  state: WorldHotState,
  envelope: WorldCommandEnvelope,
  observedAt: string,
) => {
  const cadenceFailure = validateCadence(state.lastIntentAtByActor[envelope.actorRef], observedAt, intentCadenceMs, "Intent")
  if (cadenceFailure !== null) {
    return reject(cadenceFailure)
  }
  const text = plainText(stringField(objectPayload(envelope.payload), "text") ?? "", maxIntentLength)
  if (text.length === 0) {
    return reject("Intent must be non-empty plain text.")
  }
  const intentRef = stableWorldRef("intent.world.agent", `${envelope.actorRef}:${text}`)
  const row = decodeWorldRow({
    kind: "agent_intent",
    intentRef,
    avatarRef: avatarRefForActor(state, envelope.actorRef) ?? envelope.actorRef,
    regionRef: state.regionRef,
    intent: text,
    createdAt: observedAt,
    expiresAt: new Date(Date.parse(observedAt) + 15000).toISOString(),
    safety: publicSafety(envelope.commandRef),
  })
  return {
    ok: true as const,
    state: {
      ...state,
      lastIntentAtByActor: { ...state.lastIntentAtByActor, [envelope.actorRef]: observedAt },
      expiringRefs: {
        ...state.expiringRefs,
        [intentRef]: expires(intentRef, "intent", observedAt, 15000),
      },
    },
    rows: [row],
  }
}

const reject = (reason: string) => ({
  ok: false as const,
  failure: { reason },
})

const rowRef = (row: WorldRow): WorldRef => {
  switch (row.kind) {
    case "agent_avatar":
    case "avatar_position":
      return row.avatarRef as unknown as WorldRef
    case "local_chat_message":
      return row.messageRef as WorldRef
    case "local_emote":
      return row.emoteRef as WorldRef
    case "agent_intent":
      return row.intentRef as WorldRef
    default:
      return stableWorldRef("row.world", JSON.stringify(row)) as WorldRef
  }
}

const objectPayload = (payload: unknown): Record<string, unknown> =>
  typeof payload === "object" && payload !== null && !Array.isArray(payload)
    ? payload as Record<string, unknown>
    : {}

const stringField = (payload: Record<string, unknown>, key: string): string | null =>
  typeof payload[key] === "string" && payload[key].trim().length > 0
    ? payload[key].trim()
    : null

const finiteNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

const vectorPayload = (value: unknown): { x: number; y: number; z: number } | null => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null
  }
  const payload = value as Record<string, unknown>
  const x = finiteNumber(payload.x)
  const y = finiteNumber(payload.y)
  const z = finiteNumber(payload.z)
  return x === null || y === null || z === null ? null : { x, y, z }
}

const insideStarterBounds = (value: { x: number; y: number; z: number }) =>
  value.x >= -128 && value.x <= 128 &&
  value.y >= 0 && value.y <= 24 &&
  value.z >= -128 && value.z <= 128

const animation = (value: unknown) =>
  value === "idle" || value === "walk" || value === "run" || value === "emote"
    ? value
    : "unknown"

const plainText = (value: string, maxLength: number) =>
  value
    .replace(/[\u0000-\u001f\u007f<>]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, maxLength)

const sanitizeCommandPart = (value: string) =>
  value.toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "") || "guest"

const publicSafety = (sourceRef: string) => ({
  publicProjectionAllowed: true,
  sourceRefs: [sourceRef],
  blockerRefs: [],
  caveatRefs: [],
})

const avatarRefForActor = (state: WorldHotState, actorRef: string): string | null =>
  Object.values(state.avatars).find(avatar => avatar.actorRef === actorRef)?.avatarRef ?? null

const omitKey = <Value>(
  input: Readonly<Record<string, Value>>,
  key: string,
): Readonly<Record<string, Value>> => {
  const { [key]: _removed, ...rest } = input
  return rest
}

const validateSeq = (lastSeq: number, seq: number | undefined): string | null =>
  seq === undefined || seq > lastSeq ? null : "Command sequence is stale or duplicate."

const validateCadence = (
  previousAt: string | undefined,
  observedAt: string,
  minMs: number,
  label: string,
): string | null =>
  previousAt !== undefined && Date.parse(observedAt) - Date.parse(previousAt) < minMs
    ? `${label} cadence exceeded.`
    : null

const distance = (
  a: Readonly<{ x: number; y: number; z: number }>,
  b: Readonly<{ x: number; y: number; z: number }>,
) =>
  Math.hypot(a.x - b.x, a.y - b.y, a.z - b.z)

const expires = (
  ref: string,
  kind: WorldExpiringRef["kind"],
  observedAt: string,
  ttlMs: number,
): WorldExpiringRef => ({
  ref,
  kind,
  expiresAt: new Date(Date.parse(observedAt) + ttlMs).toISOString(),
})

export const commandNamesImplementedInRegionDo: ReadonlyArray<WorldCommandName> = [
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

export const commandDeltaFrame = (delta: WorldDelta) => ({
  frameKind: delta.kind === "diagnostic" ? "diagnostic" : "delta",
  delta,
})
