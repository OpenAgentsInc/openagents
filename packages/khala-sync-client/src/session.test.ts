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
  MustRefetchFrame,
  MutationEnvelope,
  MutationId,
  type MutationResult,
  MutationResult as MutationResultClass,
  MutatorName,
  PushResponse,
  SyncError,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
  deviceLocalScope,
  LocalIdentityRef,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { type ClientMutator, createOverlay } from "./overlay.js"
import {
  createKhalaSyncSession,
  computeBackoffMs,
  type ConnectFailureSignal,
} from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"
import {
  type KhalaSyncTransport,
  KhalaSyncTransportError,
  type LiveSocketHandlers,
} from "./transport.js"

/**
 * KS-5.3 session tests: a deterministic FAKE transport/server (no real
 * network, no real WebSockets). Time is injected — `sleep` yields one
 * scheduler turn regardless of the requested delay and `random` is constant —
 * so backoff paths run instantly and no logic reads a wall clock.
 */

// ---------------------------------------------------------------------------
// Deterministic helpers
// ---------------------------------------------------------------------------

const tick = (): Promise<void> => new Promise(resolve => setImmediate(resolve))

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

const FIXED_TIME = "2026-07-04T00:00:00.000Z"

const scopeA = SyncScope.make("scope.team.alpha")

const cleanups: Array<() => void> = []
afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()!()
})

// ---------------------------------------------------------------------------
// Fake server + transport (implements the SPEC §3 semantics in-memory)
// ---------------------------------------------------------------------------

interface FakeChange {
  readonly entityType: string
  readonly entityId: string
  readonly op: "upsert" | "delete"
  readonly postImageJson?: string
}

interface SocketRecord {
  handlers: LiveSocketHandlers
  open: boolean
}

const networkError = (): KhalaSyncTransportError =>
  new KhalaSyncTransportError("network", true, "fake network failure")

class FakeSyncServer {
  readonly logs = new Map<SyncScope, Array<ChangelogEntry>>()
  readonly sockets = new Map<SyncScope, SocketRecord>()
  readonly clientLast = new Map<string, number>()
  private readonly pageStash = new Map<
    string,
    { entities: ReadonlyArray<FakeChange>; cursor: number; offset: number }
  >()
  private tokenCounter = 2

  snapshotLag = 0
  bootstrapPageSize = Number.POSITIVE_INFINITY
  offline = false
  /** Non-retryable push fault (e.g. auth): the push loop must PARK, not spin. */
  pushTerminalFault = false
  /** Simulate a response lost after the server has durably applied the push. */
  dropNextPushAck = 0
  /**
   * ST-7 (#8513): 401 on `connectLive` — the WS-auth class from the mobile
   * incident (bootstrap/log succeed over HTTP header auth while the socket
   * upgrade is refused as unauthenticated). The session must park the
   * scope after the bounded rotation budget, never spin forever.
   */
  connectAuthFault = false
  /**
   * Scopes whose reads now 403 with the typed `unauthorized_scope`
   * SyncError — the KS-7.1 resolver's post-revocation answer on
   * bootstrap/log/connect (khala-sync-scope-auth, #8305).
   */
  readonly deniedScopes = new Set<SyncScope>()
  readonly failNext = { bootstrap: 0, logPage: 0, push: 0, connect: 0 }
  /** 1-based attempt indexes to fail (checked before failNext). */
  readonly failOn: { logPage: Set<number> } = { logPage: new Set() }

  readonly attempts = { bootstrap: 0, logPage: 0, push: 0, connect: 0 }
  readonly bootstrapCalls: Array<BootstrapRequest> = []
  readonly logPageCalls: Array<{ scope: SyncScope; cursor: number; limit: number }> = []
  readonly pushCalls: Array<ReadonlyArray<MutationEnvelope>> = []
  readonly connectCalls: Array<{ scope: SyncScope; cursor: number }> = []

  private denyIfRevoked(scope: SyncScope): void {
    if (this.deniedScopes.has(scope)) {
      throw new KhalaSyncTransportError(
        "sync_error",
        false,
        "khala-sync server error unauthorized_scope: This user cannot read the requested scope.",
        {
          status: 403,
          syncError: new SyncError({
            code: "unauthorized_scope",
            messageSafe: "This user cannot read the requested scope.",
            retryable: false,
          }),
        },
      )
    }
  }

  private maybeFail(kind: keyof typeof this.attempts): void {
    this.attempts[kind] += 1
    if (kind === "push" && this.pushTerminalFault) {
      throw new KhalaSyncTransportError("http_status", false, "fake auth failure", {
        status: 401,
      })
    }
    if (this.offline) throw networkError()
    if (kind === "logPage" && this.failOn.logPage.has(this.attempts.logPage)) {
      throw networkError()
    }
    if (this.failNext[kind] > 0) {
      this.failNext[kind] -= 1
      throw networkError()
    }
  }

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

  /** Server-side state fold through `throughVersion` (post-images win by version). */
  fold(scope: SyncScope, throughVersion: number): Array<FakeChange> {
    const state = new Map<string, FakeChange>()
    for (const entry of this.logOf(scope)) {
      if (entry.version > throughVersion) continue
      const key = `${entry.entityType}\0${entry.entityId}`
      if (entry.op === "delete") state.delete(key)
      else {
        state.set(key, {
          entityType: entry.entityType,
          entityId: entry.entityId,
          op: "upsert",
          postImageJson: entry.postImageJson!,
        })
      }
    }
    return [...state.values()].sort((a, b) =>
      `${a.entityType}/${a.entityId}` < `${b.entityType}/${b.entityId}` ? -1 : 1,
    )
  }

  /** Commit one transaction (one version) and fan out a DeltaFrame. */
  commit(
    scope: SyncScope,
    changes: ReadonlyArray<FakeChange>,
    mutationRef?: string,
    emit = true,
  ): number {
    const version = this.lastVersion(scope) + 1
    const entries = changes.map(
      (change) =>
        new ChangelogEntry({
          scope,
          version: SyncVersion.make(version),
          entityType: EntityType.make(change.entityType),
          entityId: EntityId.make(change.entityId),
          op: change.op,
          ...(change.postImageJson !== undefined
            ? { postImageJson: change.postImageJson }
            : {}),
          ...(mutationRef !== undefined ? { mutationRef } : {}),
          committedAt: FIXED_TIME,
        }),
    )
    this.logOf(scope).push(...entries)
    if (emit) {
      this.emitFrame(
        scope,
        new DeltaFrame({ scope, entries, cursor: SyncVersion.make(version) }),
      )
    }
    return version
  }

