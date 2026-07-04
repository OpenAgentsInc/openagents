import {
  BootstrapEntity,
  type BootstrapRequest,
  BootstrapResponse,
  canonicalJson,
  ChangelogEntry,
  ClientGroupId,
  ClientId,
  CvrDel,
  type CvrPullRequest,
  CvrPullResponse,
  CvrVersion,
  EntityId,
  EntityType,
  type LiveFrame,
  LogPage,
  MustRefetchFrame,
  PushResponse,
  SyncError,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { createOverlay } from "./overlay.js"
import { createKhalaSyncSession } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"
import {
  type KhalaSyncTransport,
  KhalaSyncTransportError,
  type LiveSocketHandlers,
} from "./transport.js"

/**
 * KS-7.2 (#8306) session-level equivalence tests: against ONE fake server
 * history, a FLAGGED session (cvrRecovery: true) recovering from
 * `must_refetch` via the CVR diff pull must end byte-equal to an UNFLAGGED
 * session recovering via the full re-bootstrap — including the retraction
 * case (a row acquired live, then deleted while the client was offline and
 * its tombstone compacted away, arrives as a del). Also: flag off ⇒ the
 * transport's cvrPull is NEVER invoked (zero behavior change), and a
 * failing cvrPull falls back to the bootstrap path.
 */

// ---------------------------------------------------------------------------
// Deterministic helpers (same idioms as session.test.ts)
// ---------------------------------------------------------------------------

const tick = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0))

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
const scopeA = SyncScope.make("scope.team.cvr-alpha")

const cleanups: Array<() => void> = []
afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()!()
})

const refetchError = (): KhalaSyncTransportError =>
  new KhalaSyncTransportError(
    "sync_error",
    false,
    "khala-sync server error cursor_behind_retained_window",
    {
      status: 410,
      syncError: new SyncError({
        code: "cursor_behind_retained_window",
        messageSafe: "cursor behind the retained window",
        retryable: false,
      }),
    },
  )

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

const networkError = (): KhalaSyncTransportError =>
  new KhalaSyncTransportError("network", true, "fake network failure")

// ---------------------------------------------------------------------------
// Fake server: SPEC §3 log/bootstrap/connect + the KS-7.2 cvr-pull, with a
// retained-window watermark so catch-up can force must_refetch honestly.
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

class CvrFakeServer {
  readonly logs = new Map<SyncScope, Array<ChangelogEntry>>()
  readonly sockets = new Map<SyncScope, SocketRecord>()
  readonly retainedFrom = new Map<SyncScope, number>()
  /** CVRs per `${clientGroupId}:${scope}` → cvrVersion → row set. */
  private readonly cvrs = new Map<
    string,
    Map<number, { entries: Map<string, number>; cursor: number }>
  >()
  offline = false
  failNextCvrPull: KhalaSyncTransportError | null = null
  denyCvrPull = false

  readonly cvrCalls: Array<CvrPullRequest> = []
  readonly cvrResponses: Array<CvrPullResponse> = []
  readonly bootstrapCalls: Array<BootstrapRequest> = []

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

  /** Latest state per entity through `throughVersion`, with versions. */
  fold(
    scope: SyncScope,
    throughVersion: number,
  ): Map<string, { readonly postImageJson: string; readonly version: number }> {
    const state = new Map<string, { postImageJson: string; version: number }>()
    for (const entry of this.logOf(scope)) {
      if (entry.version > throughVersion) continue
      const key = `${entry.entityType}/${entry.entityId}`
      if (entry.op === "delete") state.delete(key)
      else {
        state.set(key, {
          postImageJson: entry.postImageJson!,
          version: entry.version,
        })
      }
    }
    return state
  }

  commit(scope: SyncScope, changes: ReadonlyArray<FakeChange>): number {
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
    const socket = this.sockets.get(scope)
    if (socket !== undefined && socket.open) {
      socket.handlers.onFrame({
        _tag: "DeltaFrame",
        scope,
        entries,
        cursor: SyncVersion.make(version),
      } as LiveFrame)
    }
    return version
  }

  /** Compact everything: only latest-per-entity survives; the log is gone. */
  compactAllHistory(scope: SyncScope): void {
    this.retainedFrom.set(scope, this.lastVersion(scope) + 1)
  }

  emitMustRefetch(scope: SyncScope): void {
    const socket = this.sockets.get(scope)
    if (socket !== undefined && socket.open) {
      socket.handlers.onFrame(
        new MustRefetchFrame({ scope, reason: "cursor_behind_retained_window" }),
      )
    }
  }

