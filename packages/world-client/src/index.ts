import { Data, Effect } from "effect"

import {
  WORLD_READ_MODEL_SCHEMA_VERSION,
  WORLD_CONTRACT_SCHEMA_VERSION,
  decodeWorldDelta,
  decodeWorldRow,
  decodeWorldSubscriptionPlan,
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
  characterId?: string
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

export type WorldTransportFrame =
  | Readonly<{
      frameKind: "snapshot"
      delta: WorldDelta
      readModel: WorldReadModel
    }>
  | Readonly<{
      frameKind: "delta"
      delta: WorldDelta
    }>
  | Readonly<{
      frameKind: "diagnostic"
      delta: WorldDelta
      diagnostic: WorldDiagnostic
    }>

type BrowserWebSocketLike = Readonly<{
  readonly readyState: number
  send: (data: string) => void
  close: () => void
  addEventListener: (type: string, listener: (event: unknown) => void, options?: unknown) => void
  removeEventListener: (type: string, listener: (event: unknown) => void) => void
}>

type BrowserWebSocketCtor = new (url: string) => BrowserWebSocketLike

export type BrowserWorldTransportInput = Readonly<{
  worldUrl: string
  actorRef: string
  actorClass?: "browser" | "agent" | "operator"
  fetchFn?: typeof fetch
  webSocketCtor?: BrowserWebSocketCtor
  onDelta?: (delta: WorldDelta, frame: WorldTransportFrame) => void
  onDiagnostic?: (diagnostic: WorldDiagnostic, frame: WorldTransportFrame) => void
}>

export const runWorldClientEffect = <A, E>(
  effect: Effect.Effect<A, E>,
): Promise<A> => Effect.runPromise(effect)

export const worldClientNowIso = (): string => new Date().toISOString()

export const makeWorldClientActorRef = (prefix = "world.client"): string =>
  `${prefix}.${crypto.randomUUID()}`

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

const WEBSOCKET_OPEN = 1

const socketUrlWithSession = (
  socketUrl: string,
  session: Readonly<{
    actorRef: string
    actorClass: "browser" | "agent" | "operator"
    characterId?: string
    cursor?: string
  }>,
): string => {
  const url = new URL(socketUrl)
  url.searchParams.set("actorRef", session.actorRef)
  url.searchParams.set("actorClass", session.actorClass)
  if (session.characterId !== undefined) url.searchParams.set("characterId", session.characterId)
  if (session.cursor !== undefined) url.searchParams.set("cursor", session.cursor)
  return url.toString()
}

export const decodeWorldTransportFrame = (input: unknown): WorldTransportFrame => {
  if (typeof input !== "object" || input === null) {
    throw new WorldClientError({
      phase: "delta",
      reason: "World transport frame must be an object.",
      retryable: true,
      sourceRefs: ["world-client.transport.frame"],
    })
  }
  const frame = input as Record<string, unknown>
  const delta = decodeWorldDelta(frame.delta)
  if (frame.frameKind === "snapshot") {
    return {
      frameKind: "snapshot",
      delta,
      readModel: decodeWorldReadModel(frame.readModel),
    }
  }
  if (frame.frameKind === "delta") {
    return { frameKind: "delta", delta }
  }
  if (frame.frameKind === "diagnostic" && delta.diagnostic !== undefined) {
    return {
      frameKind: "diagnostic",
      delta,
      diagnostic: delta.diagnostic,
    }
  }
  throw new WorldClientError({
    phase: "delta",
    reason: "World transport frame kind is not supported.",
    retryable: true,
    sourceRefs: ["world-client.transport.frame"],
  })
}

export const createBrowserWorldTransport = (
  input: BrowserWorldTransportInput,
): WorldClientTransport => {
  const baseUrl = input.worldUrl.replace(/\/+$/, "")
  const fetchFn = input.fetchFn ?? fetch
  const WebSocketCtor = input.webSocketCtor ??
    (globalThis as unknown as { WebSocket?: BrowserWebSocketCtor }).WebSocket
  let socket: BrowserWebSocketLike | null = null
  const pendingCommands = new Map<
    string,
    {
      resolve: (delta: WorldDelta) => void
      reject: (error: WorldClientError) => void
    }
  >()

  const applyFrame = (frame: WorldTransportFrame): void => {
    input.onDelta?.(frame.delta, frame)
    if (frame.frameKind === "diagnostic") input.onDiagnostic?.(frame.diagnostic, frame)
    const commandRef = frame.delta.receipt?.commandRef
    if (commandRef !== undefined) {
      pendingCommands.get(commandRef)?.resolve(frame.delta)
      pendingCommands.delete(commandRef)
    }
  }

  const connectSocket = (
    socketUrl: string,
  ): Effect.Effect<void, WorldClientError> =>
    Effect.tryPromise({
      try: () =>
        new Promise<void>((resolve, reject) => {
          if (WebSocketCtor === undefined) {
            reject(new Error("WebSocket is not available in this runtime."))
            return
          }
          const next = new WebSocketCtor(socketUrl)
          socket = next
          const cleanup = (): void => {
            next.removeEventListener("open", onOpen)
            next.removeEventListener("message", onMessage)
            next.removeEventListener("error", onError)
          }
          const onOpen = (): void => {
            cleanup()
            next.addEventListener("message", onMessage)
            next.send(JSON.stringify({ frameKind: "hydrate" }))
            resolve()
          }
          const onError = (): void => {
            cleanup()
            reject(new Error("World WebSocket connection failed."))
          }
          const onMessage = (event: unknown): void => {
            const data = (event as { data?: unknown }).data
            if (typeof data !== "string") return
            try {
              applyFrame(decodeWorldTransportFrame(JSON.parse(data)))
            } catch {
              // Bad frames are ignored locally; the Worker emits typed diagnostics
              // for command/schema failures that pass the transport boundary.
            }
          }
          next.addEventListener("open", onOpen, { once: true })
          next.addEventListener("error", onError, { once: true })
        }),
      catch: error => new WorldClientError({
        phase: "connect",
        reason: error instanceof Error ? error.message : "World socket connection failed.",
        retryable: true,
        sourceRefs: ["world-client.browser.socket"],
      }),
    })

  const connect = (
    request: WorldClientConnectRequest = {},
  ): Effect.Effect<WorldClientConnectResult, WorldClientError> =>
    Effect.gen(function* () {
      const url = new URL(`${baseUrl}/connect`)
      if (request.characterId !== undefined) url.searchParams.set("characterId", request.characterId)
      if (request.regionRef !== undefined) url.searchParams.set("region", request.regionRef)
      if (request.runRef !== undefined) url.searchParams.set("runRef", request.runRef)
      if (request.scope !== undefined) url.searchParams.set("scope", request.scope)
      if (request.selectedEntityRef !== undefined) {
        url.searchParams.set("selectedEntityRef", request.selectedEntityRef)
      }
      if (request.resumeCursor !== undefined) url.searchParams.set("cursor", request.resumeCursor)
      for (const selectedRef of request.selectedRefs ?? []) {
        url.searchParams.append("selectedRef", selectedRef)
      }

      const result = yield* Effect.tryPromise({
        try: async () => {
          const response = await fetchFn(url, { headers: { accept: "application/json" } })
          if (!response.ok) {
            throw new Error(`World connect failed with HTTP ${response.status}.`)
          }
          const payload = await response.json() as {
            ok?: boolean
            schemaVersion?: string
            regionRef?: string
            socketUrl?: string
            subscriptionPlan?: unknown
          }
          if (payload.ok !== true || payload.schemaVersion !== WORLD_CONTRACT_SCHEMA_VERSION) {
            throw new Error("World connect response did not match the contract schema.")
          }
          if (typeof payload.regionRef !== "string" || typeof payload.socketUrl !== "string") {
            throw new Error("World connect response did not include a region socket.")
          }
          return {
            regionRef: payload.regionRef,
            socketUrl: socketUrlWithSession(payload.socketUrl, {
              actorRef: input.actorRef,
              actorClass: input.actorClass ?? "browser",
              ...(request.characterId === undefined ? {} : { characterId: request.characterId }),
              ...(request.resumeCursor === undefined ? {} : { cursor: request.resumeCursor }),
            }),
            subscriptionPlan: decodeWorldSubscriptionPlan(payload.subscriptionPlan),
          }
        },
        catch: error => new WorldClientError({
          phase: "connect",
          reason: error instanceof Error ? error.message : "World connect request failed.",
          retryable: true,
          sourceRefs: ["world-client.browser.connect"],
        }),
      })
      yield* connectSocket(result.socketUrl)
      return result
    })

  return {
    connect,
    subscribe: connect,
    command: command =>
      Effect.tryPromise({
        try: () =>
          new Promise<WorldDelta>((resolve, reject) => {
            if (socket === null || socket.readyState !== WEBSOCKET_OPEN) {
              reject(new Error("World socket is not connected."))
              return
            }
            pendingCommands.set(command.commandRef, {
              resolve,
              reject: error => reject(error),
            })
            socket.send(JSON.stringify(command))
          }),
        catch: error => error instanceof WorldClientError
          ? error
          : new WorldClientError({
              phase: "command",
              reason: error instanceof Error ? error.message : "World command send failed.",
              retryable: true,
              sourceRefs: ["world-client.browser.command"],
            }),
      }),
    disconnect: () =>
      Effect.sync(() => {
        socket?.close()
        socket = null
        for (const pending of pendingCommands.values()) {
          pending.reject(new WorldClientError({
            phase: "disconnect",
            reason: "World socket disconnected before command acknowledgement.",
            retryable: true,
            sourceRefs: ["world-client.browser.disconnect"],
          }))
        }
        pendingCommands.clear()
      }),
  }
}

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
    gateways: {},
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

export type WorldMinimapMarkerKind =
  | "assignment"
  | "avatar"
  | "pylon"
  | "run_core"

export type WorldMinimapCoordinate = Readonly<{
  x: number
  y: number
}>

export type WorldMinimapMarker = Readonly<{
  kind: WorldMinimapMarkerKind
  ref: string
  label: string
  regionRef: string
  worldPosition: Readonly<{ x: number; y: number; z: number }>
  minimap: WorldMinimapCoordinate
  sourceRefs: ReadonlyArray<string>
  state?: string
}>

export type WorldSubzoneReadout = Readonly<{
  subzoneRef: string
  label: string
  regionRef: string
  regionLabel: string
}>

export type WorldMinimapReadout = Readonly<{
  coordinate: Readonly<{ x: number; y: number; z: number }>
  markers: ReadonlyArray<WorldMinimapMarker>
  regionRef: string
  regionLabel: string
  sizePx: number
  subzone: WorldSubzoneReadout
}>

export type WorldMinimapProjectionInput = Readonly<{
  readModel: ClientWorld
  localPosition?: Readonly<{ x: number; y?: number; z: number }>
  previousSubzoneRef?: string
  sizePx?: number
  subzoneHysteresisMeters?: number
}>

const roundCoordinate = (value: number): number =>
  Number(value.toFixed(4))

const clamp01 = (value: number): number =>
  Math.max(0, Math.min(1, value))

const regionForReadout = (readModel: ClientWorld): ClientWorld["regions"][string] => {
  const direct = readModel.regions[String(readModel.regionRef)]
  if (direct !== undefined) return direct
  const first = Object.values(readModel.regions)[0]
  if (first !== undefined) return first
  const fallback = decodeWorldRow({
    kind: "world_region",
    regionRef: readModel.regionRef,
    label: String(readModel.regionRef),
    bounds: {
      min: { x: -64, y: -8, z: -64 },
      max: { x: 64, y: 24, z: 64 },
    },
    origin: { x: 0, y: 0, z: 0 },
    proximityRadius: 12,
    staleAvatarTtlMs: 30_000,
    updatedAt: readModel.generatedAt,
    safety: {
      publicProjectionAllowed: true,
      sourceRefs: ["world-client.minimap.fallback-region"],
      blockerRefs: [],
      caveatRefs: [],
    },
  })
  if (fallback.kind !== "world_region") {
    throw new Error("fallback world region decode failed")
  }
  return fallback
}

const worldToMinimap = (
  region: ClientWorld["regions"][string],
  position: Readonly<{ x: number; z: number }>,
  sizePx: number,
): WorldMinimapCoordinate => {
  const width = Math.max(1, region.bounds.max.x - region.bounds.min.x)
  const depth = Math.max(1, region.bounds.max.z - region.bounds.min.z)
  return {
    x: roundCoordinate(clamp01((position.x - region.bounds.min.x) / width) * sizePx),
    y: roundCoordinate(clamp01((position.z - region.bounds.min.z) / depth) * sizePx),
  }
}

const runEntityPosition = (
  region: ClientWorld["regions"][string],
  index: number,
): Readonly<{ x: number; y: number; z: number }> => {
  const radius = Math.max(
    8,
    Math.min(
      28,
      Math.min(region.bounds.max.x - region.bounds.min.x, region.bounds.max.z - region.bounds.min.z) * 0.18,
    ),
  )
  const angle = -Math.PI / 2 + index * 0.76
  return {
    x: roundCoordinate(region.origin.x + Math.cos(angle) * radius),
    y: region.origin.y,
    z: roundCoordinate(region.origin.z + Math.sin(angle) * radius),
  }
}

const sourceRefsFromSafety = (
  safety: Readonly<{ sourceRefs: ReadonlyArray<string> }>,
  fallbackRef: string,
): ReadonlyArray<string> =>
  safety.sourceRefs.length > 0 ? safety.sourceRefs.map(String) : [fallbackRef]

const subzoneForPosition = (
  region: ClientWorld["regions"][string],
  position: Readonly<{ x: number; z: number }>,
  previousSubzoneRef: string | undefined,
  hysteresisMeters: number,
): WorldSubzoneReadout => {
  const centerX = (region.bounds.min.x + region.bounds.max.x) / 2
  const centerZ = (region.bounds.min.z + region.bounds.max.z) / 2
  const dx = position.x - centerX
  const dz = position.z - centerZ
  const choose = (): "center" | "east" | "north" | "south" | "west" => {
    if (Math.abs(dx) <= hysteresisMeters && Math.abs(dz) <= hysteresisMeters) return "center"
    if (Math.abs(dx) >= Math.abs(dz)) return dx >= 0 ? "east" : "west"
    return dz >= 0 ? "south" : "north"
  }
  const previous = previousSubzoneRef?.startsWith(`${region.regionRef}:`)
    ? previousSubzoneRef.slice(`${region.regionRef}:`.length)
    : undefined
  const next = choose()
  const stable =
    previous === "east" && dx > -hysteresisMeters ? "east"
      : previous === "west" && dx < hysteresisMeters ? "west"
        : previous === "south" && dz > -hysteresisMeters ? "south"
          : previous === "north" && dz < hysteresisMeters ? "north"
            : next
  const label = stable === "center"
    ? "Center"
    : `${stable.slice(0, 1).toUpperCase()}${stable.slice(1)}`
  return {
    regionRef: region.regionRef,
    regionLabel: region.label,
    subzoneRef: `${region.regionRef}:${stable}`,
    label,
  }
}

export const projectWorldMinimapReadout = ({
  localPosition,
  previousSubzoneRef,
  readModel,
  sizePx = 160,
  subzoneHysteresisMeters = 4,
}: WorldMinimapProjectionInput): WorldMinimapReadout => {
  const region = regionForReadout(readModel)
  const position = localPosition ?? region.origin
  const markers: Array<WorldMinimapMarker> = []

  for (const station of Object.values(readModel.pylons)) {
    if (station.regionRef !== region.regionRef) continue
    markers.push({
      kind: "pylon",
      ref: station.pylonRef,
      label: station.label,
      regionRef: station.regionRef,
      worldPosition: station.position,
      minimap: worldToMinimap(region, station.position, sizePx),
      state: station.status,
      sourceRefs: sourceRefsFromSafety(station.safety, station.pylonRef),
    })
  }

  for (const avatar of Object.values(readModel.avatars)) {
    const avatarPosition = readModel.positions[String(avatar.avatarRef)]
    if (avatar.regionRef !== region.regionRef || avatarPosition?.regionRef !== region.regionRef) continue
    markers.push({
      kind: "avatar",
      ref: avatar.avatarRef,
      label: avatar.label,
      regionRef: avatar.regionRef,
      worldPosition: avatarPosition.position,
      minimap: worldToMinimap(region, avatarPosition.position, sizePx),
      state: avatarPosition.animation,
      sourceRefs: sourceRefsFromSafety(avatar.safety, avatar.avatarRef),
    })
  }

  for (const run of Object.values(readModel.runs)) {
    markers.push({
      kind: "run_core",
      ref: run.runRef,
      label: run.label,
      regionRef: region.regionRef,
      worldPosition: region.origin,
      minimap: worldToMinimap(region, region.origin, sizePx),
      state: run.state,
      sourceRefs: sourceRefsFromSafety(run.safety, run.runRef),
    })
  }

  Object.values(readModel.entities).forEach((entity, index) => {
    const position = runEntityPosition(region, index)
    markers.push({
      kind: "assignment",
      ref: entity.entityRef,
      label: entity.label,
      regionRef: region.regionRef,
      worldPosition: position,
      minimap: worldToMinimap(region, position, sizePx),
      state: entity.entityKind,
      sourceRefs: sourceRefsFromSafety(entity.safety, entity.entityRef),
    })
  })

  return {
    coordinate: {
      x: roundCoordinate(position.x - region.origin.x),
      y: roundCoordinate((localPosition?.y ?? region.origin.y) - region.origin.y),
      z: roundCoordinate(position.z - region.origin.z),
    },
    markers: markers.sort((left, right) =>
      `${left.kind}:${left.ref}`.localeCompare(`${right.kind}:${right.ref}`),
    ),
    regionRef: region.regionRef,
    regionLabel: region.label,
    sizePx,
    subzone: subzoneForPosition(
      region,
      position,
      previousSubzoneRef,
      Math.max(0, subzoneHysteresisMeters),
    ),
  }
}

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
  gateways: Record<string, ClientWorld["gateways"][string]>
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
    case "gateway_station":
      readModel.gateways[key] = row
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
  delete readModel.gateways[ref]
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