  /** Wipe the scope's log and re-seed (versions restart) — a scope_reset. */
  replaceScope(scope: SyncScope, changeSets: ReadonlyArray<ReadonlyArray<FakeChange>>): void {
    this.logs.set(scope, [])
    for (const changes of changeSets) {
      const version = this.lastVersion(scope) + 1
      const entries = changes.map(
        (change) =>
          new ChangelogEntry({
            scope,
            version: SyncVersion.make(version),
            entityType: EntityType.make(change.entityType),
            entityId: EntityId.make(change.entityId),
            op: change.op,
            ...(change.postImageJson !== undefined
              ? { postImageJson: change.postImageJson }
              : {}),
            committedAt: FIXED_TIME,
          }),
      )
      this.logOf(scope).push(...entries)
    }
  }

  emitFrame(scope: SyncScope, frame: LiveFrame): void {
    const socket = this.sockets.get(scope)
    if (socket !== undefined && socket.open) socket.handlers.onFrame(frame)
  }

  closeSocket(scope: SyncScope, error?: KhalaSyncTransportError): void {
    const socket = this.sockets.get(scope)
    if (socket === undefined || !socket.open) return
    socket.open = false
    socket.handlers.onClose(error === undefined ? {} : { error })
  }

  socketOpen(scope: SyncScope): boolean {
    return this.sockets.get(scope)?.open ?? false
  }

  // -- SPEC §3 handlers --------------------------------------------------------

  bootstrap(request: BootstrapRequest): BootstrapResponse {
    this.maybeFail("bootstrap")
    this.bootstrapCalls.push(request)
    this.denyIfRevoked(request.scope)
    const scope = request.scope
    let stash: { entities: ReadonlyArray<FakeChange>; cursor: number; offset: number }
    if (request.pageToken !== undefined) {
      const found = this.pageStash.get(request.pageToken)
      if (found === undefined) {
        throw new KhalaSyncTransportError("http_status", false, "unknown page token", {
          status: 400,
        })
      }
      stash = found
    } else {
      const cursor = Math.max(0, this.lastVersion(scope) - this.snapshotLag)
      stash = { entities: this.fold(scope, cursor), cursor, offset: 0 }
    }
    const page = stash.entities.slice(stash.offset, stash.offset + this.bootstrapPageSize)
    const nextOffset = stash.offset + page.length
    const entities = page.map(
      (entity) =>
        new BootstrapEntity({
          entityType: EntityType.make(entity.entityType),
          entityId: EntityId.make(entity.entityId),
          postImageJson: entity.postImageJson!,
        }),
    )
    if (nextOffset < stash.entities.length) {
      const token = `page_${this.tokenCounter++}`
      this.pageStash.set(token, { ...stash, offset: nextOffset })
      return new BootstrapResponse({
        protocolVersion: 1,
        scope,
        entities,
        nextPageToken: token,
      })
    }
    return new BootstrapResponse({
      protocolVersion: 1,
      scope,
      entities,
      cursor: SyncVersionWatermark.make(stash.cursor),
    })
  }

  logPage(scope: SyncScope, cursor: number, limit: number): LogPage {
    this.maybeFail("logPage")
    this.logPageCalls.push({ scope, cursor, limit })
    this.denyIfRevoked(scope)
    const after = this.logOf(scope).filter((entry) => entry.version > cursor)
    const versions = [...new Set(after.map((entry) => entry.version))].sort(
      (a, b) => a - b,
    )
    const included: Array<ChangelogEntry> = []
    let next = cursor
    for (const version of versions) {
      const group = after.filter((entry) => entry.version === version)
      if (included.length > 0 && included.length + group.length > limit) break
      included.push(...group)
      next = version
      if (included.length >= limit) break
    }
    return new LogPage({
      protocolVersion: 1,
      scope,
      entries: included,
      nextCursor: SyncVersionWatermark.make(next),
      upToDate: next >= this.lastVersion(scope),
    })
  }

  push(mutations: ReadonlyArray<MutationEnvelope>, clientKey: string): PushResponse {
    this.maybeFail("push")
    this.pushCalls.push([...mutations])
    let last = this.clientLast.get(clientKey) ?? 0
    const results: Array<MutationResult> = []
    for (const mutation of mutations) {
      if (mutation.mutationId <= last) {
        results.push(
          new MutationResultClass({ mutationId: mutation.mutationId, status: "duplicate" }),
        )
        continue
      }
      if (mutation.name === "task.reject") {
        results.push(
          new MutationResultClass({
            mutationId: mutation.mutationId,
            status: "rejected",
            errorCode: "mutation_rejected",
            errorMessageSafe: "test rejection",
          }),
        )
        last = mutation.mutationId
        continue
      }
      const args = JSON.parse(mutation.argsJson) as {
        scope: string
        id: string
        value: number
      }
      this.commit(
        SyncScope.make(args.scope),
        [
          {
            entityType: "task",
            entityId: args.id,
            op: "upsert",
            postImageJson: canonicalJson({ value: args.value }),
          },
        ],
        `mut.${clientKey}.${mutation.mutationId}`,
      )
      results.push(
        new MutationResultClass({ mutationId: mutation.mutationId, status: "applied" }),
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
      // LastMutationId is the plain ledger watermark (0 = nothing acked).
      lastMutationId: last,
    })
  }

  connect(scope: SyncScope, cursor: number, handlers: LiveSocketHandlers) {
    this.maybeFail("connect")
    this.connectCalls.push({ scope, cursor })
    if (this.connectAuthFault) {
      throw new KhalaSyncTransportError(
        "sync_error",
        false,
        "khala-sync server error unauthenticated: Khala Sync connect requires an authenticated session or agent token.",
        {
          status: 401,
          syncError: new SyncError({
            code: "unauthenticated",
            messageSafe:
              "Khala Sync connect requires an authenticated session or agent token.",
            retryable: false,
          }),
        },
      )
    }
    this.denyIfRevoked(scope)
    const record: SocketRecord = { handlers, open: true }
    this.sockets.set(scope, record)
    return {
      close: () => {
        record.open = false
      },
    }
  }
}

const transportOf = (server: FakeSyncServer): KhalaSyncTransport => {
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
    logPage: (scope, cursor, limit) => attempt(() => server.logPage(scope, cursor, limit)),
    push: (request) =>
      attempt(() =>
        server.push(request.mutations, `${request.clientGroupId}:${request.clientId}`),
      ),
    connectLive: (scope, cursor, handlers) =>
      attempt(() => server.connect(scope, cursor, handlers)),
  }
}

// ---------------------------------------------------------------------------
// Client mutators
// ---------------------------------------------------------------------------

interface SetArgs {
  readonly scope: string
  readonly id: string
  readonly value: number
}