  closeSocket(scope: SyncScope): void {
    const socket = this.sockets.get(scope)
    if (socket === undefined || !socket.open) return
    socket.open = false
    socket.handlers.onClose({ error: networkError() })
  }

  // -- handlers ----------------------------------------------------------------

  bootstrap(request: BootstrapRequest): BootstrapResponse {
    if (this.offline) throw networkError()
    this.bootstrapCalls.push(request)
    const scope = request.scope
    const cursor = this.lastVersion(scope)
    const entities = [...this.fold(scope, cursor).entries()]
      .sort(([a], [b]) => (a < b ? -1 : 1))
      .map(([key, row]) => {
        const slash = key.indexOf("/")
        return new BootstrapEntity({
          entityType: EntityType.make(key.slice(0, slash)),
          entityId: EntityId.make(key.slice(slash + 1)),
          postImageJson: row.postImageJson,
        })
      })
    return new BootstrapResponse({
      protocolVersion: 1,
      scope,
      entities,
      cursor: SyncVersionWatermark.make(cursor),
    })
  }

  logPage(scope: SyncScope, cursor: number, limit: number): LogPage {
    if (this.offline) throw networkError()
    const retainedFrom = this.retainedFrom.get(scope) ?? 1
    if (cursor < retainedFrom - 1) throw refetchError()
    const after = this.logOf(scope)
      .filter((entry) => entry.version > cursor)
      .slice(0, limit)
    const last = after[after.length - 1]
    const next = last === undefined ? cursor : last.version
    return new LogPage({
      protocolVersion: 1,
      scope,
      entries: after,
      nextCursor: SyncVersionWatermark.make(next),
      upToDate: next >= this.lastVersion(scope),
    })
  }

  connect(scope: SyncScope, _cursor: number, handlers: LiveSocketHandlers) {
    if (this.offline) throw networkError()
    const record: SocketRecord = { handlers, open: true }
    this.sockets.set(scope, record)
    return {
      close: () => {
        record.open = false
      },
    }
  }

  /** The KS-7.2 pull semantics (mirror of the Postgres cvr-service). */
  cvrPull(request: CvrPullRequest): CvrPullResponse {
    if (this.offline) throw networkError()
    this.cvrCalls.push(request)
    if (this.denyCvrPull) throw accessDeniedError()
    if (this.failNextCvrPull !== null) {
      const error = this.failNextCvrPull
      this.failNextCvrPull = null
      throw error
    }
    const scope = request.scope
    const cursor = this.lastVersion(scope)
    const current = this.fold(scope, cursor)

    const storeKey = `${request.clientGroupId}:${scope}`
    let perScope = this.cvrs.get(storeKey)
    if (perScope === undefined) {
      perScope = new Map()
      this.cvrs.set(storeKey, perScope)
    }
    const referenced =
      request.cvrVersion === undefined
        ? undefined
        : perScope.get(Number(request.cvrVersion))
    let base: Map<string, number> | null = null
    if (referenced !== undefined) {
      base = new Map(referenced.entries)
      for (const drift of request.drift ?? []) {
        const key = `${drift.entityType}/${drift.entityId}`
        const existing = base.get(key)
        if (existing === undefined || drift.version > existing) {
          base.set(key, drift.version)
        }
      }
    }
    const mode = base === null ? ("reset" as const) : ("diff" as const)
    const sortedKeys = [...current.keys()].sort()
    const puts = sortedKeys
      .filter((key) => {
        if (base === null) return true
        const baseVersion = base.get(key)
        return baseVersion === undefined || current.get(key)!.version > baseVersion
      })
      .map((key) => {
        const slash = key.indexOf("/")
        return new BootstrapEntity({
          entityType: EntityType.make(key.slice(0, slash)),
          entityId: EntityId.make(key.slice(slash + 1)),
          postImageJson: current.get(key)!.postImageJson,
        })
      })
    const dels =
      base === null
        ? []
        : [...base.keys()]
            .filter((key) => !current.has(key))
            .sort()
            .map((key) => {
              const slash = key.indexOf("/")
              return new CvrDel({
                entityType: EntityType.make(key.slice(0, slash)),
                entityId: EntityId.make(key.slice(slash + 1)),
              })
            })

    const newVersion = Math.max(0, ...perScope.keys()) + 1
    perScope.set(newVersion, {
      entries: new Map(
        [...current.entries()].map(([key, row]) => [key, row.version]),
      ),
      cursor,
    })
    const response = new CvrPullResponse({
      protocolVersion: 1,
      scope,
      mode,
      puts,
      dels,
      cvrVersion: CvrVersion.make(newVersion),
      cursor: SyncVersionWatermark.make(cursor),
    })
    this.cvrResponses.push(response)
    return response
  }
}

