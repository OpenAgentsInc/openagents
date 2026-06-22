import { Data, Effect } from "effect"

import {
  WORLD_READ_MODEL_SCHEMA_VERSION,
  decodeWorldCommandReceipt,
  decodeWorldReadModel,
  sparseWorldPatchChangesOnly,
  worldRowKey,
  type WorldCommandEnvelope,
  type WorldCommandReceipt,
  type WorldDelta,
  type WorldDiagnostic,
  type WorldReadModel,
  type WorldRef,
  type WorldRow,
  type WorldSubscriptionPlan,
} from "@openagentsinc/world-contract"

export type ClientWorld = WorldReadModel
export type WorldClientPhase = "connect" | "subscribe" | "command" | "delta" | "reconnect" | "disconnect"

export class WorldClientError extends Data.TaggedError("WorldClientError")<{
  readonly phase: WorldClientPhase
  readonly reason: string
  readonly retryable: boolean
  readonly sourceRefs: ReadonlyArray<string>
}> {}

export type WorldClientConnectRequest = Readonly<{
  regionRef?: string
  scope?: WorldSubscriptionPlan["scope"]
  runRef?: string
  selectedEntityRef?: string
  selectedRefs?: ReadonlyArray<string>
  resumeCursor?: string
}>

export type WorldClientConnectResult = Readonly<{
  regionRef: string
  socketUrl: string
  subscriptionPlan: WorldSubscriptionPlan
  deltas?: ReadonlyArray<WorldDelta>
}>

export type WorldClientCommandAck = Readonly<{
  receipt: WorldCommandReceipt
  status: WorldCommandReceipt["status"]
  commandRef: string
  command: WorldCommandEnvelope["command"]
  acceptedSeq?: number
  appliedSeq?: number
  rejectedSeq?: number
}>

export type WorldClientState = Readonly<{
  connected: boolean
  regionRef: string
  socketUrl: string | null
  readModel: ClientWorld
  subscriptionPlan: WorldSubscriptionPlan | null
  selectedRefs: ReadonlyArray<string>
  interestTierByRef: Readonly<Record<string, "near" | "far">>
  diagnostics: ReadonlyArray<WorldDiagnostic>
  commandAcks: Readonly<Record<string, WorldClientCommandAck>>
}>

export type WorldClientTransport = Readonly<{
  connect: (request: WorldClientConnectRequest) => Effect.Effect<WorldClientConnectResult, WorldClientError>
  subscribe?: (request: WorldClientConnectRequest) => Effect.Effect<WorldClientConnectResult, WorldClientError>
  command: (command: WorldCommandEnvelope) => Effect.Effect<WorldDelta, WorldClientError>
  disconnect: () => Effect.Effect<void, WorldClientError>
}>

export type WorldClient = Readonly<{
  connect: (request?: WorldClientConnectRequest) => Effect.Effect<WorldClientState, WorldClientError>
  subscribe: (request?: WorldClientConnectRequest) => Effect.Effect<WorldSubscriptionPlan, WorldClientError>
  callCommand: (command: WorldCommandEnvelope) => Effect.Effect<WorldClientCommandAck, WorldClientError>
  applyDelta: (delta: WorldDelta) => Effect.Effect<WorldClientState, WorldClientError>
  reconnect: () => Effect.Effect<WorldClientState, WorldClientError>
  disconnect: () => Effect.Effect<void, WorldClientError>
  diagnostics: () => Effect.Effect<ReadonlyArray<WorldDiagnostic>>
  readModel: () => Effect.Effect<ClientWorld>
  state: () => Effect.Effect<WorldClientState>
}>

export const makeEmptyClientWorld = (
  regionRef: string,
  generatedAt: string,
  cursor = `cursor.${regionRef}.0`,
): ClientWorld =>
  decodeWorldReadModel({
    schemaVersion: WORLD_READ_MODEL_SCHEMA_VERSION,
    regionRef,
    cursor,
    generatedAt,
    regions: {},
    pylons: {},
    avatars: {},
    positions: {},
    chatMessages: {},
    chatBubbles: {},
    emotes: {},
    intents: {},
    runs: {},
    entities: {},
    edges: {},
    proofRefs: {},
    settlementRefs: {},
    events: {},
    diagnostics: [],
  })

