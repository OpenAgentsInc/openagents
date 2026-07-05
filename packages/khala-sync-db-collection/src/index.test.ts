import { createCollection } from "@tanstack/db"
import {
  BootstrapEntity,
  type BootstrapRequest,
  BootstrapResponse,
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  canonicalJson,
  ChangelogEntry,
  ClientGroupId,
  ClientId,
  DeltaFrame,
  EntityId,
  EntityType,
  type LiveFrame,
  LogPage,
  type MutationId,
  MutationResult,
  MutatorName,
  PushResponse,
  SyncError,
  SyncSchemaVersion,
  type SyncScope,
  SyncVersion,
  SyncVersionWatermark,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  decodeFleetRunEntity,
  decodeRuntimeEventEntity,
  decodeRuntimeTurnEntity,
  encodeChatMessageEntity,
  encodeChatThreadEntity,
  encodeFleetRunEntity,
  encodeRuntimeEventEntity,
  encodeRuntimeTurnEntity,
  FLEET_RUN_ENTITY_TYPE,
  fleetRunScope,
  personalScope,
  RUNTIME_EVENT_ENTITY_TYPE,
  RUNTIME_TURN_ENTITY_TYPE,
  threadScope,
  type ChatMessageEntity,
  type ChatThreadEntity,
  type FleetRunEntity,
  type RuntimeEventEntity,
  type RuntimeTurnEntity,
} from "@openagentsinc/khala-sync"
import {
  type ClientMutator,
  createKhalaSyncSession,
  createOverlay,
  type KhalaSyncTransport,
  KhalaSyncTransportError,
  type LiveSocketHandlers,
  openKhalaSyncStore,
} from "@openagentsinc/khala-sync-client"
import { afterEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  CHAT_APPEND_MESSAGE_MUTATOR_NAME,
  CHAT_CREATE_THREAD_MUTATOR_NAME,
  CHAT_RENAME_THREAD_MUTATOR_NAME,
  chatAppendMessageClientMutator,
  chatCreateThreadClientMutator,
  chatMessageKhalaSyncCollectionOptions,
  chatMessagesForTranscript,
  chatRenameThreadClientMutator,
  chatThreadKhalaSyncCollectionOptions,
  type ChatAppendMessageArgs,
  type ChatCreateThreadArgs,
  type ChatRenameThreadArgs,
  chatThreadsForSidebar,
  createKhalaSyncMutationTracker,
  FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME,
  fleetRunKhalaSyncCollectionOptions,
  fleetSetDesiredSlotsClientMutator,
  type FleetSetDesiredSlotsArgs,
  KhalaSyncDbCollectionError,
  runtimeEventKhalaSyncCollectionOptions,
  runtimeTurnKhalaSyncCollectionOptions,
} from "./index.js"

const tick = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0))

const waitFor = async (
  condition: () => boolean,
  label: string,
  ticks = 3000,
): Promise<void> => {
  for (let i = 0; i < ticks; i++) {
    if (condition()) return
    await tick()
  }
  throw new Error(`timed out waiting for: ${label}`)
}

const FIXED_TIME = "2026-07-04T20:00:00.000Z"
const UPDATED_TIME = "2026-07-04T20:01:00.000Z"

const cleanups: Array<() => void> = []
afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()!()
})

const loadFleetRunFixture = async (): Promise<FleetRunEntity> =>
  decodeFleetRunEntity(
    (await Bun.file(
      new URL("../../khala-sync/fixtures/FleetRunEntity.json", import.meta.url),
    ).json()) as unknown,
  )

type FakeChange = Readonly<{
  entityType: string
  entityId: string
  postImageJson: string
}>

type SocketRecord = Readonly<{
  handlers: LiveSocketHandlers
  open: { value: boolean }
}>

const accessDeniedError = (): KhalaSyncTransportError =>
  new KhalaSyncTransportError(
    "sync_error",
    false,
    "khala-sync server error unauthorized_scope",
    {
      status: 403,
      syncError: new SyncError({
        code: "unauthorized_scope",
        messageSafe: "This user cannot read the requested scope.",
        retryable: false,
      }),
    },
  )

class FleetFakeServer {
  readonly logs = new Map<SyncScope, Array<ChangelogEntry>>()
  readonly sockets = new Map<SyncScope, SocketRecord>()
  readonly clientLast = new Map<string, number>()
  readonly deniedScopes = new Set<SyncScope>()
  readonly pushCalls: Array<ReadonlyArray<{ name: string; mutationId: number }>> =
    []

  logOf(scope: SyncScope): Array<ChangelogEntry> {
    let log = this.logs.get(scope)
    if (log === undefined) {
      log = []
      this.logs.set(scope, log)
    }
    return log
  }