const setTask: ClientMutator<SetArgs> = {
  name: MutatorName.make("task.set"),
  apply: (args) => [
    {
      kind: "upsert",
      scope: SyncScope.make(args.scope),
      entityType: "task",
      entityId: args.id,
      postImageJson: canonicalJson({ value: args.value }),
    },
  ],
}

/** The fake server rejects every mutation with this name. */
const rejectTask: ClientMutator<SetArgs> = {
  name: MutatorName.make("task.reject"),
  apply: (args) => [
    {
      kind: "upsert",
      scope: SyncScope.make(args.scope),
      entityType: "task",
      entityId: args.id,
      postImageJson: canonicalJson({ value: args.value }),
    },
  ],
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

const config = {
  baseUrl: "http://fake.test",
  clientGroupId: ClientGroupId.make("cg_test"),
  clientId: ClientId.make("c_test"),
  schemaVersion: SyncSchemaVersion.make(1),
  authToken: () => "test-token",
}

const makeHarness = (
  server: FakeSyncServer,
  opts: {
    readonly now?: () => number
    readonly connectFailureThreshold?: number
    readonly maxConnectAuthRejections?: number
  } = {},
) => {
  const store = openKhalaSyncStore(":memory:")
  cleanups.push(() => Effect.runSync(Effect.ignore(store.close())))
  const overlay = Effect.runSync(createOverlay(store, [setTask, rejectTask]))
  const stateLog: Array<{ scope: SyncScope; phase: string }> = []
  const rejections: Array<MutationResult> = []
  const transportErrors: Array<{ context: string; error: unknown }> = []
  const connectFailureSignals: Array<ConnectFailureSignal> = []
  const session = createKhalaSyncSession(config, store, overlay, transportOf(server), {
    // Injected time: every sleep is one immediate scheduler turn, jitter is constant.
    sleep: () => tick(),
    random: () => 0,
    ...(opts.now !== undefined ? { now: opts.now } : {}),
    ...(opts.connectFailureThreshold !== undefined
      ? { connectFailureThreshold: opts.connectFailureThreshold }
      : {}),
    ...(opts.maxConnectAuthRejections !== undefined
      ? { maxConnectAuthRejections: opts.maxConnectAuthRejections }
      : {}),
    backoffBaseMs: 1,
    backoffMaxMs: 4,
    maxBootstrapAttempts: 3,
    logPageLimit: 2,
    pushBatchSize: 10,
    onRejection: (result) => rejections.push(result),
    onTransportError: (context, error) => transportErrors.push({ context, error }),
    onConnectFailure: (signal) => connectFailureSignals.push(signal),
  })
  cleanups.push(() => Effect.runSync(session.close()))
  session.subscribeState((scope, state) => stateLog.push({ scope, phase: state.phase }))
  const storeCursor = (scope: SyncScope): number | null =>
    Effect.runSync(store.cursor(scope))
  const view = (scope: SyncScope) => Effect.runSync(overlay.read(scope))
  return {
    store,
    overlay,
    session,
    stateLog,
    rejections,
    transportErrors,
    connectFailureSignals,
    storeCursor,
    view,
  }
}

const image = (value: number): string => canonicalJson({ value })

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("computeBackoffMs", () => {
  test("jittered exponential growth with a ceiling; no wall clock involved", () => {
    // random = 0 → lower bound cap/2; random → 1 approaches cap.
    expect(computeBackoffMs(1, 100, 10_000, () => 0)).toBe(50)
    expect(computeBackoffMs(2, 100, 10_000, () => 0)).toBe(100)
    expect(computeBackoffMs(3, 100, 10_000, () => 0)).toBe(200)
    expect(computeBackoffMs(1, 100, 10_000, () => 0.999999)).toBeLessThan(100)
    // ceiling: attempts past the cap stop growing
    expect(computeBackoffMs(30, 100, 10_000, () => 0)).toBe(5_000)
    expect(computeBackoffMs(31, 100, 10_000, () => 0.5)).toBe(7_500)
  })
})

describe("khala-sync session (fake transport, injected time)", () => {
  test("refuses device-local scopes before hosted transport",async()=>{const server=new FakeSyncServer();const {session}=makeHarness(server);const exit=await Effect.runPromiseExit(session.subscribe(deviceLocalScope(LocalIdentityRef.make("local_fixture"))));expect(exit._tag).toBe("Failure");expect(server.logs.size).toBe(0)})
  test("fresh subscribe: bootstrap pages → catch-up → live; entries land via overlay", async () => {
    const server = new FakeSyncServer()
    server.bootstrapPageSize = 1 // force multi-page snapshot
    server.snapshotLag = 1 // snapshot at v2; v3 arrives via catch-up
    server.commit(scopeA, [{ entityType: "task", entityId: "a", op: "upsert", postImageJson: image(1) }])
    server.commit(scopeA, [{ entityType: "task", entityId: "b", op: "upsert", postImageJson: image(2) }])
    server.commit(scopeA, [{ entityType: "task", entityId: "c", op: "upsert", postImageJson: image(3) }])

    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")

    // State ordering: bootstrapping strictly before catching_up before live.
    const phases = h.stateLog.filter((s) => s.scope === scopeA).map((s) => s.phase)
    expect(phases[0]).toBe("bootstrapping")
    const firstCatchUp = phases.indexOf("catching_up")
    const firstLive = phases.indexOf("live")
    expect(firstCatchUp).toBeGreaterThan(0)
    expect(firstLive).toBeGreaterThan(firstCatchUp)

    // Snapshot paging used the pageToken chain (2 entities at v2 → 2 pages).
    expect(server.bootstrapCalls.length).toBeGreaterThanOrEqual(2)
    expect(server.bootstrapCalls[0]!.pageToken).toBeUndefined()
    expect(server.bootstrapCalls[1]!.pageToken).toBeDefined()

    // Snapshot (v2) + catch-up (v3) both landed; durable cursor = 3.
    const view = h.view(scopeA)
    expect(view.get("task", "a")).toBe(image(1))
    expect(view.get("task", "b")).toBe(image(2))
    expect(view.get("task", "c")).toBe(image(3))
    expect(h.storeCursor(scopeA)).toBe(3)

    // Live delta lands and advances the cursor.
    server.commit(scopeA, [{ entityType: "task", entityId: "d", op: "upsert", postImageJson: image(4) }])
    await waitFor(() => h.storeCursor(scopeA) === 4, "live delta applied")
    expect(h.view(scopeA).get("task", "d")).toBe(image(4))
    const state = h.session.state(scopeA)
    expect(state.phase).toBe("live")
    expect(state.phase === "live" && state.cursor).toBe(SyncVersionWatermark.make(4))
  })

  test("empty scope: bootstrap at watermark 0 → live; durable cursor stays unset", async () => {
    const server = new FakeSyncServer()
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live on empty scope")
    expect(h.storeCursor(scopeA)).toBeNull()
    const state = h.session.state(scopeA)
    expect(state.phase === "live" && state.cursor).toBe(SyncVersionWatermark.make(0))
    expect(h.view(scopeA).list("task")).toEqual([])
  })

  test("reconnect mid-catch-up resumes from the DURABLE cursor — no re-bootstrap", async () => {
    const server = new FakeSyncServer()
    for (let i = 1; i <= 6; i++) {
      server.commit(scopeA, [
        { entityType: "task", entityId: `t${i}`, op: "upsert", postImageJson: image(i) },
      ])
    }
    // Client already synced through v2 (durable cursor exists → no bootstrap).
    const h = makeHarness(server)
    await Effect.runPromise(
      h.store.resetScope(
        scopeA,
        [
          { entityType: "task", entityId: "t1", postImageJson: image(1), version: SyncVersion.make(1) },
          { entityType: "task", entityId: "t2", postImageJson: image(2), version: SyncVersion.make(2) },
        ],
        SyncVersion.make(2),
      ),
    )
    // Fail the SECOND log page (mid-catch-up network drop).
    server.failOn.logPage.add(2)

    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live after mid-catch-up drop")

    expect(server.bootstrapCalls.length).toBe(0) // never re-bootstraps
    // First page from the seeded durable cursor (2); the retry resumes from
    // the durable cursor as advanced by page 1 (4) — never from 0.
    expect(server.logPageCalls[0]!.cursor).toBe(2)
    expect(server.logPageCalls.every((call) => call.cursor > 0)).toBe(true)
    expect(server.logPageCalls.some((call) => call.cursor === 4)).toBe(true)
    expect(h.storeCursor(scopeA)).toBe(6)
    for (let i = 1; i <= 6; i++) {
      expect(h.view(scopeA).get("task", `t${i}`)).toBe(image(i))
    }
  })

  test("MustRefetch mid-live → must_refetch → re-bootstrap → live with replaced state", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "old1", op: "upsert", postImageJson: image(1) }])
    server.commit(scopeA, [{ entityType: "task", entityId: "old2", op: "upsert", postImageJson: image(2) }])
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "initial live")
    const bootstrapsBefore = server.bootstrapCalls.length

    // Server resets the scope (fresh log, versions restart) and orders a refetch.
    server.replaceScope(scopeA, [
      [{ entityType: "task", entityId: "new1", op: "upsert", postImageJson: image(10) }],
    ])
    server.emitFrame(scopeA, new MustRefetchFrame({ scope: scopeA, reason: "scope_reset" }))

    await waitFor(
      () => h.stateLog.some((s) => s.scope === scopeA && s.phase === "must_refetch"),
      "must_refetch observed",
    )
    await waitFor(
      () => h.session.state(scopeA).phase === "live" && h.storeCursor(scopeA) === 1,
      "live again after re-bootstrap",
    )
    expect(server.bootstrapCalls.length).toBeGreaterThan(bootstrapsBefore)
    const view = h.view(scopeA)
    expect(view.get("task", "new1")).toBe(image(10))
    expect(view.get("task", "old1")).toBeUndefined()
    expect(view.get("task", "old2")).toBeUndefined()
  })

  test("MustRefetch into an EMPTY scope clears local state and the durable cursor", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "old", op: "upsert", postImageJson: image(1) }])
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "initial live")
    expect(h.storeCursor(scopeA)).toBe(1)

    server.replaceScope(scopeA, []) // scope reset to nothing (watermark 0)
    server.emitFrame(scopeA, new MustRefetchFrame({ scope: scopeA, reason: "scope_reset" }))

    await waitFor(
      () => h.session.state(scopeA).phase === "live" && h.storeCursor(scopeA) === null,
      "live at scope start after empty refetch",
    )
    expect(h.view(scopeA).list("task")).toEqual([])
    const state = h.session.state(scopeA)
    expect(state.phase === "live" && state.cursor).toBe(SyncVersionWatermark.make(0))
  })

  // Behavior contract khala_sync.access.revocation_clears_synced_state.v1
  // (KS-9.2, #8311): the three revocation tests below are the client-side
  // oracle — denied re-bootstrap CLEARS durable rows + cursor and parks
  // TERMINAL. The full-stack oracle (real Postgres + real hub DO + real
  // Worker routes + this client) is
  // apps/openagents.com/workers/api/src/khala-sync-access-revocation.e2e.test.ts.
  test("revocation (KS-7.1 invariant 7): MustRefetch(access_changed) → denied re-bootstrap CLEARS scope-local state and parks TERMINAL denied — no retry", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "team-doc", op: "upsert", postImageJson: image(1) }])
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live while authorized")
    expect(h.storeCursor(scopeA)).toBe(1)
    expect(h.view(scopeA).get("task", "team-doc")).toBe(image(1))

    // Membership revoked: reads now 403, and the hub broadcasts
    // MustRefetch(access_changed) + closes the socket.
    server.deniedScopes.add(scopeA)
    server.emitFrame(scopeA, new MustRefetchFrame({ scope: scopeA, reason: "access_changed" }))
    server.closeSocket(scopeA)

    await waitFor(() => h.session.state(scopeA).phase === "denied", "parked denied")
    const state = h.session.state(scopeA)
    expect(state.phase === "denied" && state.reason).toBe("access_denied")

    // Revocation retracted the synced state: durable rows + cursor CLEARED,
    // and the read view is empty.
    expect(h.storeCursor(scopeA)).toBeNull()
    expect(Effect.runSync(h.store.readEntities(scopeA))).toEqual([])
    expect(h.view(scopeA).list("task")).toEqual([])

    // TERMINAL: exactly one denied re-bootstrap, then silence — no retry
    // loop against a 403 that can never succeed.
    const bootstrapsAtPark = server.attempts.bootstrap
    const connectsAtPark = server.attempts.connect
    for (let i = 0; i < 25; i++) await tick()
    expect(server.attempts.bootstrap).toBe(bootstrapsAtPark)
    expect(server.attempts.connect).toBe(connectsAtPark)
    // must_refetch was observed BEFORE the deny → clear → park sequence.
    const phases = h.stateLog.filter((s) => s.scope === scopeA).map((s) => s.phase)
    expect(phases.indexOf("must_refetch")).toBeGreaterThanOrEqual(0)
    expect(phases.indexOf("denied")).toBeGreaterThan(phases.indexOf("must_refetch"))
  })

  test("revocation mid catch-up: a 403 log page clears scope-local state and parks denied", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "t1", op: "upsert", postImageJson: image(1) }])
    const h = makeHarness(server)
    // Client already synced through v1 (durable cursor → catch-up, no bootstrap).
    await Effect.runPromise(
      h.store.resetScope(
        scopeA,
        [{ entityType: "task", entityId: "t1", postImageJson: image(1), version: SyncVersion.make(1) }],
        SyncVersion.make(1),
      ),
    )
    server.deniedScopes.add(scopeA)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "denied", "parked denied from catch-up")
    expect(server.bootstrapCalls.length).toBe(0)
    expect(h.storeCursor(scopeA)).toBeNull()
    expect(Effect.runSync(h.store.readEntities(scopeA))).toEqual([])
  })

  test("denial on FIRST contact (never authorized): parks denied after a single bootstrap attempt", async () => {
    const server = new FakeSyncServer()
    server.deniedScopes.add(scopeA)
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "denied", "parked denied on first contact")
    expect(server.attempts.bootstrap).toBe(1)
    for (let i = 0; i < 25; i++) await tick()
    expect(server.attempts.bootstrap).toBe(1)
    expect(h.storeCursor(scopeA)).toBeNull()
  })

  // ST-7 (#8513): 401-on-connect parking + connect-failure telemetry. The
  // regression class: mobile builds 10–13 showed "Loading threads" forever
  // because a WS-auth 401 on connect was retried infinitely with no signal
  // (docs/khala-code/2026-07-06-mobile-loading-threads-websocket-auth-audit.md).
  test("ST-7: 401 on connect parks TERMINAL denied(auth_rejected) after the bounded rotation budget — never an infinite retry loop", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "a", op: "upsert", postImageJson: image(1) }])
    server.connectAuthFault = true // bootstrap + log succeed; the WS 401s
    const h = makeHarness(server)

    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "denied", "parked denied on 401 connect")
    const state = h.session.state(scopeA)
    expect(state.phase === "denied" && state.reason).toBe("auth_rejected")

    // Bounded budget: default 2 total attempts (1 rejection + 1 rotation
    // retry), then park. NOT forever.
    expect(server.attempts.connect).toBe(2)
    const connectsAtPark = server.attempts.connect
    for (let i = 0; i < 25; i++) await tick()
    expect(server.attempts.connect).toBe(connectsAtPark) // TERMINAL: no further retry

    // Parking is the same clear-and-stop as a 403 denial: durable rows +
    // cursor cleared, freshness stamp retracted.
    expect(h.storeCursor(scopeA)).toBeNull()
    expect(Effect.runSync(h.store.readEntities(scopeA))).toEqual([])
    expect(h.session.lastDeltaAt(scopeA)).toBeNull()

    // Every rejected connect was tapped through onTransportError("live").
    expect(h.transportErrors.filter((e) => e.context === "live").length).toBeGreaterThanOrEqual(2)
  })

  test("ST-7: a transient 401 during token rotation heals within the budget — no over-park", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "a", op: "upsert", postImageJson: image(1) }])
    server.connectAuthFault = true
    const h = makeHarness(server)

    await Effect.runPromise(h.session.subscribe(scopeA))
    // The FIRST connect 401s (stale token mid-rotation); the fault clears
    // before the bounded re-attempt — exactly the case that must NOT park.
    await waitFor(() => server.attempts.connect >= 1, "first connect rejected")
    server.connectAuthFault = false

    await waitFor(() => h.session.state(scopeA).phase === "live", "live after rotation heal")
    expect(h.storeCursor(scopeA)).toBe(1) // durable state survived — never cleared
    expect(h.view(scopeA).get("task", "a")).toBe(image(1))
  })

  test("ST-7: transient network connect failures still retry (unchanged) and reach live", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "a", op: "upsert", postImageJson: image(1) }])
    server.failNext.connect = 3 // network-class, retryable
    const h = makeHarness(server)

    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live after transient connect faults")
    expect(server.attempts.connect).toBeGreaterThanOrEqual(4) // 3 failures + success
    const phases = h.stateLog.filter((s) => s.scope === scopeA).map((s) => s.phase)
    expect(phases).not.toContain("denied") // transient faults never park
  })

  test("ST-7: the connect-failure signal fires at N consecutive failures (bounded, structured) and the streak resets on success", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "a", op: "upsert", postImageJson: image(1) }])
    server.failNext.connect = 3
    const h = makeHarness(server, { connectFailureThreshold: 3 })

    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live after signal")

    // Exactly one signal: at failure 3 (not 1, not 2), none after recovery.
    expect(h.connectFailureSignals).toEqual([
      { scope: scopeA, consecutiveFailures: 3, reason: "network" },
    ])

    // The success reset the streak: two MORE failures stay under the
    // threshold, so no second signal fires.
    server.failNext.connect = 2
    server.closeSocket(scopeA, networkError())
    await waitFor(() => h.session.state(scopeA).phase === "live", "live again after reset streak")
    expect(h.connectFailureSignals.length).toBe(1)
  })

  test("ST-7: a 401 streak carries the HTTP status in the signal before parking", async () => {
    const server = new FakeSyncServer()
    server.connectAuthFault = true
    // Threshold 2 == budget 2: the signal fires on the same failure that
    // exhausts the budget, so the page and the park agree.
    const h = makeHarness(server, { connectFailureThreshold: 2 })

    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "denied", "parked denied")
    expect(h.connectFailureSignals).toEqual([
      { scope: scopeA, consecutiveFailures: 2, reason: "sync_error", status: 401 },
    ])
  })

  test("push: transient failures retry with the queue intact; rejection is surfaced but ACKs in-band", async () => {
    const server = new FakeSyncServer()
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")

    server.failNext.push = 2 // two transient faults before the server answers
    await Effect.runPromise(
      h.session.mutate(setTask, { scope: scopeA, id: "ok1", value: 7 }),
    )
    await Effect.runPromise(
      h.session.mutate(rejectTask, { scope: scopeA, id: "rej1", value: 9 }),
    )
    expect(h.overlay.pending().length).toBeGreaterThan(0) // queue intact mid-retry

    await waitFor(() => h.overlay.pending().length === 0, "queue drained")

    // Retried: at least 3 attempts (2 failures + 1 success), ids in order.
    expect(server.attempts.push).toBeGreaterThanOrEqual(3)
    const delivered = server.pushCalls[server.pushCalls.length - 1]!
    expect(delivered.map((m) => m.mutationId)).toEqual([1, 2].map((n) => MutationId.make(n)))

    // Rejection surfaced through the hook AND acked (queue advanced past it).
    expect(h.rejections.length).toBe(1)
    expect(h.rejections[0]!.mutationId).toBe(MutationId.make(2))
    expect(h.rejections[0]!.status).toBe("rejected")
    expect(h.rejections[0]!.errorCode).toBe("mutation_rejected")
    expect(Effect.runSync(h.store.lastMutationId())).toBe(MutationId.make(2))
    expect(Effect.runSync(h.store.pendingMutations())).toEqual([])

    // Applied mutation converges to confirmed; rejected one leaves no residue.
    await waitFor(
      () => h.view(scopeA).get("task", "ok1") === image(7),
      "applied mutation confirmed",
    )
    await waitFor(
      () => h.view(scopeA).get("task", "rej1") === undefined,
      "rejected optimistic effect dropped",
    )
  })

  test("lost push acknowledgement reconciles as one duplicate replay, never a second effect", async () => {
    const server = new FakeSyncServer()
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")

    // The first call commits task/lost exactly once but its response is lost.
    // The client must keep mutation 1 pending and retry that exact identity.
    server.dropNextPushAck = 1
    await Effect.runPromise(
      h.session.mutate(setTask, { scope: scopeA, id: "lost", value: 42 }),
    )
    await waitFor(() => server.pushCalls.length >= 2, "duplicate retry after lost ack")
    await waitFor(() => h.overlay.pending().length === 0, "reconciled queue")

    expect(server.pushCalls.slice(0, 2).map((call) => call.map((mutation) => mutation.mutationId))).toEqual([
      [MutationId.make(1)],
      [MutationId.make(1)],
    ])
    expect(server.logOf(scopeA).filter((entry) => entry.entityId === "lost")).toHaveLength(1)
    expect(Effect.runSync(h.store.lastMutationId())).toBe(MutationId.make(1))
    expect(Effect.runSync(h.store.pendingMutations())).toEqual([])
    await waitFor(() => h.view(scopeA).get("task", "lost") === image(42), "one confirmed effect")
    expect(h.view(scopeA).list("task").filter((task) => task.entityId === "lost")).toHaveLength(1)
  })

  test("offline queueing (v1 online-optimistic): mutations wait, drain on recovery, converge", async () => {
    const server = new FakeSyncServer()
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")

    server.offline = true
    await Effect.runPromise(h.session.mutate(setTask, { scope: scopeA, id: "q1", value: 1 }))
    await Effect.runPromise(h.session.mutate(setTask, { scope: scopeA, id: "q2", value: 2 }))
    await Effect.runPromise(h.session.mutate(setTask, { scope: scopeA, id: "q3", value: 3 }))

    // Queue holds all three; reads stay available with optimistic effects.
    expect(h.overlay.pending().map((m) => m.mutationId)).toEqual([1, 2, 3].map((n) => MutationId.make(n)))
    expect(h.view(scopeA).get("task", "q2")).toBe(image(2))
    // Nothing optimistic leaked into the durable store.
    expect(Effect.runSync(h.store.readEntities(scopeA))).toEqual([])
    // Let the push loop retry a few times against the dead transport.
    await tick()
    await tick()
    expect(h.overlay.pending().length).toBe(3) // queue intact through failures

    server.offline = false
    await waitFor(() => h.overlay.pending().length === 0, "queue drained on recovery")
    await waitFor(() => h.storeCursor(scopeA) === 3, "confirmed deltas landed")

    // Per-client ordering preserved; convergence to server state.
    const delivered = server.pushCalls.flat().map((m) => m.mutationId)
    expect(delivered).toEqual([...delivered].sort((a, b) => a - b))
    for (const [id, value] of [
      ["q1", 1],
      ["q2", 2],
      ["q3", 3],
    ] as const) {
      expect(h.view(scopeA).get("task", id)).toBe(image(value))
      const confirmed = Effect.runSync(h.store.readEntities(scopeA)).find(
        (entity) => entity.entityId === id,
      )
      expect(confirmed?.postImageJson).toBe(image(value))
    }
  })

  test("terminal push fault PARKS the queue (no hot loop); next mutate re-kicks and drains", async () => {
    const server = new FakeSyncServer()
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")

    server.pushTerminalFault = true // e.g. auth: non-retryable
    await Effect.runPromise(h.session.mutate(setTask, { scope: scopeA, id: "p1", value: 1 }))
    await waitFor(() => server.attempts.push >= 1, "terminal push attempted")
    const attemptsAtPark = server.attempts.push

    // Parked: the queue is intact and NO further attempts happen — a
    // re-kick after a terminal fault would spin against the same failure.
    for (let i = 0; i < 25; i++) await tick()
    expect(server.attempts.push).toBe(attemptsAtPark)
    expect(h.overlay.pending().map((m) => m.mutationId)).toEqual(
      [1].map((n) => MutationId.make(n)),
    )
    expect(h.view(scopeA).get("task", "p1")).toBe(image(1)) // optimistic effect kept

    // Fault cleared + next mutate re-kicks: BOTH mutations drain in order.
    server.pushTerminalFault = false
    await Effect.runPromise(h.session.mutate(setTask, { scope: scopeA, id: "p2", value: 2 }))
    await waitFor(() => h.overlay.pending().length === 0, "queue drained after re-kick")
    expect(server.pushCalls.flat().map((m) => m.mutationId)).toEqual(
      [1, 2].map((n) => MutationId.make(n)),
    )
    await waitFor(() => h.storeCursor(scopeA) === 2, "confirmed deltas landed")
    expect(h.view(scopeA).get("task", "p1")).toBe(image(1))
    expect(h.view(scopeA).get("task", "p2")).toBe(image(2))
  })

  test("out-of-order and duplicate delta delivery is safe (idempotent apply)", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "a", op: "upsert", postImageJson: image(1) }])
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")
    expect(h.storeCursor(scopeA)).toBe(1)

    const v2Entries = [
      new ChangelogEntry({
        scope: scopeA,
        version: SyncVersion.make(2),
        entityType: EntityType.make("task"),
        entityId: EntityId.make("b"),
        op: "upsert",
        postImageJson: image(2),
        committedAt: FIXED_TIME,
      }),
    ]
    const v1Entries = [
      new ChangelogEntry({
        scope: scopeA,
        version: SyncVersion.make(1),
        entityType: EntityType.make("task"),
        entityId: EntityId.make("a"),
        op: "upsert",
        postImageJson: image(1),
        committedAt: FIXED_TIME,
      }),
    ]
    // Legit v2, then a duplicate v2, then a stale re-delivered v1.
    server.emitFrame(scopeA, new DeltaFrame({ scope: scopeA, entries: v2Entries, cursor: SyncVersion.make(2) }))
    server.emitFrame(scopeA, new DeltaFrame({ scope: scopeA, entries: v2Entries, cursor: SyncVersion.make(2) }))
    server.emitFrame(scopeA, new DeltaFrame({ scope: scopeA, entries: v1Entries, cursor: SyncVersion.make(1) }))

    await waitFor(() => h.storeCursor(scopeA) === 2, "cursor advanced to 2")
    // Extra ticks: the duplicate/stale frames must be no-ops, not errors.
    await tick()
    await tick()
    expect(h.storeCursor(scopeA)).toBe(2)
    expect(h.view(scopeA).get("task", "a")).toBe(image(1))
    expect(h.view(scopeA).get("task", "b")).toBe(image(2))
    expect(h.session.state(scopeA).phase).toBe("live")
    expect(h.transportErrors.filter((e) => e.context === "live")).toEqual([])
  })

  test("a missing live version is refused and replayed from the durable cursor", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{
      entityType: "task",
      entityId: "a",
      op: "upsert",
      postImageJson: image(1),
    }])
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")
    expect(h.storeCursor(scopeA)).toBe(1)

    server.commit(scopeA, [{
      entityType: "task",
      entityId: "b",
      op: "upsert",
      postImageJson: image(2),
    }], undefined, false)
    server.commit(scopeA, [{
      entityType: "task",
      entityId: "c",
      op: "upsert",
      postImageJson: image(3),
    }], undefined, false)
    const onlyV3 = server.logOf(scopeA).filter(entry => Number(entry.version) === 3)
    server.emitFrame(scopeA, new DeltaFrame({
      scope: scopeA,
      entries: onlyV3,
      cursor: SyncVersion.make(3),
    }))

    await waitFor(
      () => h.transportErrors.some(error =>
        error.context === "live" &&
        error.error instanceof KhalaSyncTransportError &&
        error.error.reason === "decode_failure"),
      "gap refusal",
    )
    await waitFor(
      () => h.storeCursor(scopeA) === 3 && h.session.state(scopeA).phase === "live",
      "dense replay through v3",
    )
    expect(h.view(scopeA).get("task", "a")).toBe(image(1))
    expect(h.view(scopeA).get("task", "b")).toBe(image(2))
    expect(h.view(scopeA).get("task", "c")).toBe(image(3))
    expect(server.logPageCalls.some(call => call.cursor === 1)).toBe(true)
    expect(h.session.state(scopeA).phase).toBe("live")
  })

  test("socket drop mid-live reconnects from the durable cursor and catches up", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "a", op: "upsert", postImageJson: image(1) }])
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")
    const connectsBefore = server.connectCalls.length
    const bootstrapsBefore = server.bootstrapCalls.length

    // Kill the socket; commit while disconnected (socket is closed, so the
    // delta frame is NOT delivered live — only catch-up can recover it).
    server.closeSocket(scopeA, networkError())
    server.commit(scopeA, [{ entityType: "task", entityId: "b", op: "upsert", postImageJson: image(2) }])

    await waitFor(
      () => h.session.state(scopeA).phase === "live" && h.storeCursor(scopeA) === 2,
      "reconnected and caught up",
    )
    expect(server.connectCalls.length).toBeGreaterThan(connectsBefore)
    expect(server.bootstrapCalls.length).toBe(bootstrapsBefore) // durable cursor, no re-bootstrap
    // The reconnect resumed from the durable cursor (1), not from 0.
    expect(server.connectCalls[server.connectCalls.length - 1]!.cursor).toBe(2)
    expect(h.view(scopeA).get("task", "b")).toBe(image(2))
  })

  test("unsubscribe stops the loop: no reconnect, no further applies", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "a", op: "upsert", postImageJson: image(1) }])
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")

    await Effect.runPromise(h.session.unsubscribe(scopeA))
    expect(h.session.state(scopeA).phase).toBe("idle")
    const connectsAfterUnsubscribe = server.connectCalls.length

    server.commit(scopeA, [{ entityType: "task", entityId: "b", op: "upsert", postImageJson: image(2) }])
    for (let i = 0; i < 10; i++) await tick()
    expect(h.storeCursor(scopeA)).toBe(1) // nothing applied after unsubscribe
    expect(server.connectCalls.length).toBe(connectsAfterUnsubscribe) // no reconnect
  })

  test("a closed session refuses mutation before anything reaches the durable queue", async () => {
    const h = makeHarness(new FakeSyncServer())
    await Effect.runPromise(h.session.close())
    const exit = await Effect.runPromiseExit(h.session.mutate(setTask, {
      id: "after-close",
      scope: scopeA,
      value: 1,
    }))
    expect(exit._tag).toBe("Failure")
    expect(Effect.runSync(h.store.pendingMutations())).toEqual([])
  })

  test("proven revocation burns queued commands and retracts confirmed hosted state", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{
      entityId: "confirmed-before-revoke",
      entityType: "task",
      op: "upsert",
      postImageJson: image(1),
    }])
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live before revoke")
    server.offline = true
    await Effect.runPromise(h.session.mutate(setTask, {
      id: "queued-before-revoke",
      scope: scopeA,
      value: 2,
    }))
    expect(h.session.pending()).toHaveLength(1)

    await Effect.runPromise(h.session.revoke())

    expect(h.session.pending()).toEqual([])
    expect(h.storeCursor(scopeA)).toBeNull()
    expect(Effect.runSync(h.store.readEntities(scopeA))).toEqual([])
    expect((await Effect.runPromiseExit(h.session.mutate(setTask, {
      id: "after-revoke",
      scope: scopeA,
      value: 3,
    })))._tag).toBe("Failure")
  })

  test("mobile background/foreground resubscribe catches up from the durable cursor without duplicate state", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "a", op: "upsert", postImageJson: image(1) }])
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "initial foreground live")
    const bootstrapsBeforeBackground = server.bootstrapCalls.length

    // Backgrounding releases the live socket. A server update while the app
    // is suspended must remain absent from the local projection until the
    // foreground subscription catches it up from the durable cursor.
    await Effect.runPromise(h.session.unsubscribe(scopeA))
    server.commit(scopeA, [{ entityType: "task", entityId: "b", op: "upsert", postImageJson: image(2) }])
    expect(h.storeCursor(scopeA)).toBe(1)
    expect(h.view(scopeA).get("task", "b")).toBeUndefined()

    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(
      () => h.session.state(scopeA).phase === "live" && h.storeCursor(scopeA) === 2,
      "foreground catch-up",
    )
    expect(server.bootstrapCalls.length).toBe(bootstrapsBeforeBackground)
    expect(h.view(scopeA).list("task").map((task) => task.entityId).sort()).toEqual(["a", "b"])
    expect(h.view(scopeA).get("task", "a")).toBe(image(1))
    expect(h.view(scopeA).get("task", "b")).toBe(image(2))
  })
})

