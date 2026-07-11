/**
 * Dual-client chat.composeTurn proof (Effect Native #64 residual).
 *
 * Two real `createKhalaSyncSession` clients (desktop + mobile clientIds)
 * share one FakeSyncServer over the real transport seam. Each mutates
 * `chat.composeTurn`; the server commits post-image changelog entries and
 * fans `DeltaFrame`s so both sessions converge on the same transcript
 * entities — the Khala Sync protocol algebra, not a hand-rolled hub.
 *
 * This is still an **in-process fake server** (no Cloud SQL / live staging).
 * It is strictly stronger than a framework-only memory hub: it exercises
 * overlay mutators, push, live socket apply, and dual durable stores from
 * `@openagentsinc/khala-sync-client`.
 */
import {
  BootstrapEntity,
  type BootstrapRequest,
  BootstrapResponse,
  ChangelogEntry,
  ClientGroupId,
  ClientId,
  DeltaFrame,
  EntityId,
  EntityType,
  type LiveFrame,
  LogPage,
  MutationEnvelope,
  MutationId,
  MutationResult as MutationResultClass,
  MustRefetchFrame,
  MutatorName,
  PushResponse,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
  canonicalJson,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { type ClientMutator, createOverlay } from "./overlay.js"
import { createKhalaSyncSession } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"
import {
  type KhalaSyncTransport,
  KhalaSyncTransportError,
  type LiveSocketHandlers,
} from "./transport.js"

// Deterministic scheduler yield: the proof injects this into every retry loop
// and never waits on wall-clock timers.
const tick = (): Promise<void> => Promise.resolve()

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

const FIXED_TIME = "2026-07-09T00:00:00.000Z"
const threadScope = SyncScope.make("scope.thread.cross-app-proof")
const fleetScope = SyncScope.make("scope.fleet_run.cross-app-fleet-proof")

const cleanups: Array<() => void> = []
afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()!()
})

interface SocketRecord {
  handlers: LiveSocketHandlers
  open: boolean
}

const networkError = (): KhalaSyncTransportError =>
  new KhalaSyncTransportError("network", true, "fake cross-app network failure")

/** Minimal SPEC §3 server that understands chat.composeTurn for dual clients. */
class DualClientChatServer {
  readonly logs = new Map<SyncScope, Array<ChangelogEntry>>()
  /** Multiple live sockets per scope (desktop + mobile). */
  readonly sockets = new Map<SyncScope, Array<SocketRecord>>()
  readonly clientLast = new Map<string, number>()
  readonly pushCalls: Array<ReadonlyArray<MutationEnvelope>> = []
  dropNextPushAck = 0
  offline = false

  logOf(scope: SyncScope): Array<ChangelogEntry> {
    const existing = this.logs.get(scope)
    if (existing !== undefined) return existing
    const created: Array<ChangelogEntry> = []
    this.logs.set(scope, created)
    return created
  }

  private lastVersion(scope: SyncScope): number {
    const log = this.logOf(scope)
    return log.length === 0 ? 0 : Number(log[log.length - 1]!.version)
  }

  emitFrame(scope: SyncScope, frame: LiveFrame): void {
    const sockets = this.sockets.get(scope) ?? []
    for (const socket of sockets) {
      if (socket.open) socket.handlers.onFrame(frame)
    }
  }

  commit(
    scope: SyncScope,
    change: {
      readonly entityType: string
      readonly entityId: string
      readonly postImageJson: string
    },
    mutationRef: string,
    options: { readonly emit?: boolean } = {},
  ): number {
    const version = this.lastVersion(scope) + 1
    const entry = new ChangelogEntry({
      scope,
      version: SyncVersion.make(version),
      entityType: EntityType.make(change.entityType),
      entityId: EntityId.make(change.entityId),
      op: "upsert",
      postImageJson: change.postImageJson,
      mutationRef,
      committedAt: FIXED_TIME,
    })
    this.logOf(scope).push(entry)
    if (options.emit !== false) {
      this.emitFrame(
        scope,
        new DeltaFrame({
          scope,
          entries: [entry],
          cursor: SyncVersion.make(version),
        }) as LiveFrame,
      )
    }
    return version
  }