  lastVersion(scope: SyncScope): number {
    const log = this.logOf(scope)
    return log.length === 0 ? 0 : log[log.length - 1]!.version
  }

  fold(scope: SyncScope, throughVersion: number): Array<FakeChange> {
    const state = new Map<string, FakeChange>()
    for (const entry of this.logOf(scope)) {
      if (entry.version > throughVersion) continue
      const key = `${entry.entityType}/${entry.entityId}`
      if (entry.op === "delete") state.delete(key)
      else {
        state.set(key, {
          entityId: entry.entityId,
          entityType: entry.entityType,
          postImageJson: entry.postImageJson!,
        })
      }
    }
    return [...state.values()].sort((a, b) =>
      `${a.entityType}/${a.entityId}` < `${b.entityType}/${b.entityId}` ? -1 : 1,
    )
  }

  currentFleetRun(runId: string): FleetRunEntity | null {
    const scope = fleetRunScope(runId)
    const row = this
      .fold(scope, this.lastVersion(scope))
      .find(
        entity =>
          entity.entityType === FLEET_RUN_ENTITY_TYPE &&
          entity.entityId === runId,
      )
    return row === undefined
      ? null
      : decodeFleetRunEntity(JSON.parse(row.postImageJson) as unknown)
  }

  commitFleetRun(row: FleetRunEntity, mutationRef?: string): number {
    const scope = fleetRunScope(row.runId)
    const version = this.lastVersion(scope) + 1
    const entry = new ChangelogEntry({
      committedAt: FIXED_TIME,
      entityId: EntityId.make(row.runId),
      entityType: EntityType.make(FLEET_RUN_ENTITY_TYPE),
      ...(mutationRef !== undefined ? { mutationRef } : {}),
      op: "upsert",
      postImageJson: canonicalJson(encodeFleetRunEntity(row)),
      scope,
      version: SyncVersion.make(version),
    })
    this.logOf(scope).push(entry)
    this.emitFrame(
      scope,
      new DeltaFrame({
        cursor: SyncVersion.make(version),
        entries: [entry],
        scope,
      }),
    )
    return version
  }

  currentChatThread(ownerUserId: string, threadId: string): ChatThreadEntity | null {
    const scope = personalScope(ownerUserId)
    const row = this
      .fold(scope, this.lastVersion(scope))
      .find(
        entity =>
          entity.entityType === CHAT_THREAD_ENTITY_TYPE &&
          entity.entityId === threadId,
      )
    return row === undefined
      ? null
      : decodeChatThreadEntity(JSON.parse(row.postImageJson) as unknown)
  }

  currentChatMessage(threadId: string, messageId: string): ChatMessageEntity | null {
    const scope = threadScope(threadId)
    const row = this
      .fold(scope, this.lastVersion(scope))
      .find(
        entity =>
          entity.entityType === CHAT_MESSAGE_ENTITY_TYPE &&
          entity.entityId === messageId,
      )
    return row === undefined
      ? null
      : decodeChatMessageEntity(JSON.parse(row.postImageJson) as unknown)
  }

  commitChatThreadToScope(
    scope: SyncScope,
    row: ChatThreadEntity,
    mutationRef?: string,
  ): number {
    const version = this.lastVersion(scope) + 1
    const entry = new ChangelogEntry({
      committedAt: FIXED_TIME,
      entityId: EntityId.make(row.threadId),
      entityType: EntityType.make(CHAT_THREAD_ENTITY_TYPE),
      ...(mutationRef !== undefined ? { mutationRef } : {}),
      op: "upsert",
      postImageJson: canonicalJson(encodeChatThreadEntity(row)),
      scope,
      version: SyncVersion.make(version),
    })
    this.logOf(scope).push(entry)
    this.emitFrame(
      scope,
      new DeltaFrame({
        cursor: SyncVersion.make(version),
        entries: [entry],
        scope,
      }),
    )
    return version
  }

  commitChatThread(row: ChatThreadEntity, mutationRef?: string): number {
    const ownerVersion = this.commitChatThreadToScope(
      personalScope(row.ownerUserId),
      row,
      mutationRef,
    )
    this.commitChatThreadToScope(threadScope(row.threadId), row, mutationRef)
    return ownerVersion
  }