export const createWorldClient = (input: {
  readonly transport: WorldClientTransport
  readonly initialRegionRef?: string
  readonly now?: () => string
}): WorldClient => {
  const now = input.now ?? (() => new Date().toISOString())
  let state = initialState(input.initialRegionRef ?? "region.run.tassadar.executor.20260615.street", now())

  const setState = (next: WorldClientState): WorldClientState => {
    state = next
    return state
  }

  const applyConnectResult = (
    result: WorldClientConnectResult,
    phase: WorldClientPhase,
  ): Effect.Effect<WorldClientState, WorldClientError> =>
    Effect.gen(function* () {
      const base = setState({
        ...state,
        connected: true,
        regionRef: result.regionRef,
        socketUrl: result.socketUrl,
        subscriptionPlan: result.subscriptionPlan,
        selectedRefs: result.subscriptionPlan.interest.selectedRefs.map(String),
        readModel: state.readModel.regionRef === result.regionRef
          ? state.readModel
          : makeEmptyClientWorld(result.regionRef, now()),
      })
      let next = base
      for (const delta of result.deltas ?? []) {
        next = applyDeltaToState(next, delta)
      }
      return setState(next)
    }).pipe(Effect.mapError(error => normalizeClientError(error, phase)))

  return {
    connect: (request = {}) =>
      Effect.gen(function* () {
        const result = yield* input.transport.connect(request)
        return yield* applyConnectResult(result, "connect")
      }),
    subscribe: (request = {}) =>
      Effect.gen(function* () {
        const result = yield* (input.transport.subscribe ?? input.transport.connect)({
          ...request,
          resumeCursor: request.resumeCursor ?? state.readModel.cursor,
        })
        const next = yield* applyConnectResult(result, "subscribe")
        if (next.subscriptionPlan === null) {
          return yield* new WorldClientError({
            phase: "subscribe",
            reason: "World subscription did not include a server-approved plan.",
            retryable: true,
            sourceRefs: ["world-client.subscribe"],
          })
        }
        return next.subscriptionPlan
      }),
    callCommand: (command) =>
      Effect.gen(function* () {
        const delta = yield* input.transport.command(command)
        const next = applyDeltaToState(state, delta)
        setState(next)
        const receipt = delta.receipt ?? decodeWorldCommandReceipt({
          receiptRef: `receipt.world_client.missing.${command.commandRef}`,
          commandRef: command.commandRef,
          command: command.command,
          status: "rejected",
          actorClass: command.actorClass,
          observedAt: delta.generatedAt,
          changedRefs: [],
          error: {
            tag: "command",
            message: "World command delta did not include a receipt.",
            retryable: true,
            publicSafe: true,
            sourceRefs: [command.commandRef],
          },
        })
        const ack = commandAckFromReceipt(receipt)
        setState({
          ...state,
          commandAcks: {
            ...state.commandAcks,
            [ack.commandRef]: ack,
          },
        })
        return ack
      }).pipe(Effect.mapError(error => normalizeClientError(error, "command"))),
    applyDelta: (delta) =>
      Effect.sync(() => setState(applyDeltaToState(state, delta))).pipe(
        Effect.mapError(error => normalizeClientError(error, "delta")),
      ),
    reconnect: () =>
      Effect.gen(function* () {
        const result = yield* input.transport.connect({
          regionRef: state.regionRef,
          resumeCursor: state.readModel.cursor,
          selectedRefs: state.selectedRefs,
        })
        return yield* applyConnectResult(result, "reconnect")
      }),
    disconnect: () =>
      Effect.gen(function* () {
        yield* input.transport.disconnect()
        setState({
          ...state,
          connected: false,
          socketUrl: null,
        })
      }).pipe(Effect.mapError(error => normalizeClientError(error, "disconnect"))),
    diagnostics: () => Effect.succeed(state.diagnostics),
    readModel: () => Effect.succeed(state.readModel),
    state: () => Effect.succeed(state),
  }
}

export const applyDeltaToReadModel = (
  readModel: ClientWorld,
  delta: WorldDelta,
): ClientWorld => {
  if ((delta.patches ?? []).some(patch =>
    typeof patch === "object" &&
    patch !== null &&
    !sparseWorldPatchChangesOnly(patch as Record<string, unknown>)
  )) {
    throw new WorldClientError({
      phase: "delta",
      reason: "Sparse world patches may not contain undefined fields.",
      retryable: false,
      sourceRefs: [delta.deltaRef],
    })
  }

  const next = cloneReadModel(readModel, delta)

  for (const row of delta.rows ?? []) {
    upsertReadModelRow(next, row)
  }

  for (const patch of delta.patches ?? []) {
    applyPatch(next, patch)
  }

  for (const ref of delta.deletedRefs ?? []) {
    pruneReadModelRef(next, String(ref))
  }

  if (delta.diagnostic !== undefined) {
    next.diagnostics.push(delta.diagnostic)
  }

  return decodeWorldReadModel(next)
}

export const applyDeltaToState = (
  state: WorldClientState,
  delta: WorldDelta,
): WorldClientState => {
  const deletedRefs = new Set((delta.deletedRefs ?? []).map(String))
  return {
    ...state,
    regionRef: delta.regionRef,
    readModel: applyDeltaToReadModel(state.readModel, delta),
    selectedRefs: state.selectedRefs.filter(ref => !deletedRefs.has(ref)),
    interestTierByRef: Object.fromEntries(
      Object.entries(state.interestTierByRef).filter(([ref]) => !deletedRefs.has(ref)),
    ),
    diagnostics: [
      ...state.diagnostics,
      ...(delta.diagnostic === undefined ? [] : [delta.diagnostic]),
    ],
  }
}