  tombstone(
    scope: SyncScope,
    change: { readonly entityType: string; readonly entityId: string },
    mutationRef: string,
  ): number {
    const version = this.lastVersion(scope) + 1
    const entry = new ChangelogEntry({
      scope,
      version: SyncVersion.make(version),
      entityType: EntityType.make(change.entityType),
      entityId: EntityId.make(change.entityId),
      op: "delete",
      mutationRef,
      committedAt: FIXED_TIME,
    })
    this.logOf(scope).push(entry)
    this.emitFrame(
      scope,
      new DeltaFrame({ scope, entries: [entry], cursor: SyncVersion.make(version) }) as LiveFrame,
    )
    return version
  }

  /** Replace the server log as after a scope-owning process restart. */
  replaceScope(
    scope: SyncScope,
    change: {
      readonly entityType: string
      readonly entityId: string
      readonly postImageJson: string
    },
  ): void {
    this.logs.set(scope, [])
    this.commit(scope, change, "mutation.server.restart.1")
    this.emitFrame(
      scope,
      new MustRefetchFrame({ scope, reason: "scope_reset" }) as LiveFrame,
    )
  }

  bootstrap(request: BootstrapRequest): BootstrapResponse {
    const scope = request.scope
    const cursor = this.lastVersion(scope)
    const entities = this.logOf(scope)
      .filter((e) => e.op === "upsert" && e.postImageJson !== undefined)
      .map(
        (e) =>
          new BootstrapEntity({
            entityType: e.entityType,
            entityId: e.entityId,
            postImageJson: e.postImageJson!,
          }),
      )
    return new BootstrapResponse({
      protocolVersion: 1,
      scope,
      entities,
      cursor: SyncVersionWatermark.make(cursor),
    })
  }

  logPage(scope: SyncScope, cursor: number, limit: number): LogPage {
    const after = this.logOf(scope).filter((entry) => entry.version > cursor)
    const included = after.slice(0, limit)
    const next =
      included.length === 0
        ? cursor
        : Number(included[included.length - 1]!.version)
    return new LogPage({
      protocolVersion: 1,
      scope,
      entries: included,
      nextCursor: SyncVersionWatermark.make(next),
      upToDate: next >= this.lastVersion(scope),
    })
  }

  push(mutations: ReadonlyArray<MutationEnvelope>, clientKey: string): PushResponse {
    if (this.offline) throw networkError()
    this.pushCalls.push([...mutations])
    let last = this.clientLast.get(clientKey) ?? 0
    const results = []
    for (const mutation of mutations) {
      if (mutation.mutationId <= last) {
        results.push(
          new MutationResultClass({
            mutationId: mutation.mutationId,
            status: "duplicate",
          }),
        )
        continue
      }
      if (mutation.name !== "chat.composeTurn") {
        results.push(
          new MutationResultClass({
            mutationId: mutation.mutationId,
            status: "rejected",
            errorCode: "mutation_rejected",
            errorMessageSafe: `unknown mutator ${mutation.name}`,
          }),
        )
        last = mutation.mutationId
        continue
      }
      const args = JSON.parse(mutation.argsJson) as {
        threadId: string
        text: string
        client: "desktop" | "mobile"
        author: string
        id: string
      }
      this.commit(
        SyncScope.make(args.threadId.startsWith("scope.")
          ? args.threadId
          : `scope.thread.${args.threadId}`),
        {
          entityType: "chat_turn_event",
          entityId: args.id,
          postImageJson: canonicalJson({
            id: args.id,
            threadId: args.threadId,
            role: "user",
            author: args.author,
            text: args.text,
            client: args.client,
            committedAt: FIXED_TIME,
          }),
        },
        `mut.${clientKey}.${mutation.mutationId}`,
      )
      results.push(
        new MutationResultClass({
          mutationId: mutation.mutationId,
          status: "applied",
        }),
      )
      last = mutation.mutationId
    }
    this.clientLast.set(clientKey, last)
    if (this.dropNextPushAck > 0) {
      this.dropNextPushAck -= 1
      throw networkError()
    }
    return new PushResponse({
      protocolVersion: 1,
      results,
      lastMutationId: last,
    })
  }

  connect(scope: SyncScope, _cursor: number, handlers: LiveSocketHandlers) {
    const record: SocketRecord = { handlers, open: true }
    const existing = this.sockets.get(scope) ?? []
    existing.push(record)
    this.sockets.set(scope, existing)
    return {
      close: () => {
        record.open = false
      },
    }
  }
}

