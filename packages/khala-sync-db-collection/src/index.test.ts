import { createCollection } from "@tanstack/db"
import {
  BootstrapEntity,
  type BootstrapRequest,
  BootstrapResponse,
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
  decodeFleetRunEntity,
  encodeFleetRunEntity,
  FLEET_RUN_ENTITY_TYPE,
  fleetRunScope,
  type FleetRunEntity,
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
  createKhalaSyncMutationTracker,
  FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME,
  fleetRunKhalaSyncCollectionOptions,
  fleetSetDesiredSlotsClientMutator,
  type FleetSetDesiredSlotsArgs,
  KhalaSyncDbCollectionError,
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

const makeHarness = async (server: FleetFakeServer) => {
  const tracker = createKhalaSyncMutationTracker()
  const store = openKhalaSyncStore(":memory:")
  cleanups.push(() => Effect.runSync(Effect.ignore(store.close())))
  const overlay = Effect.runSync(
    createOverlay(store, [
      fleetSetDesiredSlotsClientMutator,
      rejectingDesiredSlotsMutator,
    ]),
  )
  const session = createKhalaSyncSession(
    {
      authToken: () => "test-token",
      baseUrl: "http://fake.test",
      clientGroupId: ClientGroupId.make("cg_fleet"),
      clientId: ClientId.make("c_fleet"),
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