  commitChatMessage(row: ChatMessageEntity, mutationRef?: string): number {
    const scope = threadScope(row.threadId)
    const version = this.lastVersion(scope) + 1
    const entry = new ChangelogEntry({
      committedAt: FIXED_TIME,
      entityId: EntityId.make(row.messageId),
      entityType: EntityType.make(CHAT_MESSAGE_ENTITY_TYPE),
      ...(mutationRef !== undefined ? { mutationRef } : {}),
      op: "upsert",
      postImageJson: canonicalJson(encodeChatMessageEntity(row)),
      scope,
      version: SyncVersion.make(version),
    })
    this.logOf(scope).push(entry)
    this.emitFrame(
      scope,
      new DeltaFrame({
        cursor: SyncVersion.make(version),
        entries: [entry],
        scope,
      }),
    )
    return version
  }

  /** #8425: desktop's runtime_event/runtime_turn render gap — these two rows
   * are server-authored (the runtime-intent-supervisor writes them, never
   * the desktop client), so this fake commits them directly to
   * `threadScope`, matching how `commitChatMessage` above simulates a
   * server-side write rather than a client mutation round-trip. */
  commitRuntimeTurn(row: RuntimeTurnEntity): number {
    const scope = threadScope(row.threadId)
    const version = this.lastVersion(scope) + 1
    const entry = new ChangelogEntry({
      committedAt: FIXED_TIME,
      entityId: EntityId.make(row.turnId),
      entityType: EntityType.make(RUNTIME_TURN_ENTITY_TYPE),
      op: "upsert",
      postImageJson: canonicalJson(encodeRuntimeTurnEntity(row)),
      scope,
      version: SyncVersion.make(version),
    })
    this.logOf(scope).push(entry)
    this.emitFrame(
      scope,
      new DeltaFrame({ cursor: SyncVersion.make(version), entries: [entry], scope }),
    )
    return version
  }

  commitRuntimeEvent(row: RuntimeEventEntity): number {
    const scope = threadScope(row.threadId)
    const version = this.lastVersion(scope) + 1
    const entry = new ChangelogEntry({
      committedAt: FIXED_TIME,
      entityId: EntityId.make(row.eventId),
      entityType: EntityType.make(RUNTIME_EVENT_ENTITY_TYPE),
      op: "upsert",
      postImageJson: canonicalJson(encodeRuntimeEventEntity(row)),
      scope,
      version: SyncVersion.make(version),
    })
    this.logOf(scope).push(entry)
    this.emitFrame(
      scope,
      new DeltaFrame({ cursor: SyncVersion.make(version), entries: [entry], scope }),
    )
    return version
  }

  emitFrame(scope: SyncScope, frame: LiveFrame): void {
    const socket = this.sockets.get(scope)
    if (socket !== undefined && socket.open.value) {
      socket.handlers.onFrame(frame)
    }
  }

  denyIfNeeded(scope: SyncScope): void {
    if (this.deniedScopes.has(scope)) throw accessDeniedError()
  }

  bootstrap(request: BootstrapRequest): BootstrapResponse {
    this.denyIfNeeded(request.scope)
    const cursor = this.lastVersion(request.scope)
    return new BootstrapResponse({
      cursor: SyncVersionWatermark.make(cursor),
      entities: this.fold(request.scope, cursor).map(
        entity =>
          new BootstrapEntity({
            entityId: EntityId.make(entity.entityId),
            entityType: EntityType.make(entity.entityType),
            postImageJson: entity.postImageJson,
          }),
      ),
      protocolVersion: 1,
      scope: request.scope,
    })
  }

  logPage(scope: SyncScope, cursor: number, limit: number): LogPage {
    this.denyIfNeeded(scope)
    const entries = this
      .logOf(scope)
      .filter(entry => entry.version > cursor)
      .slice(0, limit)
    const last = entries[entries.length - 1]
    const nextCursor = last === undefined ? cursor : last.version
    return new LogPage({
      entries,
      nextCursor: SyncVersionWatermark.make(nextCursor),
      protocolVersion: 1,
      scope,
      upToDate: nextCursor >= this.lastVersion(scope),
    })
  }