const transportOf = (server: DualClientChatServer): KhalaSyncTransport => {
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
    bootstrap: (request) => attempt(() => server.bootstrap(request)),
    logPage: (scope, cursor, limit) =>
      attempt(() => server.logPage(scope, cursor, limit)),
    push: (request) =>
      attempt(() =>
        server.push(
          request.mutations,
          `${request.clientGroupId}:${request.clientId}`,
        ),
      ),
    connectLive: (scope, cursor, handlers) =>
      attempt(() => server.connect(scope, cursor, handlers)),
  }
}

interface ComposeArgs {
  readonly threadId: string
  readonly text: string
  readonly client: "desktop" | "mobile"
  readonly author: string
  readonly id: string
}

const composeTurn: ClientMutator<ComposeArgs> = {
  name: MutatorName.make("chat.composeTurn"),
  apply: (args) => [
    {
      kind: "upsert",
      scope: SyncScope.make(
        args.threadId.startsWith("scope.")
          ? args.threadId
          : `scope.thread.${args.threadId}`,
      ),
      entityType: "chat_turn_event",
      entityId: args.id,
      postImageJson: canonicalJson({
        id: args.id,
        threadId: args.threadId,
        role: "user",
        author: args.author,
        text: args.text,
        client: args.client,
        committedAt: FIXED_TIME,
      }),
    },
  ],
}

const makeClient = (
  server: DualClientChatServer,
  clientId: string,
) => {
  const store = openKhalaSyncStore(":memory:")
  cleanups.push(() => Effect.runSync(Effect.ignore(store.close())))
  const overlay = Effect.runSync(createOverlay(store, [composeTurn]))
  const session = createKhalaSyncSession(
    {
      baseUrl: "http://fake.test",
      clientGroupId: ClientGroupId.make("cg_cross_app"),
      clientId: ClientId.make(clientId),
      schemaVersion: SyncSchemaVersion.make(1),
      authToken: () => "test-token",
    },
    store,
    overlay,
    transportOf(server),
    {
      sleep: () => tick(),
      random: () => 0,
      backoffBaseMs: 1,
      backoffMaxMs: 4,
      maxBootstrapAttempts: 3,
      logPageLimit: 50,
      pushBatchSize: 10,
    },
  )
  cleanups.push(() => Effect.runSync(session.close()))
  return {
    session,
    overlay,
    store,
    listTurns: () => {
      const view = Effect.runSync(overlay.read(threadScope))
      return view
        .list("chat_turn_event")
        .map((row) => JSON.parse(row.postImageJson) as {
          id: string
          text: string
          client: string
          author: string
        })
        .sort((a, b) => a.id.localeCompare(b.id))
    },
    listFleetRuns: () =>
      Effect.runSync(overlay.read(fleetScope))
        .list("fleet_run")
        .map((row) => JSON.parse(row.postImageJson) as { readonly runId: string }),
  }
}