export const commandAckFromReceipt = (
  receipt: WorldCommandReceipt,
): WorldClientCommandAck => ({
  receipt,
  status: receipt.status,
  commandRef: receipt.commandRef,
  command: receipt.command,
  ...(receipt.acceptedSeq === undefined ? {} : { acceptedSeq: Number(receipt.acceptedSeq) }),
  ...(receipt.appliedSeq === undefined ? {} : { appliedSeq: Number(receipt.appliedSeq) }),
  ...(receipt.rejectedSeq === undefined ? {} : { rejectedSeq: Number(receipt.rejectedSeq) }),
})

const initialState = (regionRef: string, generatedAt: string): WorldClientState => ({
  connected: false,
  regionRef,
  socketUrl: null,
  readModel: makeEmptyClientWorld(regionRef, generatedAt),
  subscriptionPlan: null,
  selectedRefs: [],
  interestTierByRef: {},
  diagnostics: [],
  commandAcks: {},
})

const normalizeClientError = (
  error: unknown,
  phase: WorldClientPhase,
): WorldClientError =>
  error instanceof WorldClientError
    ? error
    : new WorldClientError({
        phase,
        reason: error instanceof Error ? error.message : "World client operation failed.",
        retryable: true,
        sourceRefs: ["world-client"],
      })

type MutableReadModel = {
  schemaVersion: ClientWorld["schemaVersion"]
  regionRef: ClientWorld["regionRef"]
  cursor: ClientWorld["cursor"]
  generatedAt: ClientWorld["generatedAt"]
  regions: Record<string, ClientWorld["regions"][string]>
  pylons: Record<string, ClientWorld["pylons"][string]>
  avatars: Record<string, ClientWorld["avatars"][string]>
  positions: Record<string, ClientWorld["positions"][string]>
  chatMessages: Record<string, ClientWorld["chatMessages"][string]>
  chatBubbles: Record<string, ClientWorld["chatBubbles"][string]>
  emotes: Record<string, ClientWorld["emotes"][string]>
  intents: Record<string, ClientWorld["intents"][string]>
  runs: Record<string, ClientWorld["runs"][string]>
  entities: Record<string, ClientWorld["entities"][string]>
  edges: Record<string, ClientWorld["edges"][string]>
  proofRefs: Record<string, ClientWorld["proofRefs"][string]>
  settlementRefs: Record<string, ClientWorld["settlementRefs"][string]>
  events: Record<string, ClientWorld["events"][string]>
  diagnostics: Array<ClientWorld["diagnostics"][number]>
}

const cloneReadModel = (
  readModel: ClientWorld,
  delta: WorldDelta,
): MutableReadModel => ({
  ...readModel,
  regionRef: delta.regionRef,
  cursor: delta.cursor,
  generatedAt: delta.generatedAt,
  regions: { ...readModel.regions },
  pylons: { ...readModel.pylons },
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
})

const upsertReadModelRow = (readModel: MutableReadModel, row: WorldRow): void => {
  const key = worldRowKey(row)
  switch (row.kind) {
    case "world_region":
      readModel.regions[key] = row
      break
    case "pylon_station":
      readModel.pylons[key] = row
      break
    case "agent_avatar":
      readModel.avatars[key] = row
      break
    case "avatar_position":
      readModel.positions[key] = row
      break
    case "local_chat_message":
      readModel.chatMessages[key] = row
      break
    case "chat_bubble":
      readModel.chatBubbles[key] = row
      break
    case "local_emote":
      readModel.emotes[key] = row
      break
    case "agent_intent":
      readModel.intents[key] = row
      break
    case "training_run":
      readModel.runs[key] = row
      break
    case "run_entity":
      readModel.entities[key] = row
      break
    case "world_edge":
      readModel.edges[key] = row
      break
    case "proof_ref":
      readModel.proofRefs[key] = row
      break
    case "settlement_ref":
      readModel.settlementRefs[key] = row
      break
    case "world_event":
      readModel.events[key] = row
      break
    case "projection_cursor":
    case "bridge_health":
      break
  }
}

const pruneReadModelRef = (readModel: MutableReadModel, ref: string): void => {
  delete readModel.avatars[ref]
  delete readModel.positions[ref]
  delete readModel.chatMessages[ref]
  delete readModel.chatBubbles[ref]
  delete readModel.emotes[ref]
  delete readModel.intents[ref]
  delete readModel.entities[ref]
  delete readModel.edges[ref]
  delete readModel.proofRefs[ref]
  delete readModel.settlementRefs[ref]
  delete readModel.events[ref]
}

const applyPatch = (readModel: MutableReadModel, patch: unknown): void => {
  if (typeof patch !== "object" || patch === null || Array.isArray(patch)) {
    return
  }
  const fields = patch as Record<string, unknown>
  if (fields.movement === "settled" && typeof fields.ref === "string") {
    const position = readModel.positions[fields.ref]
    if (position !== undefined) {
      readModel.positions[fields.ref] = {
        ...position,
        animation: "idle",
      }
    }
  }
}

export type { WorldDiagnostic, WorldRef }