  push(
    mutations: ReadonlyArray<{
      mutationId: MutationId
      name: string
      argsJson: string
    }>,
    clientKey: string,
  ): PushResponse {
    this.pushCalls.push(
      mutations.map(mutation => ({
        mutationId: Number(mutation.mutationId),
        name: mutation.name,
      })),
    )
    let last = this.clientLast.get(clientKey) ?? 0
    const results: Array<MutationResult> = []

    for (const mutation of mutations) {
      if (mutation.mutationId <= last) {
        results.push(
          new MutationResult({
            mutationId: mutation.mutationId,
            status: "duplicate",
          }),
        )
        continue
      }

      if (mutation.name === "fleet.rejectDesiredSlots") {
        last = mutation.mutationId
        results.push(
          new MutationResult({
            errorCode: "test_rejected",
            errorMessageSafe: "test rejected desired slot change",
            mutationId: mutation.mutationId,
            status: "rejected",
          }),
        )
        continue
      }

      if (mutation.name === FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME) {
        const args = JSON.parse(mutation.argsJson) as FleetSetDesiredSlotsArgs
        const current = this.currentFleetRun(args.runId)
        if (current === null) {
          throw new Error(`missing fleet run ${args.runId}`)
        }
        this.commitFleetRun(
          decodeFleetRunEntity({
            ...current,
            counters: { ...current.counters },
            desiredSlots: args.desiredSlots,
            updatedAt: UPDATED_TIME,
          }),
          `mut.${clientKey}.${mutation.mutationId}`,
        )
        last = mutation.mutationId
        results.push(
          new MutationResult({
            mutationId: mutation.mutationId,
            status: "applied",
          }),
        )
        continue
      }

      if (mutation.name === CHAT_CREATE_THREAD_MUTATOR_NAME) {
        const args = JSON.parse(mutation.argsJson) as ChatCreateThreadArgs
        const ownerUserId = "user-chat-owner"
        const existing = this.currentChatThread(ownerUserId, args.threadId)
        if (existing !== null) {
          last = mutation.mutationId
          results.push(
            new MutationResult({
              errorCode: "thread_exists",
              errorMessageSafe: "this chat thread already exists",
              mutationId: mutation.mutationId,
              status: "rejected",
            }),
          )
          continue
        }
        this.commitChatThread(
          decodeChatThreadEntity({
            createdAt: FIXED_TIME,
            lastMessageAt: null,
            messageCount: 0,
            ownerUserId,
            status: "active",
            threadId: args.threadId,
            title: args.title.trim(),
            updatedAt: FIXED_TIME,
          }),
          `mut.${clientKey}.${mutation.mutationId}`,
        )
        last = mutation.mutationId
        results.push(
          new MutationResult({
            mutationId: mutation.mutationId,
            status: "applied",
          }),
        )
        continue
      }

      if (mutation.name === CHAT_APPEND_MESSAGE_MUTATOR_NAME) {
        const args = JSON.parse(mutation.argsJson) as ChatAppendMessageArgs
        const ownerUserId = "user-chat-owner"
        const current = this.currentChatThread(ownerUserId, args.threadId)
        if (current === null) {
          last = mutation.mutationId
          results.push(
            new MutationResult({
              errorCode: "thread_not_found",
              errorMessageSafe: "this chat thread does not exist",
              mutationId: mutation.mutationId,
              status: "rejected",
            }),
          )
          continue
        }
        if (this.currentChatMessage(args.threadId, args.messageId) !== null) {
          last = mutation.mutationId
          results.push(
            new MutationResult({
              errorCode: "message_exists",
              errorMessageSafe: "this chat message already exists",
              mutationId: mutation.mutationId,
              status: "rejected",
            }),
          )
          continue
        }
        const message = decodeChatMessageEntity({
          authorUserId: ownerUserId,
          body: args.body,
          createdAt: UPDATED_TIME,
          deletedAt: null,
          messageId: args.messageId,
          threadId: args.threadId,
          updatedAt: UPDATED_TIME,
        })
        this.commitChatThread(
          decodeChatThreadEntity({
            ...current,
            lastMessageAt: UPDATED_TIME,
            messageCount: current.messageCount + 1,
            updatedAt: UPDATED_TIME,
          }),
          `mut.${clientKey}.${mutation.mutationId}`,
        )
        this.commitChatMessage(message, `mut.${clientKey}.${mutation.mutationId}`)
        last = mutation.mutationId
        results.push(
          new MutationResult({
            mutationId: mutation.mutationId,
            status: "applied",
          }),
        )
        continue
      }

      if (mutation.name === CHAT_RENAME_THREAD_MUTATOR_NAME) {
        const args = JSON.parse(mutation.argsJson) as ChatRenameThreadArgs
        const ownerUserId = "user-chat-owner"
        const current = this.currentChatThread(ownerUserId, args.threadId)
        if (current === null) {
          last = mutation.mutationId
          results.push(
            new MutationResult({
              errorCode: "thread_not_found",
              errorMessageSafe: "this chat thread does not exist",
              mutationId: mutation.mutationId,
              status: "rejected",
            }),
          )
          continue
        }
        this.commitChatThread(
          decodeChatThreadEntity({
            ...current,
            title: args.title.trim(),
            updatedAt: UPDATED_TIME,
          }),
          `mut.${clientKey}.${mutation.mutationId}`,
        )
        last = mutation.mutationId
        results.push(
          new MutationResult({
            mutationId: mutation.mutationId,
            status: "applied",
          }),
        )
        continue
      }

      throw new Error(`unknown mutator ${mutation.name}`)
    }

    this.clientLast.set(clientKey, last)
    return new PushResponse({
      lastMutationId: last,
      protocolVersion: 1,
      results,
    })
  }