describe("cross-app chat.composeTurn over real khala-sync-client sessions", () => {
  test("desktop mutate lands on mobile and mobile mutate lands on desktop via live hub fan-out", async () => {
    const server = new DualClientChatServer()
    const desktop = makeClient(server, "c_desktop")
    const mobile = makeClient(server, "c_mobile")

    await Effect.runPromise(desktop.session.subscribe(threadScope))
    await Effect.runPromise(mobile.session.subscribe(threadScope))
    await waitFor(
      () =>
        desktop.session.state(threadScope).phase === "live" &&
        mobile.session.state(threadScope).phase === "live",
      "both clients live",
    )

    await Effect.runPromise(
      desktop.session.mutate(composeTurn, {
        threadId: "scope.thread.cross-app-proof",
        text: "hello from desktop over Khala Sync",
        client: "desktop",
        author: "Desktop",
        id: "turn-desktop-1",
      }),
    )
    await waitFor(
      () => mobile.listTurns().some((t) => t.id === "turn-desktop-1"),
      "mobile sees desktop turn",
    )

    await Effect.runPromise(
      mobile.session.mutate(composeTurn, {
        threadId: "scope.thread.cross-app-proof",
        text: "hello from mobile over Khala Sync",
        client: "mobile",
        author: "Mobile",
        id: "turn-mobile-1",
      }),
    )
    await waitFor(
      () => desktop.listTurns().some((t) => t.id === "turn-mobile-1"),
      "desktop sees mobile turn",
    )

    const desktopTurns = desktop.listTurns()
    const mobileTurns = mobile.listTurns()
    expect(desktopTurns.map((t) => t.id).sort()).toEqual(
      mobileTurns.map((t) => t.id).sort(),
    )
    expect(desktopTurns.map((t) => t.text).sort()).toEqual([
      "hello from desktop over Khala Sync",
      "hello from mobile over Khala Sync",
    ])
    expect(server.pushCalls.length).toBeGreaterThanOrEqual(2)
    // Both clients applied both post-images (optimistic + confirmed).
    expect(desktopTurns).toHaveLength(2)
    expect(mobileTurns).toHaveLength(2)
  })

  test("lost desktop acknowledgement retries the same MutationId and confirms one turn on both clients", async () => {
    const server = new DualClientChatServer()
    const desktop = makeClient(server, "c_desktop_lost_ack")
    const mobile = makeClient(server, "c_mobile_lost_ack")

    await Effect.runPromise(desktop.session.subscribe(threadScope))
    await Effect.runPromise(mobile.session.subscribe(threadScope))
    await waitFor(
      () =>
        desktop.session.state(threadScope).phase === "live" &&
        mobile.session.state(threadScope).phase === "live",
      "both clients live before lost ack",
    )

    server.dropNextPushAck = 1
    const mutationId = await Effect.runPromise(
      desktop.session.mutate(composeTurn, {
        threadId: "scope.thread.cross-app-proof",
        text: "lost acknowledgement stays one command",
        client: "desktop",
        author: "Desktop",
        id: "turn-desktop-lost-ack",
      }),
    )

    expect(mutationId).toBe(MutationId.make(1))
    await waitFor(
      () => server.pushCalls.length >= 2,
      "same command retried after lost acknowledgement",
    )
    await waitFor(
      () =>
        desktop.session.pending().length === 0 &&
        mobile.listTurns().some((turn) => turn.id === "turn-desktop-lost-ack"),
      "lost acknowledgement reconciled",
    )

    expect(
      server.pushCalls
        .slice(0, 2)
        .map((call) => call.map((mutation) => mutation.mutationId)),
    ).toEqual([[MutationId.make(1)], [MutationId.make(1)]])
    expect(
      server
        .logOf(threadScope)
        .filter((entry) => entry.entityId === "turn-desktop-lost-ack"),
    ).toHaveLength(1)
    expect(
      desktop.listTurns().filter((turn) => turn.id === "turn-desktop-lost-ack"),
    ).toHaveLength(1)
    expect(
      mobile.listTurns().filter((turn) => turn.id === "turn-desktop-lost-ack"),
    ).toHaveLength(1)
  })

  test("offline mobile enqueue stays pending until recovery, then confirms on desktop and mobile", async () => {
    const server = new DualClientChatServer()
    const desktop = makeClient(server, "c_desktop_offline")
    const mobile = makeClient(server, "c_mobile_offline")

    await Effect.runPromise(desktop.session.subscribe(threadScope))
    await Effect.runPromise(mobile.session.subscribe(threadScope))
    await waitFor(
      () =>
        desktop.session.state(threadScope).phase === "live" &&
        mobile.session.state(threadScope).phase === "live",
      "both clients live before offline enqueue",
    )

    server.offline = true
    await Effect.runPromise(
      mobile.session.mutate(composeTurn, {
        threadId: "scope.thread.cross-app-proof",
        text: "queued while mobile is offline",
        client: "mobile",
        author: "Mobile",
        id: "turn-mobile-offline",
      }),
    )
    await tick()
    await tick()

    expect(mobile.session.pending().map((mutation) => mutation.mutationId)).toEqual([
      MutationId.make(1),
    ])
    expect(mobile.listTurns().some((turn) => turn.id === "turn-mobile-offline")).toBe(true)
    expect(desktop.listTurns().some((turn) => turn.id === "turn-mobile-offline")).toBe(false)
    expect(server.pushCalls).toEqual([])

    server.offline = false
    await waitFor(
      () =>
        mobile.session.pending().length === 0 &&
        desktop.listTurns().some((turn) => turn.id === "turn-mobile-offline"),
      "offline mobile turn confirmed on desktop after recovery",
    )

    expect(server.pushCalls.map((call) => call.map((mutation) => mutation.mutationId))).toEqual([
      [MutationId.make(1)],
    ])
    expect(
      server
        .logOf(threadScope)
        .filter((entry) => entry.entityId === "turn-mobile-offline"),
    ).toHaveLength(1)
    expect(
      mobile.listTurns().filter((turn) => turn.id === "turn-mobile-offline"),
    ).toHaveLength(1)
    expect(
      desktop.listTurns().filter((turn) => turn.id === "turn-mobile-offline"),
    ).toHaveLength(1)
  })

  test("duplicate and stale live deltas keep desktop and mobile on one stable confirmed transcript", async () => {
    const server = new DualClientChatServer()
    const desktop = makeClient(server, "c_desktop_ordering")
    const mobile = makeClient(server, "c_mobile_ordering")

    await Effect.runPromise(desktop.session.subscribe(threadScope))
    await Effect.runPromise(mobile.session.subscribe(threadScope))
    await waitFor(
      () =>
        desktop.session.state(threadScope).phase === "live" &&
        mobile.session.state(threadScope).phase === "live",
      "both clients live before ordering trace",
    )

    server.commit(
      threadScope,
      {
        entityType: "chat_turn_event",
        entityId: "turn-ordering-1",
        postImageJson: canonicalJson({
          id: "turn-ordering-1",
          threadId: "scope.thread.cross-app-proof",
          role: "user",
          author: "Desktop",
          text: "first ordered event",
          client: "desktop",
          committedAt: FIXED_TIME,
        }),
      },
      "mutation.server.ordering.1",
    )
    await waitFor(
      () =>
        desktop.listTurns().some((turn) => turn.id === "turn-ordering-1") &&
        mobile.listTurns().some((turn) => turn.id === "turn-ordering-1"),
      "both clients have first ordered turn",
    )
    expect(Effect.runSync(desktop.store.cursor(threadScope))).toBe(SyncVersion.make(1))
    expect(Effect.runSync(mobile.store.cursor(threadScope))).toBe(SyncVersion.make(1))

    server.commit(
      threadScope,
      {
        entityType: "chat_turn_event",
        entityId: "turn-ordering-2",
        postImageJson: canonicalJson({
          id: "turn-ordering-2",
          threadId: "scope.thread.cross-app-proof",
          role: "user",
          author: "Mobile",
          text: "second ordered event",
          client: "mobile",
          committedAt: FIXED_TIME,
        }),
      },
      "mutation.server.ordering.2",
      { emit: false },
    )

    const v1Entries = server
      .logOf(threadScope)
      .filter((entry) => Number(entry.version) === 1)
    const v2Entries = server
      .logOf(threadScope)
      .filter((entry) => Number(entry.version) === 2)

    server.emitFrame(
      threadScope,
      new DeltaFrame({
        scope: threadScope,
        entries: v2Entries,
        cursor: SyncVersion.make(2),
      }) as LiveFrame,
    )
    server.emitFrame(
      threadScope,
      new DeltaFrame({
        scope: threadScope,
        entries: v2Entries,
        cursor: SyncVersion.make(2),
      }) as LiveFrame,
    )
    server.emitFrame(
      threadScope,
      new DeltaFrame({
        scope: threadScope,
        entries: v1Entries,
        cursor: SyncVersion.make(1),
      }) as LiveFrame,
    )

    await waitFor(
      () =>
        Effect.runSync(desktop.store.cursor(threadScope)) === SyncVersion.make(2) &&
        Effect.runSync(mobile.store.cursor(threadScope)) === SyncVersion.make(2),
      "both clients advanced to ordering cursor",
    )
    await tick()
    await tick()

    const expectedIds = ["turn-ordering-1", "turn-ordering-2"]
    expect(desktop.listTurns().map((turn) => turn.id).sort()).toEqual(expectedIds)
    expect(mobile.listTurns().map((turn) => turn.id).sort()).toEqual(expectedIds)
    expect(Effect.runSync(desktop.store.cursor(threadScope))).toBe(SyncVersion.make(2))
    expect(Effect.runSync(mobile.store.cursor(threadScope))).toBe(SyncVersion.make(2))
  })

  test("both clients converge on a fleet projection tombstone and preserve its cursor after restart", async () => {
    const server = new DualClientChatServer()
    const desktop = makeClient(server, "c_desktop_fleet")
    const mobile = makeClient(server, "c_mobile_fleet")

    await Effect.runPromise(desktop.session.subscribe(fleetScope))
    await Effect.runPromise(mobile.session.subscribe(fleetScope))
    await waitFor(
      () =>
        desktop.session.state(fleetScope).phase === "live" &&
        mobile.session.state(fleetScope).phase === "live",
      "both fleet projections live",
    )

    const run = {
      runId: "fleet-run.pylon.supervisor.crossapp",
      status: "running",
      desiredSlots: 1,
      workerKind: "codex",
      startedAt: FIXED_TIME,
      counters: {
        workUnitsTotal: 1,
        activeAssignments: 1,
        completedAssignments: 0,
        failedAssignments: 0,
        blockedAssignments: 0,
      },
      updatedAt: FIXED_TIME,
    }
    server.commit(
      fleetScope,
      {
        entityType: "fleet_run",
        entityId: run.runId,
        postImageJson: canonicalJson(run),
      },
      "mutation.server.fleet-run.1",
    )
    await waitFor(
      () =>
        desktop.listFleetRuns()[0]?.runId === run.runId &&
        mobile.listFleetRuns()[0]?.runId === run.runId,
      "both clients see fleet run",
    )
    expect(Effect.runSync(desktop.store.cursor(fleetScope))).toBe(SyncVersion.make(1))
    expect(Effect.runSync(mobile.store.cursor(fleetScope))).toBe(SyncVersion.make(1))

    server.tombstone(
      fleetScope,
      { entityType: "fleet_run", entityId: run.runId },
      "mutation.server.fleet-run.2",
    )
    await waitFor(
      () => desktop.listFleetRuns().length === 0 && mobile.listFleetRuns().length === 0,
      "fleet tombstone on both clients",
    )
    expect(Effect.runSync(desktop.store.cursor(fleetScope))).toBe(SyncVersion.make(2))
    expect(Effect.runSync(mobile.store.cursor(fleetScope))).toBe(SyncVersion.make(2))

    const restartedOverlay = Effect.runSync(createOverlay(desktop.store, [composeTurn]))
    expect(Effect.runSync(restartedOverlay.read(fleetScope)).list("fleet_run")).toEqual([])
    expect(Effect.runSync(desktop.store.cursor(fleetScope))).toBe(SyncVersion.make(2))
  })

  test("both clients discard pre-restart fleet rows on scope_reset and converge on the replacement snapshot", async () => {
    const server = new DualClientChatServer()
    const desktop = makeClient(server, "c_desktop_restart")
    const mobile = makeClient(server, "c_mobile_restart")
    const oldRun = {
      runId: "fleet-run.pylon.pre-restart",
      status: "running",
      desiredSlots: 1,
      workerKind: "codex",
      startedAt: FIXED_TIME,
      counters: { workUnitsTotal: 1, activeAssignments: 1, completedAssignments: 0, failedAssignments: 0, blockedAssignments: 0 },
      updatedAt: FIXED_TIME,
    }
    const replacementRun = {
      ...oldRun,
      runId: "fleet-run.pylon.after-restart",
      status: "paused",
      desiredSlots: 0,
    }

    await Effect.runPromise(desktop.session.subscribe(fleetScope))
    await Effect.runPromise(mobile.session.subscribe(fleetScope))
    await waitFor(
      () => desktop.session.state(fleetScope).phase === "live" && mobile.session.state(fleetScope).phase === "live",
      "both clients live before restart",
    )
    server.commit(fleetScope, {
      entityType: "fleet_run",
      entityId: oldRun.runId,
      postImageJson: canonicalJson(oldRun),
    }, "mutation.server.pre-restart")
    await waitFor(
      () => desktop.listFleetRuns()[0]?.runId === oldRun.runId && mobile.listFleetRuns()[0]?.runId === oldRun.runId,
      "both clients see pre-restart run",
    )

    server.replaceScope(fleetScope, {
      entityType: "fleet_run",
      entityId: replacementRun.runId,
      postImageJson: canonicalJson(replacementRun),
    })
    await waitFor(
      () =>
        desktop.session.state(fleetScope).phase === "live" &&
        mobile.session.state(fleetScope).phase === "live" &&
        desktop.listFleetRuns()[0]?.runId === replacementRun.runId &&
        mobile.listFleetRuns()[0]?.runId === replacementRun.runId,
      "both clients converge after restart",
    )
    expect(desktop.listFleetRuns().map((run) => run.runId)).toEqual([replacementRun.runId])
    expect(mobile.listFleetRuns().map((run) => run.runId)).toEqual([replacementRun.runId])
    expect(Effect.runSync(desktop.store.cursor(fleetScope))).toBe(SyncVersion.make(1))
    expect(Effect.runSync(mobile.store.cursor(fleetScope))).toBe(SyncVersion.make(1))
  })
})