const transportOf = (server: CvrFakeServer): KhalaSyncTransport => {
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
    push: () =>
      attempt(
        () =>
          new PushResponse({ protocolVersion: 1, results: [], lastMutationId: 0 }),
      ),
    connectLive: (scope, cursor, handlers) =>
      attempt(() => server.connect(scope, cursor, handlers)),
    cvrPull: (request) => attempt(() => server.cvrPull(request)),
  }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

const makeHarness = (
  server: CvrFakeServer,
  input: Readonly<{ cvrRecovery: boolean; name: string }>,
) => {
  const store = openKhalaSyncStore(":memory:")
  cleanups.push(() => Effect.runSync(Effect.ignore(store.close())))
  const overlay = Effect.runSync(createOverlay(store, []))
  const session = createKhalaSyncSession(
    {
      baseUrl: "http://fake.test",
      clientGroupId: ClientGroupId.make(`cg_${input.name}`),
      clientId: ClientId.make(`c_${input.name}`),
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
      cvrRecovery: input.cvrRecovery,
    },
  )
  cleanups.push(() => Effect.runSync(session.close()))
  const endState = (scope: SyncScope): Record<string, string> =>
    Object.fromEntries(
      Effect.runSync(store.readEntities(scope)).map((entity) => [
        `${entity.entityType}/${entity.entityId}`,
        entity.postImageJson,
      ]),
    )
  const cursor = (scope: SyncScope): number | null =>
    Effect.runSync(store.cursor(scope))
  return { store, session, endState, cursor }
}