  connect(scope: SyncScope, _cursor: number, handlers: LiveSocketHandlers) {
    this.denyIfNeeded(scope)
    const open = { value: true }
    this.sockets.set(scope, { handlers, open })
    return {
      close: () => {
        open.value = false
      },
    }
  }
}

const transportOf = (server: FleetFakeServer): KhalaSyncTransport => {
  const attempt = <A>(run: () => A): Effect.Effect<A, KhalaSyncTransportError> =>
    Effect.suspend(() => {
      try {
        return Effect.succeed(run())
      } catch (error) {
        return Effect.fail(
          error instanceof KhalaSyncTransportError
            ? error
            : new KhalaSyncTransportError("network", true, String(error), {
                cause: error,
              }),
        )
      }
    })

  return {
    bootstrap: request => attempt(() => server.bootstrap(request)),
    connectLive: (scope, cursor, handlers) =>
      attempt(() => server.connect(scope, cursor, handlers)),
    logPage: (scope, cursor, limit) =>
      attempt(() => server.logPage(scope, cursor, limit)),
    push: request =>
      attempt(() =>
        server.push(
          request.mutations,
          `${request.clientGroupId}:${request.clientId}`,
        ),
      ),
  }
}

const rejectingDesiredSlotsMutator: ClientMutator<FleetSetDesiredSlotsArgs> = {
  ...fleetSetDesiredSlotsClientMutator,
  name: MutatorName.make("fleet.rejectDesiredSlots"),
}

const makeHarness = async (
  server: FleetFakeServer,
  options: {
    readonly clientGroupId?: string
    readonly clientId?: string
    // Heterogeneous test overlay registry, matching createOverlay's runtime API.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    readonly mutators?: readonly ClientMutator<any>[]
  } = {},
) => {
  const tracker = createKhalaSyncMutationTracker()
  const store = openKhalaSyncStore(":memory:")
  cleanups.push(() => Effect.runSync(Effect.ignore(store.close())))
  const overlay = Effect.runSync(
    createOverlay(
      store,
      options.mutators ?? [
        fleetSetDesiredSlotsClientMutator,
        rejectingDesiredSlotsMutator,
      ],
    ),
  )
  const session = createKhalaSyncSession(
    {
      authToken: () => "test-token",
      baseUrl: "http://fake.test",
      clientGroupId: ClientGroupId.make(options.clientGroupId ?? "cg_fleet"),
      clientId: ClientId.make(options.clientId ?? "c_fleet"),
      schemaVersion: SyncSchemaVersion.make(1),
    },
    store,
    overlay,
    transportOf(server),
    {
      backoffBaseMs: 1,
      backoffMaxMs: 4,
      logPageLimit: 2,
      onRejection: tracker.onRejection,
      random: () => 0,
      sleep: () => tick(),
    },
  )
  cleanups.push(() => Effect.runSync(session.close()))
  return { overlay, session, store, tracker }
}

const createFleetRunCollection = (
  harness: Awaited<ReturnType<typeof makeHarness>>,
  scope: SyncScope,
  options: {
    readonly mutator?: ClientMutator<FleetSetDesiredSlotsArgs>
    readonly onError?: (error: KhalaSyncDbCollectionError) => void
  } = {},
) =>
  createCollection(
    fleetRunKhalaSyncCollectionOptions({
      awaitMutationPollIntervalMs: 0,
      awaitMutationTimeoutMs: 1_000,
      mutationTracker: harness.tracker,
      ...(options.onError !== undefined ? { onError: options.onError } : {}),
      overlay: harness.overlay,
      scope,
      session: harness.session,
      sleep: () => tick(),
      startSync: true,
      ...(options.mutator !== undefined
        ? { setDesiredSlotsMutator: options.mutator }
        : {}),
    }),
  )