// ---------------------------------------------------------------------------
// KS-9.2 (#8311) synced-surface behavior contracts — oracles over the same
// fake transport. These are the registered owner-stated expectations in
// packages/behavior-contracts/src/khala-sync.ts; each test name carries its
// contractId so the coverage checker can prove the linkage.
// ---------------------------------------------------------------------------

describe("khala-sync synced-surface behavior contracts (KS-9.2, #8311)", () => {
  test("contract khala_sync.client.offline_pushes_queue_honestly.v1: offline mutations are durably queued, visibly pending (never shown confirmed), drain in order on recovery, and a terminal rejection acks honestly", async () => {
    const server = new FakeSyncServer()
    const h = makeHarness(server)
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")

    // Transport starts failing; the user keeps mutating.
    server.offline = true
    await Effect.runPromise(h.session.mutate(setTask, { scope: scopeA, id: "q1", value: 1 }))
    await Effect.runPromise(h.session.mutate(rejectTask, { scope: scopeA, id: "bad", value: 2 }))
    await Effect.runPromise(h.session.mutate(setTask, { scope: scopeA, id: "q3", value: 3 }))

    // DURABLY queued: the intents live in the store's durable FIFO queue
    // (they survive a restart), in submission order.
    expect(Effect.runSync(h.store.pendingMutations()).map((m) => m.mutationId)).toEqual(
      [1, 2, 3].map((n) => MutationId.make(n)),
    )
    // VISIBLY pending: the session's UI-facing exposure lists exactly the
    // unconfirmed mutations, so a surface can mark them pending...
    expect(h.session.pending().map((m) => m.mutationId)).toEqual(
      [1, 2, 3].map((n) => MutationId.make(n)),
    )
    // ...and NOT confirmed: the durable entity store (server truth) holds
    // none of the optimistic effects.
    expect(Effect.runSync(h.store.readEntities(scopeA))).toEqual([])
    // The optimistic view still shows the work (online-optimistic reads).
    expect(h.view(scopeA).get("task", "q1")).toBe(image(1))

    // Let the push loop retry against the dead transport: the queue and
    // the pending exposure stay intact — nothing expires silently.
    await tick()
    await tick()
    expect(h.session.pending().length).toBe(3)

    // Recovery: the queue drains IN ORDER.
    server.offline = false
    await waitFor(() => h.session.pending().length === 0, "queue drained on recovery")
    const delivered = server.pushCalls.flat().map((m) => m.mutationId)
    expect(delivered).toEqual([...delivered].sort((a, b) => a - b))

    // Terminal rejection is HONEST: the rejected mutation is acked in-band
    // (it leaves the queue — no immortal retry), the rejection is surfaced,
    // and it leaves no confirmed residue masquerading as success.
    expect(h.rejections.map((r) => [r.mutationId, r.status])).toEqual([
      [MutationId.make(2), "rejected"],
    ])
    expect(Effect.runSync(h.store.pendingMutations())).toEqual([])
    await waitFor(
      () => h.view(scopeA).get("task", "bad") === undefined,
      "rejected optimistic effect retracted",
    )
    // The genuinely applied mutations converged to confirmed truth.
    await waitFor(() => h.view(scopeA).get("task", "q1") === image(1), "q1 confirmed")
    expect(h.view(scopeA).get("task", "q3")).toBe(image(3))
    const confirmedIds = Effect.runSync(h.store.readEntities(scopeA)).map((e) => e.entityId)
    expect(confirmedIds).toContain("q1")
    expect(confirmedIds).toContain("q3")
    expect(confirmedIds).not.toContain("bad")

    // Terminal transport fault (e.g. auth): the queue PARKS intact — it
    // neither drains as fake success nor vanishes.
    const attemptsBeforeFault = server.attempts.push
    server.pushTerminalFault = true
    await Effect.runPromise(h.session.mutate(setTask, { scope: scopeA, id: "p1", value: 9 }))
    await waitFor(() => server.attempts.push > attemptsBeforeFault, "terminal push attempted")
    for (let i = 0; i < 20; i++) await tick()
    expect(h.session.pending().map((m) => m.mutationId)).toEqual([MutationId.make(4)])
    expect(
      Effect.runSync(h.store.readEntities(scopeA)).some((e) => e.entityId === "p1"),
    ).toBe(false)
  })

  test("contract khala_sync.client.staleness_never_fabricated.v1: the session exposes real freshness primitives — phase + lastDeltaAt — and never fabricates them", async () => {
    const server = new FakeSyncServer()
    server.commit(scopeA, [{ entityType: "task", entityId: "a", op: "upsert", postImageJson: image(1) }])
    let clock = 1_000
    const h = makeHarness(server, { now: () => clock })

    // Before anything confirmed lands there is NO freshness to claim:
    // phase is idle and lastDeltaAt is null — not a fake "live" default.
    expect(h.session.state(scopeA).phase).toBe("idle")
    expect(h.session.lastDeltaAt(scopeA)).toBeNull()

    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")
    // Bootstrap (a full server-confirmed snapshot) stamped the clock.
    expect(h.session.lastDeltaAt(scopeA)).toBe(1_000)

    // A live delta advances the stamp to the injected clock's new time.
    clock = 2_000
    server.commit(scopeA, [{ entityType: "task", entityId: "b", op: "upsert", postImageJson: image(2) }])
    await waitFor(() => h.session.lastDeltaAt(scopeA) === 2_000, "delta stamped")

    // Socket death: the phase drops out of `live` (no fabricated liveness)
    // while lastDeltaAt honestly keeps the LAST confirmed time — it does
    // not advance without server truth.
    clock = 3_000
    server.offline = true
    server.closeSocket(scopeA, networkError())
    await waitFor(() => h.session.state(scopeA).phase !== "live", "left live on socket death")
    expect(h.session.lastDeltaAt(scopeA)).toBe(2_000)

    // Recovery returns to a REAL live phase and real stamps resume.
    server.offline = false
    clock = 4_000
    await waitFor(() => h.session.state(scopeA).phase === "live", "live again")
    server.commit(scopeA, [{ entityType: "task", entityId: "c", op: "upsert", postImageJson: image(3) }])
    await waitFor(() => h.session.lastDeltaAt(scopeA) === 4_000, "post-recovery delta stamped")

    // Revocation clears the synced data — and with it the freshness stamp:
    // cleared data must not keep claiming it was fresh.
    server.deniedScopes.add(scopeA)
    server.emitFrame(scopeA, new MustRefetchFrame({ scope: scopeA, reason: "access_changed" }))
    server.closeSocket(scopeA)
    await waitFor(() => h.session.state(scopeA).phase === "denied", "parked denied")
    expect(h.session.lastDeltaAt(scopeA)).toBeNull()

    // The desktop Fleet indicator consumes the phase primitive under the
    // desktop contract khala_code.fleet.khala_sync_indicator_truthful.v1
    // (clients/khala-code-desktop/tests/ux-contracts.test.ts) — referenced
    // here, not duplicated.
  })
})