const image = (id: string, rev: number): string => canonicalJson({ id, rev })

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("khala-sync session CVR recovery (KS-7.2, fake transport, injected time)", () => {
  test("flag OFF (default): must_refetch recovery never touches cvrPull — zero behavior change", async () => {
    const server = new CvrFakeServer()
    server.commit(scopeA, [
      { entityType: "task", entityId: "a", op: "upsert", postImageJson: image("a", 1) },
    ])
    const h = makeHarness(server, { cvrRecovery: false, name: "off" })
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")

    server.emitMustRefetch(scopeA)
    await waitFor(
      () => server.bootstrapCalls.length >= 2,
      "re-bootstrap after MustRefetch",
    )
    await waitFor(() => h.session.state(scopeA).phase === "live", "live again")
    expect(server.cvrCalls).toHaveLength(0)
    expect(h.endState(scopeA)).toEqual({ "task/a": image("a", 1) })
  })

  test("EQUIVALENCE: flagged CVR recovery ends byte-equal to unflagged re-bootstrap — including live-drift retraction of a compacted delete", async () => {
    const server = new CvrFakeServer()
    server.commit(scopeA, [
      { entityType: "task", entityId: "a", op: "upsert", postImageJson: image("a", 1) },
    ])
    server.commit(scopeA, [
      { entityType: "task", entityId: "b", op: "upsert", postImageJson: image("b", 1) },
    ])

    // Two clients over the SAME history: flagged and unflagged. (One live
    // socket per scope in this fake ⇒ drive them sequentially per phase.)
    const flagged = makeHarness(server, { cvrRecovery: true, name: "flagged" })
    const control = makeHarness(server, { cvrRecovery: false, name: "control" })

    // Phase 1 — flagged client: initial bootstrap → live, then a first
    // must_refetch so it acquires a CVR (reset-mode pull).
    await Effect.runPromise(flagged.session.subscribe(scopeA))
    await waitFor(() => flagged.session.state(scopeA).phase === "live", "flagged live")
    server.emitMustRefetch(scopeA)
    await waitFor(() => server.cvrCalls.length >= 1, "first cvr pull")
    await waitFor(() => flagged.session.state(scopeA).phase === "live", "flagged live again")
    expect(server.cvrResponses[0]!.mode).toBe("reset")

    // Phase 2 — "w" is born and the flagged client applies it LIVE (the
    // drift Replicache does not have: the CVR does not contain w).
    server.commit(scopeA, [
      { entityType: "task", entityId: "w", op: "upsert", postImageJson: image("w", 1) },
    ])
    await waitFor(
      () => flagged.endState(scopeA)["task/w"] === image("w", 1),
      "flagged applied w live",
    )

    // Phase 3 — the client goes offline; w is deleted and its tombstone
    // compacted away; more changes land.
    server.offline = true
    server.closeSocket(scopeA)
    server.commit(scopeA, [{ entityType: "task", entityId: "w", op: "delete" }])
    server.commit(scopeA, [
      { entityType: "task", entityId: "c", op: "upsert", postImageJson: image("c", 1) },
    ])
    server.compactAllHistory(scopeA)

    // Phase 4 — back online: catch-up hits the retained window → refetch →
    // flagged path recovers via a DIFF pull whose dels retract w.
    const cvrCallsBefore = server.cvrCalls.length
    const bootstrapCallsBefore = server.bootstrapCalls.length
    server.offline = false
    await waitFor(
      () => server.cvrCalls.length > cvrCallsBefore,
      "diff cvr pull after retained-window refetch",
    )
    await waitFor(() => flagged.session.state(scopeA).phase === "live", "flagged recovered")

    const diffResponse = server.cvrResponses[server.cvrResponses.length - 1]!
    expect(diffResponse.mode).toBe("diff")
    expect(
      diffResponse.dels.map((d) => `${d.entityType}/${d.entityId}`),
    ).toContain("task/w")
    // The drift set carried w (version > the CVR's snapshot cursor).
    const diffRequest = server.cvrCalls[server.cvrCalls.length - 1]!
    expect(
      (diffRequest.drift ?? []).map((d) => `${d.entityType}/${d.entityId}`),
    ).toContain("task/w")
    // Recovery did NOT fall back to bootstrap.
    expect(server.bootstrapCalls.length).toBe(bootstrapCallsBefore)

    // Control client (unflagged): full re-bootstrap over the same history.
    await Effect.runPromise(control.session.subscribe(scopeA))
    await waitFor(() => control.session.state(scopeA).phase === "live", "control live")

    // THE acceptance: byte-equal end states, and w is retracted.
    expect(flagged.endState(scopeA)).toEqual(control.endState(scopeA))
    expect(flagged.endState(scopeA)["task/w"]).toBeUndefined()
    expect(flagged.endState(scopeA)).toEqual({
      "task/a": image("a", 1),
      "task/b": image("b", 1),
      "task/c": image("c", 1),
    })
    expect(flagged.cursor(scopeA)).toBe(control.cursor(scopeA))
  })

  test("flagged but cvrPull fails (e.g. unflagged server 404): recovery falls back to the plain bootstrap", async () => {
    const server = new CvrFakeServer()
    server.commit(scopeA, [
      { entityType: "task", entityId: "a", op: "upsert", postImageJson: image("a", 1) },
    ])
    const h = makeHarness(server, { cvrRecovery: true, name: "fallback" })
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")

    server.failNextCvrPull = new KhalaSyncTransportError(
      "http_status",
      false,
      "khala-sync request failed with HTTP 404",
      { status: 404 },
    )
    const bootstrapCallsBefore = server.bootstrapCalls.length
    server.emitMustRefetch(scopeA)
    await waitFor(
      () => server.bootstrapCalls.length > bootstrapCallsBefore,
      "bootstrap fallback",
    )
    await waitFor(() => h.session.state(scopeA).phase === "live", "live after fallback")
    expect(server.cvrCalls).toHaveLength(1)
    expect(h.endState(scopeA)).toEqual({ "task/a": image("a", 1) })
  })

  test("flagged and the CVR pull is DENIED (403): terminal denied phase, durable scope state cleared (invariant 7)", async () => {
    const server = new CvrFakeServer()
    server.commit(scopeA, [
      { entityType: "task", entityId: "a", op: "upsert", postImageJson: image("a", 1) },
    ])
    const h = makeHarness(server, { cvrRecovery: true, name: "denied" })
    await Effect.runPromise(h.session.subscribe(scopeA))
    await waitFor(() => h.session.state(scopeA).phase === "live", "live")
    expect(h.endState(scopeA)).toEqual({ "task/a": image("a", 1) })

    server.denyCvrPull = true
    server.emitMustRefetch(scopeA)
    await waitFor(() => h.session.state(scopeA).phase === "denied", "denied")
    expect(h.endState(scopeA)).toEqual({})
    expect(h.cursor(scopeA)).toBeNull()
    // No bootstrap fallback happened after the denial (retry cannot succeed).
    expect(server.bootstrapCalls).toHaveLength(1)
  })
})