const createChatThreadCollection = (
  harness: Awaited<ReturnType<typeof makeHarness>>,
  ownerUserId: string,
  mutators?: {
    readonly appendMessage?: ClientMutator<ChatAppendMessageArgs>
    readonly createThread: ClientMutator<ChatCreateThreadArgs>
    readonly renameThread: ClientMutator<ChatRenameThreadArgs>
  },
) =>
  createCollection(
    chatThreadKhalaSyncCollectionOptions({
      awaitMutationPollIntervalMs: 0,
      awaitMutationTimeoutMs: 1_000,
      ...(mutators === undefined
        ? {}
        : {
            createThreadMutator: mutators.createThread,
            renameThreadMutator: mutators.renameThread,
          }),
      mutationTracker: harness.tracker,
      optimisticNow: () => FIXED_TIME,
      overlay: harness.overlay,
      ownerUserId,
      scope: personalScope(ownerUserId),
      session: harness.session,
      sleep: () => tick(),
      startSync: true,
    }),
  )

const createChatMessageCollection = (
  harness: Awaited<ReturnType<typeof makeHarness>>,
  threadId: string,
  mutators?: {
    readonly appendMessage: ClientMutator<ChatAppendMessageArgs>
  },
) =>
  createCollection(
    chatMessageKhalaSyncCollectionOptions({
      awaitMutationPollIntervalMs: 0,
      awaitMutationTimeoutMs: 1_000,
      ...(mutators === undefined
        ? {}
        : { appendMessageMutator: mutators.appendMessage }),
      mutationTracker: harness.tracker,
      overlay: harness.overlay,
      scope: threadScope(threadId),
      session: harness.session,
      sleep: () => tick(),
      startSync: true,
    }),
  )

const createRuntimeTurnCollection = (
  harness: Awaited<ReturnType<typeof makeHarness>>,
  threadId: string,
) =>
  createCollection(
    runtimeTurnKhalaSyncCollectionOptions({
      mutationTracker: harness.tracker,
      overlay: harness.overlay,
      scope: threadScope(threadId),
      session: harness.session,
      sleep: () => tick(),
      startSync: true,
    }),
  )

const createRuntimeEventCollection = (
  harness: Awaited<ReturnType<typeof makeHarness>>,
  threadId: string,
) =>
  createCollection(
    runtimeEventKhalaSyncCollectionOptions({
      mutationTracker: harness.tracker,
      overlay: harness.overlay,
      scope: threadScope(threadId),
      session: harness.session,
      sleep: () => tick(),
      startSync: true,
    }),
  )

describe("khalaSyncCollectionOptions / fleet_run", () => {
  test("loads fleet_run fixture rows, applies live updates, and persists desiredSlots through the named mutator", async () => {
    const fixture = await loadFleetRunFixture()
    const server = new FleetFakeServer()
    server.commitFleetRun(fixture)
    const harness = await makeHarness(server)
    const scope = fleetRunScope(fixture.runId)
    const collection = createFleetRunCollection(harness, scope)

    await waitFor(() => collection.isReady(), "collection ready")
    expect(collection.get(fixture.runId)?.desiredSlots).toBe(5)

    server.commitFleetRun(
      decodeFleetRunEntity({
        ...fixture,
        counters: { ...fixture.counters },
        desiredSlots: 6,
        updatedAt: UPDATED_TIME,
      }),
    )
    await waitFor(
      () => collection.get(fixture.runId)?.desiredSlots === 6,
      "live update visible",
    )

    const tx = collection.update(fixture.runId, draft => {
      draft.desiredSlots = 7
    })
    await tx.isPersisted.promise

    await waitFor(
      () => collection.get(fixture.runId)?.desiredSlots === 7,
      "optimistic mutation confirmed",
    )
    expect(harness.session.pending()).toEqual([])
    expect(server.pushCalls.flat().map(call => call.name)).toContain(
      FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME,
    )
  })

  test("matches in-band rejections by mutation_id and rolls back the TanStack optimistic row", async () => {
    const fixture = await loadFleetRunFixture()
    const server = new FleetFakeServer()
    server.commitFleetRun(fixture)
    const harness = await makeHarness(server)
    const scope = fleetRunScope(fixture.runId)
    const collection = createFleetRunCollection(harness, scope, {
      mutator: rejectingDesiredSlotsMutator,
    })

    await waitFor(() => collection.isReady(), "collection ready")
    const tx = collection.update(fixture.runId, draft => {
      draft.desiredSlots = 99
    })

    await expect(tx.isPersisted.promise).rejects.toMatchObject({
      _tag: "KhalaSyncDbCollectionError",
      reasonRef: "khala_sync_db_collection.mutation_rejected",
    })
    await waitFor(
      () => collection.get(fixture.runId)?.desiredSlots === fixture.desiredSlots,
      "rejected optimistic row rolled back",
    )
  })

  test("surfaces denied fleet_run scope as a typed error instead of marking an empty collection ready", async () => {
    const fixture = await loadFleetRunFixture()
    const server = new FleetFakeServer()
    const scope = fleetRunScope(fixture.runId)
    server.deniedScopes.add(scope)
    const harness = await makeHarness(server)
    const errors: Array<KhalaSyncDbCollectionError> = []
    const collection = createFleetRunCollection(harness, scope, {
      onError: error => errors.push(error),
    })

    await waitFor(
      () =>
        errors.some(
          error => error.reasonRef === "khala_sync_db_collection.scope_denied",
        ),
      "typed denied error",
    )

    expect(collection.utils.getStatus().phase).toBe("denied")
    expect(collection.utils.getLastError()?.reasonRef).toBe(
      "khala_sync_db_collection.scope_denied",
    )
    await expect(collection.utils.loadSubset()).rejects.toMatchObject({
      _tag: "KhalaSyncDbCollectionError",
      reasonRef: "khala_sync_db_collection.scope_denied",
    })
  })
})

describe("chatThreadKhalaSyncCollectionOptions / chat_thread", () => {
  test("khala_code.chat.sync_remote_thread_appears_without_restart.v1: client A creates a thread and client B sees it without restart", async () => {
    const ownerUserId = "user-chat-owner"
    const server = new FleetFakeServer()
    const createThread = chatCreateThreadClientMutator({
      now: () => FIXED_TIME,
      ownerUserId,
    })
    const appendMessage = chatAppendMessageClientMutator({
      now: () => UPDATED_TIME,
      ownerUserId,
    })
    const renameThread = chatRenameThreadClientMutator({
      now: () => UPDATED_TIME,
      ownerUserId,
    })
    const mutators = [createThread, appendMessage, renameThread]
    const clientA = await makeHarness(server, {
      clientGroupId: "cg_chat_a",
      clientId: "c_chat_a",
      mutators,
    })
    const clientB = await makeHarness(server, {
      clientGroupId: "cg_chat_b",
      clientId: "c_chat_b",
      mutators,
    })
    const collectionA = createChatThreadCollection(clientA, ownerUserId, {
      createThread,
      renameThread,
    })
    const collectionB = createChatThreadCollection(clientB, ownerUserId, {
      appendMessage,
      createThread,
      renameThread,
    })
    const messagesA = createChatMessageCollection(clientA, "thread.remote.a", {
      appendMessage,
    })
    const messagesB = createChatMessageCollection(clientB, "thread.remote.a", {
      appendMessage,
    })

    await waitFor(
      () =>
        collectionA.isReady() &&
        collectionB.isReady() &&
        messagesA.isReady() &&
        messagesB.isReady(),
      "both chat thread/message collections ready",
    )

    const createTx = collectionA.insert(
      decodeChatThreadEntity({
        createdAt: FIXED_TIME,
        lastMessageAt: null,
        messageCount: 0,
        ownerUserId,
        status: "active",
        threadId: "thread.remote.a",
        title: "Remote desktop-visible thread",
        updatedAt: FIXED_TIME,
      }),
    )

    expect(collectionA.get("thread.remote.a")?.title).toBe(
      "Remote desktop-visible thread",
    )
    await createTx.isPersisted.promise

    await waitFor(
      () =>
        collectionB.get("thread.remote.a")?.title ===
        "Remote desktop-visible thread",
      "client B observed client A thread through live delta",
    )
    expect(clientA.session.pending()).toEqual([])
    expect(clientB.session.pending()).toEqual([])

    const appendTx = messagesA.insert(
      decodeChatMessageEntity({
        authorUserId: ownerUserId,
        body: "hello from client A",
        createdAt: UPDATED_TIME,
        deletedAt: null,
        messageId: "chat-message.remote.a.1",
        threadId: "thread.remote.a",
        updatedAt: UPDATED_TIME,
      }),
    )
    expect(messagesA.get("chat-message.remote.a.1")?.body).toBe(
      "hello from client A",
    )
    await appendTx.isPersisted.promise

    await waitFor(
      () => messagesB.get("chat-message.remote.a.1")?.body === "hello from client A",
      "client B observed appended message through thread scope",
    )
    expect(
      chatMessagesForTranscript(messagesB.values()).map(message => message.messageId),
    ).toEqual(["chat-message.remote.a.1"])
    await waitFor(
      () => collectionB.get("thread.remote.a")?.messageCount === 1,
      "client B observed thread metadata update after append",
    )

    const renameTx = collectionA.update("thread.remote.a", draft => {
      draft.title = "Renamed from client A"
    })
    await renameTx.isPersisted.promise

    await waitFor(
      () => collectionB.get("thread.remote.a")?.title === "Renamed from client A",
      "client B observed rename through live delta",
    )

    server.commitChatThread(
      decodeChatThreadEntity({
        createdAt: FIXED_TIME,
        lastMessageAt: null,
        messageCount: 0,
        ownerUserId,
        status: "active",
        threadId: "thread.older",
        title: "Older thread",
        updatedAt: "2026-07-04T19:59:00.000Z",
      }),
    )
    await waitFor(
      () => collectionB.get("thread.older")?.title === "Older thread",
      "older live delta visible",
    )

    expect(
      chatThreadsForSidebar(collectionB.values()).map(thread => thread.threadId),
    ).toEqual(["thread.remote.a", "thread.older"])
    expect(server.pushCalls.flat().map(call => call.name)).toEqual([
      CHAT_CREATE_THREAD_MUTATOR_NAME,
      CHAT_APPEND_MESSAGE_MUTATOR_NAME,
      CHAT_RENAME_THREAD_MUTATOR_NAME,
    ])
  })
})

describe("runtimeTurnKhalaSyncCollectionOptions / runtimeEventKhalaSyncCollectionOptions (#8425)", () => {
  const threadId = "thread.remote.runtime.1"

  const turnRow = (patch: Partial<RuntimeTurnEntity> = {}): RuntimeTurnEntity =>
    decodeRuntimeTurnEntity({
      createdAt: FIXED_TIME,
      eventCount: 1,
      lane: "codex_app_server",
      latestIntentId: null,
      ownerUserId: "user-chat-owner",
      settledAt: null,
      startedAt: FIXED_TIME,
      status: "completed",
      threadId,
      turnId: "turn.remote.1",
      updatedAt: FIXED_TIME,
      ...patch,
    })

  const textDeltaEventRow = (patch: Partial<RuntimeEventEntity> = {}): RuntimeEventEntity =>
    decodeRuntimeEventEntity({
      createdAt: FIXED_TIME,
      event: {
        causalityRefs: [],
        chunkId: "chunk.remote.1",
        eventId: "event.remote.1",
        kind: "text.delta",
        messageId: "message.remote.1",
        observedAt: FIXED_TIME,
        redactionClass: "private_ref",
        schema: "openagents.khala_runtime_event.v1",
        sequence: 1,
        source: { lane: "codex_app_server", surface: "server" },
        text: "codex mobile-to-desktop-test-ok",
        threadId,
        turnId: "turn.remote.1",
        visibility: "private",
      },
      eventId: "event.remote.1",
      kind: "text.delta",
      observedAt: FIXED_TIME,
      ownerUserId: "user-chat-owner",
      sequence: 1,
      threadId,
      turnId: "turn.remote.1",
      ...patch,
    })

  test("reads server-authored runtime_turn/runtime_event rows without any local mutator — a mobile-dispatched turn's reply is visible read-only", async () => {
    const server = new FleetFakeServer()
    server.commitRuntimeTurn(turnRow())
    server.commitRuntimeEvent(textDeltaEventRow())
    const harness = await makeHarness(server, { mutators: [] })

    const turns = createRuntimeTurnCollection(harness, threadId)
    const events = createRuntimeEventCollection(harness, threadId)

    await waitFor(() => turns.isReady() && events.isReady(), "runtime collections ready")

    expect(turns.get("turn.remote.1")).toMatchObject({
      lane: "codex_app_server",
      status: "completed",
      turnId: "turn.remote.1",
    })
    expect(events.get("event.remote.1")?.event).toMatchObject({
      kind: "text.delta",
      text: "codex mobile-to-desktop-test-ok",
    })
  })

  test("live updates land without a restart — a turn transitioning queued -> completed is visible as it happens", async () => {
    const server = new FleetFakeServer()
    server.commitRuntimeTurn(turnRow({ settledAt: null, startedAt: null, status: "queued" }))
    const harness = await makeHarness(server, { mutators: [] })
    const turns = createRuntimeTurnCollection(harness, threadId)
    await waitFor(() => turns.isReady(), "runtime_turn collection ready")
    expect(turns.get("turn.remote.1")?.status).toBe("queued")

    server.commitRuntimeTurn(turnRow({ status: "completed" }))
    await waitFor(
      () => turns.get("turn.remote.1")?.status === "completed",
      "live turn-status update visible without restart",
    )
  })

  test("never dispatches a mutation for these read-only collections", async () => {
    const server = new FleetFakeServer()
    server.commitRuntimeTurn(turnRow())
    const harness = await makeHarness(server, { mutators: [] })
    const turns = createRuntimeTurnCollection(harness, threadId)
    await waitFor(() => turns.isReady(), "runtime_turn collection ready")
    expect(server.pushCalls).toEqual([])
  })
})
