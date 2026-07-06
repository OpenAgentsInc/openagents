import {
  BootstrapRequest,
  type BootstrapResponse,
  ChangelogEntry,
  type ClientGroupId,
  type ClientId,
  CvrDriftEntry,
  CvrPullRequest,
  type CvrPullResponse,
  CvrVersion,
  EntityId,
  EntityType,
  type MutationEnvelope,
  MutationId,
  type MutationResult,
  type MustRefetchReason,
  PushRequest,
  PushResponse,
  type SyncSchemaVersion,
  type SyncScope,
  SyncVersion,
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import { Cause, Effect, Exit, Queue, Stream } from "effect"
import type { ClientMutator, KhalaSyncOverlay, OverlayError } from "./overlay.js"
import type { ConfirmedEntity, KhalaSyncLocalStore } from "./store.js"
import {
  isAccessDeniedSignal,
  isRefetchSignal,
  KhalaSyncTransportError,
  type KhalaSyncTransport,
  type LiveSocket,
} from "./transport.js"

/**
 * Khala Sync client session (KS-5.3; SPEC §3 wire protocol, §6 client
 * engine): the per-scope state machine
 * `idle → bootstrapping → catching_up → live`, with `must_refetch` from any
 * state, plus the push loop that drains the durable pending-mutation queue.
 *
 * Ground rules (SPEC §7):
 * - The DURABLE CURSOR, not the connection, is the source of truth.
 *   Reconnect always resumes catch-up from `store.cursor(scope)`.
 * - Delivery is at-least-once; apply is idempotent. Duplicate or stale
 *   `DeltaFrame`s (cursor ≤ current) are skipped — the store would apply
 *   them as no-ops anyway.
 * - Rejected mutations ACK in-band (they advance `lastMutationId` and leave
 *   the queue); the session surfaces them through `onRejection` and keeps
 *   draining.
 * - v1 offline contract is ONLINE-OPTIMISTIC: reads and optimistic mutates
 *   work offline (overlay + durable queue), pushes simply wait — transient
 *   transport failure retries with jittered exponential backoff and the
 *   queue stays intact until connectivity returns.
 *
 * No wall-clock reads in logic paths: timing is injected (`sleep`,
 * `random`, `now`), so tests run instantly and deterministically.
 *
 * Freshness primitives (KS-9.2, behavior contract
 * `khala_sync.client.staleness_never_fabricated.v1`): consuming surfaces
 * must derive freshness from {@link KhalaSyncSession.state} (the real
 * phase) plus {@link KhalaSyncSession.lastDeltaAt} (the injected-clock
 * time of the last server-confirmed apply) — never from a fabricated
 * "live" default. Pending-vs-confirmed visibility (KS-9.2, behavior
 * contract `khala_sync.client.offline_pushes_queue_honestly.v1`) comes
 * from {@link KhalaSyncSession.pending}: everything it returns is queued
 * optimistic intent, NOT server-confirmed truth.
 */

// ---------------------------------------------------------------------------
// Public state model
// ---------------------------------------------------------------------------

export type ScopeSyncState =
  | { readonly phase: "idle" }
  | { readonly phase: "bootstrapping" }
  | { readonly phase: "catching_up"; readonly cursor: SyncVersionWatermark }
  | { readonly phase: "live"; readonly cursor: SyncVersionWatermark }
  | { readonly phase: "must_refetch"; readonly reason: string }
  /**
   * TERMINAL: scope access was denied. Two ways in (ST-7, #8513):
   *
   * - `reason: "access_denied"` — KS-7.1 revocation (e.g.
   *   `MustRefetch(access_changed)` followed by a 403 re-bootstrap, or any
   *   `unauthorized_scope`/403 read).
   * - `reason: "auth_rejected"` — the live connect was REJECTED as
   *   unauthenticated (HTTP 401 / `unauthenticated`) past the bounded
   *   token-rotation retry budget. A 401 loop never self-heals; parking it
   *   surfaces a crisp denial instead of an infinite "Loading" spinner.
   *
   * Either way the scope's durable local state has been CLEARED
   * (invariant 7: revocation retracts synced state; an unauthenticatable
   * client must not keep presenting synced data as entitled) and the loop
   * has stopped — no automatic retry. A fresh `subscribe(scope)` (e.g.
   * after obtaining a new token) is the only way to try again.
   */
  | { readonly phase: "denied"; readonly reason: string }

export interface KhalaSyncSessionConfig {
  readonly baseUrl: string
  readonly clientGroupId: ClientGroupId
  readonly clientId: ClientId
  readonly schemaVersion: SyncSchemaVersion
  readonly authToken: () => string
}

export interface KhalaSyncSessionOptions {
  /** Injected timer; defaults to real `setTimeout`. Tests inject instant sleeps. */
  readonly sleep?: (ms: number) => Promise<void>
  /** Injected jitter source in [0, 1); defaults to `Math.random`. */
  readonly random?: () => number
  /**
   * Injected clock for {@link KhalaSyncSession.lastDeltaAt} stamps;
   * defaults to `Date.now`. Tests inject a deterministic counter.
   */
  readonly now?: () => number
  /** Backoff base delay (default 500ms). */
  readonly backoffBaseMs?: number
  /** Backoff ceiling (default 30s). */
  readonly backoffMaxMs?: number
  /** Bounded retries for a (re-)bootstrap before parking in `must_refetch`. */
  readonly maxBootstrapAttempts?: number
  /** `GET /api/sync/log` page size (default 500). */
  readonly logPageLimit?: number
  /** Max mutations per `POST /api/sync/push` batch (default 50). */
  readonly pushBatchSize?: number
  /**
   * KS-7.2 (#8306), default OFF: when true AND the transport provides
   * `cvrPull`, the `must_refetch` recovery path tries the flag-gated CVR
   * diff pull FIRST (docs/khala-sync/CVR_DESIGN.md) — dels retract rows
   * that left the authorized set (deleted+compacted, or permission-driven)
   * without replacing the whole scope. ANY CVR failure other than an
   * access denial falls back to the plain bootstrap, so unflagged servers
   * (404 on the route) degrade to exactly today's behavior. The very
   * first sync of a scope (no durable cursor) always uses bootstrap.
   */
  readonly cvrRecovery?: boolean
  /**
   * Drift-set upload bound: when more local rows changed since the last
   * CVR than this, request a reset-mode pull instead of shipping a huge
   * drift list (default 5000).
   */
  readonly maxDriftEntries?: number
  /** Rejected mutation results, surfaced as they are acked in-band. */
  readonly onRejection?: (
    result: MutationResult,
    mutation: MutationEnvelope | undefined,
  ) => void
  /** Observability tap for retried/terminal transport faults. Never throws. */
  readonly onTransportError?: (
    context: "bootstrap" | "catch_up" | "live" | "push" | "session",
    error: unknown,
  ) => void
  /**
   * ST-7 (#8513) connect-failure tripwire: fires when a scope has failed
   * `connectLive` `connectFailureThreshold` consecutive times without a
   * single successful connect in between (and again at every further
   * multiple, so a long outage stays visible without firing per attempt).
   * Hosts should forward this bounded, structured signal into their
   * observability pipe (for the openagents.com Worker family: Analytics
   * Engine / Tail Worker via an existing ingest route) so a fleet-wide
   * connect-failure spike pages within minutes of a bad deploy instead of
   * hiding behind an infinite "Loading" state. Must not throw.
   */
  readonly onConnectFailure?: (signal: ConnectFailureSignal) => void
  /**
   * Consecutive `connectLive` failures before {@link onConnectFailure}
   * fires (default 5). A successful connect resets the streak.
   */
  readonly connectFailureThreshold?: number
  /**
   * ST-7 (#8513) 401-on-connect budget: total consecutive auth-REJECTED
   * (HTTP 401 / `unauthenticated`) `connectLive` attempts allowed before
   * the scope parks TERMINALLY in `denied` (`reason: "auth_rejected"`).
   * Default 2 — i.e. one bounded re-attempt so a transient 401 during
   * token rotation can heal (`authToken()` is re-read per attempt), while
   * a genuinely rejected token stops looping after the second refusal
   * instead of retrying an unauthenticatable connect forever. A non-401
   * failure or a successful connect resets the budget.
   */
  readonly maxConnectAuthRejections?: number
}

/**
 * Structured repeated-connect-failure signal (ST-7, #8513). Bounded: at
 * most one signal per {@link KhalaSyncSessionOptions.connectFailureThreshold}
 * consecutive failures, and the streak resets on any successful connect.
 */
export interface ConnectFailureSignal {
  readonly scope: SyncScope
  /** Consecutive failed `connectLive` attempts since the last success. */
  readonly consecutiveFailures: number
  /** Transport error taxonomy (`"unknown"` for non-transport throwables). */
  readonly reason: "network" | "http_status" | "decode_failure" | "sync_error" | "unknown"
  /** HTTP status when the transport error carried one (e.g. 401). */
  readonly status?: number
}

export interface KhalaSyncSession {
  /**
   * Start syncing a scope (idempotent while its loop runs). Store has a
   * durable cursor → catch up, then live; no cursor → bootstrap → catch up
   * → live. TRANSIENT faults reconnect forever (jittered exponential
   * backoff) until {@link unsubscribe} / {@link close}; access denials
   * (403) and auth rejections (401 past the bounded rotation budget) park
   * the scope TERMINALLY in `denied` instead (ST-7, #8513).
   */
  readonly subscribe: (
    scope: SyncScope,
  ) => Effect.Effect<void, OverlayError>
  /** Stop the scope's loop and close its socket. State returns to `idle`. */
  readonly unsubscribe: (scope: SyncScope) => Effect.Effect<void>
  readonly state: (scope: SyncScope) => ScopeSyncState
  /**
   * Freshness primitive (KS-9.2,
   * `khala_sync.client.staleness_never_fabricated.v1`): the injected-clock
   * timestamp of the scope's most recent SERVER-CONFIRMED apply (bootstrap
   * snapshot, catch-up page, or live delta). `null` when nothing confirmed
   * has ever landed — and again after a denial clears the scope (revoked
   * data must not keep claiming freshness). Surfaces derive staleness from
   * this plus {@link state}; they must never default to "live".
   */
  readonly lastDeltaAt: (scope: SyncScope) => number | null
  /**
   * Pending-vs-confirmed exposure (KS-9.2,
   * `khala_sync.client.offline_pushes_queue_honestly.v1`): the still
   * unconfirmed queued mutations, ascending mutationId. UI surfaces use
   * this to mark optimistic content as pending instead of presenting it
   * as server-confirmed.
   */
  readonly pending: () => ReadonlyArray<MutationEnvelope>
  /**
   * State-transition notifications, shaped like the overlay's `subscribe`:
   * `listener(scope, state)` per transition; returns unsubscribe.
   */
  readonly subscribeState: (
    listener: (scope: SyncScope, state: ScopeSyncState) => void,
  ) => () => void
  /** Content-change notifications (overlay-backed) as an Effect Stream. */
  readonly changes: Stream.Stream<SyncScope>
  /** Optimistic mutate (overlay) + kick the push loop. */
  readonly mutate: <Args>(
    mutator: ClientMutator<Args>,
    args: Args,
  ) => Effect.Effect<MutationId, OverlayError>
  /** Stop all loops and sockets. The session cannot be restarted. */
  readonly close: () => Effect.Effect<void>
}

// ---------------------------------------------------------------------------
// Backoff (pure, injected randomness — no Date.now anywhere)
// ---------------------------------------------------------------------------

/**
 * Jittered exponential backoff: cap = min(maxMs, baseMs · 2^(attempt−1)),
 * result uniform in [cap/2, cap). Pure — the caller injects `random`.
 */
export const computeBackoffMs = (
  attempt: number,
  baseMs: number,
  maxMs: number,
  random: () => number,
): number => {
  const exponent = Math.max(0, attempt - 1)
  const cap = Math.min(maxMs, baseMs * 2 ** exponent)
  return cap / 2 + random() * (cap / 2)
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

const PROTOCOL_VIOLATION = (message: string): KhalaSyncTransportError =>
  new KhalaSyncTransportError("decode_failure", false, message)

/**
 * ST-7 (#8513): the connect was auth-REJECTED — an HTTP 401 or a typed
 * `unauthenticated` SyncError. Distinct from `isAccessDeniedSignal` (403 /
 * `unauthorized_scope`, which parks immediately): a 401 gets a small
 * bounded retry budget first because `authToken()` is re-read per attempt
 * and token rotation can heal a momentary rejection — but past the budget
 * it parks terminally, because an unauthenticatable connect retried
 * forever is exactly the silent-spinner failure that hid the mobile
 * WS-auth server bug for 4 builds
 * (docs/khala-code/2026-07-06-mobile-loading-threads-websocket-auth-audit.md).
 */
const isAuthRejectedSignal = (error: unknown): boolean =>
  error instanceof KhalaSyncTransportError &&
  (error.details?.status === 401 ||
    (error.reason === "sync_error" &&
      error.details?.syncError?.code === "unauthenticated"))

type LiveOutcome =
  | { readonly kind: "must_refetch"; readonly reason: MustRefetchReason }
  | { readonly kind: "closed"; readonly error?: unknown }
  | { readonly kind: "connect_failed"; readonly error: unknown }

interface ScopeRuntime {
  generation: number
  loopRunning: boolean
  state: ScopeSyncState
  socket: LiveSocket | null
  /** Set by MustRefetch (or refetch-signal errors): next pass re-bootstraps. */
  forceBootstrap: boolean
  /** Injected-clock time of the last server-confirmed apply; null = never. */
  lastDeltaAt: number | null
  /**
   * The CVR the durable store was last reconciled to (KS-7.2): its server
   * version and its snapshot cursor (drift = local rows newer than that
   * cursor). Session-lifetime only — `null` after restart, plain
   * bootstrap, or denial, which simply makes the next CVR pull reset-mode
   * (always sound; see CVR_DESIGN.md §5).
   */
  cvr: { readonly version: number; readonly cursor: number } | null
}

/** Run a typed Effect from promise-land, rethrowing the TYPED error. */
const runEffect = async <A, E>(effect: Effect.Effect<A, E>): Promise<A> => {
  const exit = await Effect.runPromiseExit(effect)
  if (Exit.isSuccess(exit)) return exit.value
  throw Cause.squash(exit.cause)
}

const watermark = SyncVersionWatermark.make

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

export const createKhalaSyncSession = (
  config: KhalaSyncSessionConfig,
  store: KhalaSyncLocalStore,
  overlay: KhalaSyncOverlay,
  transport: KhalaSyncTransport,
  options: KhalaSyncSessionOptions = {},
): KhalaSyncSession => {
  const sleep =
    options.sleep ??
    ((ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms)))
  const random = options.random ?? Math.random
  const now = options.now ?? Date.now
  const backoffBaseMs = options.backoffBaseMs ?? 500
  const backoffMaxMs = options.backoffMaxMs ?? 30_000
  const maxBootstrapAttempts = options.maxBootstrapAttempts ?? 8
  const logPageLimit = options.logPageLimit ?? 500
  const pushBatchSize = options.pushBatchSize ?? 50
  const cvrRecovery = options.cvrRecovery ?? false
  const maxDriftEntries = options.maxDriftEntries ?? 5_000
  const onTransportError = options.onTransportError
  const onConnectFailure = options.onConnectFailure
  const connectFailureThreshold = options.connectFailureThreshold ?? 5
  const maxConnectAuthRejections = options.maxConnectAuthRejections ?? 2

  const backoff = (attempt: number): Promise<void> =>
    sleep(computeBackoffMs(attempt, backoffBaseMs, backoffMaxMs, random))

  const scopes = new Map<SyncScope, ScopeRuntime>()
  const stateListeners = new Set<
    (scope: SyncScope, state: ScopeSyncState) => void
  >()
  let closed = false

  const setState = (
    scope: SyncScope,
    runtime: ScopeRuntime,
    state: ScopeSyncState,
  ): void => {
    runtime.state = state
    for (const listener of [...stateListeners]) listener(scope, state)
  }

  // -- access denial (terminal per scope) --------------------------------------

  /**
   * Park a scope in the TERMINAL `denied` phase after an authorization
   * denial (KS-7.1; SPEC §7 invariant 7) or an exhausted 401 budget
   * (ST-7, #8513; `reason: "auth_rejected"`): CLEAR the scope's durable
   * local rows + cursor (revocation retracts synced state — the data must
   * not survive locally after access is gone, and an unauthenticatable
   * client gets the same treatment) and rebuild the overlay, then stop.
   * Clearing failures are surfaced through `onTransportError` but never
   * keep revoked data live-retryable: the scope parks regardless.
   */
  const parkDenied = async (
    scope: SyncScope,
    runtime: ScopeRuntime,
    reason: "access_denied" | "auth_rejected" = "access_denied",
  ): Promise<void> => {
    try {
      await runEffect(store.resetScope(scope, [], watermark(0)))
      await runEffect(overlay.refetched(scope))
    } catch (error) {
      onTransportError?.("session", error)
    }
    runtime.forceBootstrap = false
    // The synced data is gone; its freshness stamp must not survive it.
    runtime.lastDeltaAt = null
    runtime.cvr = null
    setState(scope, runtime, { phase: "denied", reason })
  }

  // -- bootstrap --------------------------------------------------------------

  /** Fetch the full snapshot (all pages, one token chain) atomically. */
  const fetchSnapshot = async (
    scope: SyncScope,
  ): Promise<{
    entities: ReadonlyArray<ConfirmedEntity>
    cursor: SyncVersionWatermark
  }> => {
    const collected: Array<{
      entityType: string
      entityId: string
      postImageJson: string
    }> = []
    let pageToken: string | undefined = undefined
    for (;;) {
      const response: BootstrapResponse = await runEffect(
        transport.bootstrap(
          new BootstrapRequest({
            protocolVersion: 1,
            schemaVersion: config.schemaVersion,
            scope,
            clientGroupId: config.clientGroupId,
            ...(pageToken !== undefined ? { pageToken } : {}),
          }),
        ),
      )
      if (response.scope !== scope) {
        throw PROTOCOL_VIOLATION("bootstrap response is for a different scope")
      }
      collected.push(...response.entities)
      if (response.nextPageToken !== undefined) {
        pageToken = response.nextPageToken
        continue
      }
      if (response.cursor === undefined) {
        throw PROTOCOL_VIOLATION("final bootstrap page is missing its cursor")
      }
      const cursor = response.cursor
      if (cursor === 0 && collected.length > 0) {
        throw PROTOCOL_VIOLATION(
          "bootstrap snapshot has entities at watermark 0",
        )
      }
      return {
        // Snapshot entities carry the snapshot cursor as their version:
        // every entry ≤ cursor is already reflected, so later catch-up
        // entries (version > cursor) overwrite correctly.
        entities: collected.map((entity) => ({
          entityType: entity.entityType,
          entityId: entity.entityId,
          postImageJson: entity.postImageJson,
          version: SyncVersion.make(cursor),
        })),
        cursor,
      }
    }
  }

  /**
   * Bounded-retry bootstrap: snapshot pages → `resetScope` at the final
   * cursor → overlay rebuild. Returns the snapshot cursor, or `undefined`
   * when stale or exhausted (state already reflects the outcome).
   */
  const bootstrapScope = async (
    scope: SyncScope,
    runtime: ScopeRuntime,
    generation: number,
  ): Promise<SyncVersionWatermark | undefined> => {
    const stale = (): boolean => closed || runtime.generation !== generation
    setState(scope, runtime, { phase: "bootstrapping" })
    for (let attempt = 1; attempt <= maxBootstrapAttempts; attempt++) {
      if (stale()) return undefined
      try {
        const snapshot = await fetchSnapshot(scope)
        await runEffect(
          store.resetScope(scope, snapshot.entities, snapshot.cursor),
        )
        await runEffect(overlay.refetched(scope))
        runtime.lastDeltaAt = now() // full snapshot = server-confirmed apply
        return snapshot.cursor
      } catch (error) {
        if (stale()) return undefined
        onTransportError?.("bootstrap", error)
        if (isAccessDeniedSignal(error)) {
          // 403 on (re-)bootstrap: access is gone; retrying can never
          // succeed. Clear scope-local state and park (terminal).
          await parkDenied(scope, runtime)
          return undefined
        }
        if (attempt >= maxBootstrapAttempts) {
          setState(scope, runtime, {
            phase: "must_refetch",
            reason: "bootstrap_retries_exhausted",
          })
          return undefined
        }
        await backoff(attempt)
      }
    }
    return undefined
  }

  // -- CVR recovery (KS-7.2, #8306 — flag-gated must_refetch path) --------------

  /**
   * A `committedAt` for synthesized CVR entries. The store ignores it (it
   * persists key/image/version only); a fixed epoch keeps the no-wall-clock
   * rule intact.
   */
  const CVR_SYNTHESIZED_COMMITTED_AT = "1970-01-01T00:00:00.000Z"

  /**
   * One CVR diff-pull recovery attempt (docs/khala-sync/CVR_DESIGN.md):
   * send the last CVR version + the drift set (local rows newer than that
   * CVR's snapshot), apply the response —
   *
   * - `reset`: replace scope state with `puts` (bootstrap semantics);
   * - `diff`: apply `dels` + `puts` as synthesized confirmed entries at
   *   the snapshot cursor through the overlay (store apply + rebase in one
   *   step, exactly like a log page) — rows that left the authorized set
   *   are RETRACTED without touching the rest of the scope
   *
   * — then return the snapshot cursor to stitch catch-up from. Returns
   * `"fallback"` on any failure the plain bootstrap should absorb
   * (unflagged server 404, row-set-too-large, storage faults, protocol
   * violations), and `undefined` when stale or parked (access denial is
   * TERMINAL here exactly as on the bootstrap path).
   */
  const cvrRecoverScope = async (
    scope: SyncScope,
    runtime: ScopeRuntime,
    generation: number,
  ): Promise<SyncVersionWatermark | "fallback" | undefined> => {
    const pull = transport.cvrPull
    if (pull === undefined) return "fallback"
    const stale = (): boolean => closed || runtime.generation !== generation
    setState(scope, runtime, { phase: "bootstrapping" })
    try {
      // Drift: rows applied (via log/delta) after the last CVR's snapshot.
      // Oversized drift → reset-mode pull (no cvrVersion) instead of a
      // huge upload.
      let cvrVersion: number | null = runtime.cvr?.version ?? null
      let drift: Array<CvrDriftEntry> = []
      if (runtime.cvr !== null) {
        const cvrCursor = runtime.cvr.cursor
        const entities = await runEffect(store.readEntities(scope))
        drift = entities
          .filter((entity) => entity.version > cvrCursor)
          .map(
            (entity) =>
              new CvrDriftEntry({
                entityType: EntityType.make(entity.entityType),
                entityId: EntityId.make(entity.entityId),
                version: entity.version,
              }),
          )
        if (drift.length > maxDriftEntries) {
          cvrVersion = null
          drift = []
        }
      }
      const response: CvrPullResponse = await runEffect(
        pull(
          new CvrPullRequest({
            protocolVersion: 1,
            schemaVersion: config.schemaVersion,
            scope,
            clientGroupId: config.clientGroupId,
            ...(cvrVersion !== null
              ? { cvrVersion: CvrVersion.make(cvrVersion) }
              : {}),
            ...(drift.length > 0 ? { drift } : {}),
          }),
        ),
      )
      if (stale()) return undefined
      if (response.scope !== scope) {
        throw PROTOCOL_VIOLATION("cvr-pull response is for a different scope")
      }
      const cursor = response.cursor
      if (response.mode === "reset" || cursor === 0) {
        // Reset semantics = bootstrap semantics: `puts` IS the complete
        // set (a diff at watermark 0 can only be empty — treat it as the
        // equivalent reset to keep the store's watermark-0 rule intact).
        if (cursor === 0 && response.puts.length > 0) {
          throw PROTOCOL_VIOLATION("cvr-pull snapshot has puts at watermark 0")
        }
        await runEffect(
          store.resetScope(
            scope,
            response.puts.map((put) => ({
              entityType: put.entityType,
              entityId: put.entityId,
              postImageJson: put.postImageJson,
              version: SyncVersion.make(cursor),
            })),
            cursor,
          ),
        )
        await runEffect(overlay.refetched(scope))
      } else {
        const version = SyncVersion.make(cursor)
        const entries: Array<ChangelogEntry> = [
          // Dels first (pure defensiveness — apply is keyed per entity and
          // no key appears in both sets).
          ...response.dels.map(
            (del) =>
              new ChangelogEntry({
                scope,
                version,
                entityType: del.entityType,
                entityId: del.entityId,
                op: "delete",
                committedAt: CVR_SYNTHESIZED_COMMITTED_AT,
              }),
          ),
          ...response.puts.map(
            (put) =>
              new ChangelogEntry({
                scope,
                version,
                entityType: put.entityType,
                entityId: put.entityId,
                op: "upsert",
                postImageJson: put.postImageJson,
                committedAt: CVR_SYNTHESIZED_COMMITTED_AT,
              }),
          ),
        ]
        // Overlay apply = store apply + rebase, same as a log page. An
        // empty diff still advances the durable cursor to the snapshot.
        await runEffect(overlay.onConfirmed(scope, entries, version))
      }
      runtime.cvr = { version: Number(response.cvrVersion), cursor: Number(cursor) }
      runtime.lastDeltaAt = now() // CVR pull = server-confirmed apply
      return cursor
    } catch (error) {
      if (stale()) return undefined
      onTransportError?.("bootstrap", error)
      if (isAccessDeniedSignal(error)) {
        await parkDenied(scope, runtime)
        return undefined
      }
      // Anything else — unflagged server (404), row set too large, storage
      // fault, decode failure — falls back to the plain bootstrap path.
      runtime.cvr = null
      return "fallback"
    }
  }

  // -- catch-up ---------------------------------------------------------------

  /**
   * `GET log` loop from `start` until `upToDate`. Throws on transport
   * failure (the scope loop owns retry — it re-reads the DURABLE cursor,
   * so a mid-catch-up reconnect resumes exactly where the store is).
   */
  const catchUp = async (
    scope: SyncScope,
    runtime: ScopeRuntime,
    generation: number,
    start: SyncVersionWatermark,
  ): Promise<SyncVersionWatermark | undefined> => {
    let cursor = start
    setState(scope, runtime, { phase: "catching_up", cursor })
    for (;;) {
      if (closed || runtime.generation !== generation) return undefined
      const page = await runEffect(
        transport.logPage(scope, cursor, logPageLimit),
      )
      if (page.scope !== scope) {
        throw PROTOCOL_VIOLATION("log page is for a different scope")
      }
      if (page.entries.length > 0 && page.nextCursor > cursor) {
        await runEffect(
          overlay.onConfirmed(
            scope,
            [...page.entries],
            SyncVersion.make(page.nextCursor),
          ),
        )
      }
      if (page.nextCursor > cursor) {
        cursor = page.nextCursor
        runtime.lastDeltaAt = now()
        setState(scope, runtime, { phase: "catching_up", cursor })
      }
      if (page.upToDate) return cursor
    }
  }

  // -- live tail ---------------------------------------------------------------

  /**
   * Connect the live socket at `cursor` and pump frames until it dies or
   * the server orders a refetch. Frame effects are serialized through one
   * promise chain (arrival order is apply order); a failing apply drops
   * the connection so the durable cursor stays the recovery point.
   */
  const liveTail = (
    scope: SyncScope,
    runtime: ScopeRuntime,
    generation: number,
    cursor: SyncVersionWatermark,
    onConnected: () => void,
  ): Promise<LiveOutcome> =>
    new Promise<LiveOutcome>((resolve) => {
      const stale = (): boolean => closed || runtime.generation !== generation
      let settled = false
      let current = cursor
      let socketRef: LiveSocket | null = null
      let chain: Promise<void> = Promise.resolve()

      const settle = (outcome: LiveOutcome): void => {
        if (settled) return
        settled = true
        runtime.socket = null
        socketRef?.close()
        resolve(outcome)
      }

      const enqueue = (task: () => Promise<void>): void => {
        chain = chain.then(async () => {
          if (settled || stale()) return
          try {
            await task()
          } catch (error) {
            // A confirmed apply that failed must not be skipped over —
            // drop the connection and resume from the durable cursor.
            onTransportError?.("live", error)
            settle({ kind: "closed", error })
          }
        })
      }

      runEffect(
        transport.connectLive(scope, cursor, {
          onFrame: (frame) => {
            if (settled || stale()) return
            switch (frame._tag) {
              case "PingFrame":
                return
              case "MutationAckFrame": {
                if (frame.clientId !== config.clientId) return
                enqueue(() => runEffect(overlay.onAck(frame.lastMutationId)))
                return
              }
              case "MustRefetchFrame": {
                if (frame.scope !== scope) return
                settle({ kind: "must_refetch", reason: frame.reason })
                return
              }
              case "DeltaFrame": {
                if (frame.scope !== scope) return
                enqueue(async () => {
                  // Duplicate / out-of-order delivery: everything through
                  // `current` is already applied (at-least-once safety).
                  if (frame.cursor <= current) return
                  await runEffect(
                    overlay.onConfirmed(scope, [...frame.entries], frame.cursor),
                  )
                  current = watermark(frame.cursor)
                  runtime.lastDeltaAt = now()
                  if (!settled && !stale()) {
                    setState(scope, runtime, { phase: "live", cursor: current })
                  }
                })
                return
              }
            }
          },
          onClose: (cause) => {
            settle({ kind: "closed", error: cause.error })
          },
        }),
      ).then(
        (socket) => {
          if (settled || stale()) {
            socket.close()
            settle({ kind: "closed" })
            return
          }
          socketRef = socket
          runtime.socket = socket
          onConnected()
          setState(scope, runtime, { phase: "live", cursor: current })
        },
        (error: unknown) => {
          settle({ kind: "connect_failed", error })
        },
      )
    })

  // -- per-scope loop ----------------------------------------------------------

  const driveScope = async (
    scope: SyncScope,
    runtime: ScopeRuntime,
    generation: number,
  ): Promise<void> => {
    const stale = (): boolean => closed || runtime.generation !== generation
    let reconnectAttempt = 0
    // ST-7 (#8513): consecutive `connectLive` failures since the last
    // successful connect — drives the bounded observability signal…
    let connectFailureStreak = 0
    // …and consecutive auth-REJECTED (401) connects — drives the bounded
    // pre-park retry budget for token rotation.
    let connectAuthRejections = 0
    while (!stale()) {
      try {
        const durable = await runEffect(store.cursor(scope))
        let cursor: SyncVersionWatermark = watermark(durable ?? 0)
        if (durable === null || runtime.forceBootstrap) {
          // KS-7.2: when flagged, must_refetch recovery tries the CVR diff
          // pull first (never the very first sync — no durable cursor means
          // nothing to diff-recover). "fallback" = plain bootstrap.
          let recovered: SyncVersionWatermark | "fallback" | undefined =
            "fallback"
          if (cvrRecovery && runtime.forceBootstrap && durable !== null) {
            recovered = await cvrRecoverScope(scope, runtime, generation)
            if (recovered === undefined) return // stale or parked denied
          }
          if (recovered === "fallback") {
            const bootstrapped = await bootstrapScope(scope, runtime, generation)
            if (bootstrapped === undefined) return // stale or parked in must_refetch
            // A plain bootstrap replaced state the CVR does not describe;
            // drop it so the next CVR pull is reset-mode (always sound).
            runtime.cvr = null
            recovered = bootstrapped
          }
          runtime.forceBootstrap = false
          cursor = recovered
        }
        const caughtUp = await catchUp(scope, runtime, generation, cursor)
        if (caughtUp === undefined) return // stale
        const outcome = await liveTail(scope, runtime, generation, caughtUp, () => {
          reconnectAttempt = 0
          connectFailureStreak = 0
          connectAuthRejections = 0
        })
        if (stale()) return
        if (outcome.kind === "must_refetch") {
          setState(scope, runtime, {
            phase: "must_refetch",
            reason: outcome.reason,
          })
          runtime.forceBootstrap = true
          continue // automatic re-bootstrap (bounded retries inside)
        }
        if (outcome.kind !== "closed" || outcome.error !== undefined) {
          onTransportError?.("live", outcome.error)
        }
        if (isAccessDeniedSignal(outcome.error)) {
          // Live tail (or its reconnect) was refused with a 403: terminal.
          await parkDenied(scope, runtime)
          return
        }
        if (isRefetchSignal(outcome.error)) {
          setState(scope, runtime, {
            phase: "must_refetch",
            reason: "cursor_behind_retained_window",
          })
          runtime.forceBootstrap = true
          continue
        }
        if (outcome.kind === "connect_failed") {
          // ST-7 (#8513): the connect itself was refused (vs a live socket
          // dying later). Count the streak and surface the bounded signal
          // so repeated silent connect failures page instead of hiding
          // behind an eternal "Loading" phase.
          connectFailureStreak += 1
          if (
            onConnectFailure !== undefined &&
            connectFailureStreak % connectFailureThreshold === 0
          ) {
            const error = outcome.error
            const transportError =
              error instanceof KhalaSyncTransportError ? error : undefined
            const status = transportError?.details?.status
            onConnectFailure({
              scope,
              consecutiveFailures: connectFailureStreak,
              reason: transportError?.reason ?? "unknown",
              ...(status !== undefined ? { status } : {}),
            })
          }
          if (isAuthRejectedSignal(outcome.error)) {
            // 401 on connect: the token was REJECTED. Allow the bounded
            // rotation budget (authToken() is re-read per attempt), then
            // park terminally — a 401 loop never self-heals and must not
            // present as an infinite spinner.
            connectAuthRejections += 1
            if (connectAuthRejections >= maxConnectAuthRejections) {
              await parkDenied(scope, runtime, "auth_rejected")
              return
            }
          } else {
            // A non-401 failure breaks the consecutive-rejection chain
            // (e.g. 401 → network blip → …): only an uninterrupted run of
            // rejections proves the token itself is bad.
            connectAuthRejections = 0
          }
        }
        // Socket closed/errored: reconnect from the DURABLE cursor.
        reconnectAttempt += 1
        await backoff(reconnectAttempt)
      } catch (error) {
        if (stale()) return
        if (isAccessDeniedSignal(error)) {
          // A 403 mid catch-up is the same revocation: terminal.
          onTransportError?.("catch_up", error)
          await parkDenied(scope, runtime)
          return
        }
        if (isRefetchSignal(error)) {
          setState(scope, runtime, {
            phase: "must_refetch",
            reason: "cursor_behind_retained_window",
          })
          runtime.forceBootstrap = true
          continue
        }
        onTransportError?.("catch_up", error)
        reconnectAttempt += 1
        await backoff(reconnectAttempt)
      }
    }
  }

  // -- push loop ---------------------------------------------------------------

  let pushRunning = false

  const drainPushQueue = async (): Promise<"drained" | "terminal"> => {
    let attempt = 0
    while (!closed) {
      const pending = overlay.pending() // ascending mutationId (FIFO)
      if (pending.length === 0) return "drained"
      const batch = pending.slice(0, pushBatchSize)
      try {
        const response: PushResponse = await runEffect(
          transport.push(
            new PushRequest({
              protocolVersion: 1,
              schemaVersion: config.schemaVersion,
              clientGroupId: config.clientGroupId,
              clientId: config.clientId,
              mutations: batch,
            }),
          ),
        )
        for (const result of response.results) {
          if (result.status === "rejected") {
            options.onRejection?.(
              result,
              batch.find((m) => m.mutationId === result.mutationId),
            )
          }
        }
        // In-band ack: applied, duplicate AND rejected all advance the
        // queue (rejections carry their error in the result, never block).
        // `lastMutationId` is the ledger watermark — 0 means nothing has
        // been acked yet (e.g. an all-out_of_order batch), so skip the ack.
        if (response.lastMutationId > 0) {
          await runEffect(
            overlay.onAck(MutationId.make(response.lastMutationId)),
          )
        }
        // Defensive: a successful push that did NOT advance the queue head
        // (e.g. the server acked nothing) must not spin — back off instead.
        const head = overlay.pending()[0]
        if (head !== undefined && head.mutationId === batch[0]!.mutationId) {
          attempt += 1
          await backoff(attempt)
        } else {
          attempt = 0
        }
      } catch (error) {
        onTransportError?.("push", error)
        if (
          error instanceof KhalaSyncTransportError &&
          !error.retryable
        ) {
          // Terminal fault (auth, protocol, decode): stop draining; the
          // queue stays intact and the next mutate/subscribe re-kicks.
          return "terminal"
        }
        // v1 online-optimistic: offline just means the queue waits.
        attempt += 1
        await backoff(attempt)
      }
    }
    return "drained"
  }

  const kickPush = (): void => {
    if (pushRunning || closed) return
    pushRunning = true
    void drainPushQueue().then(
      (outcome) => {
        pushRunning = false
        // Late arrivals between the last pending() check and the flag
        // reset: re-kick so nothing waits for the next user action. A
        // TERMINAL fault must NOT re-kick (the queue is intentionally
        // parked until the next mutate/subscribe) — re-kicking would hot
        // loop against the same non-retryable failure.
        if (outcome === "drained" && !closed && overlay.pending().length > 0) {
          kickPush()
        }
      },
      (error: unknown) => {
        pushRunning = false
        onTransportError?.("push", error)
      },
    )
  }

  // -- public surface ----------------------------------------------------------

  const subscribe = (
    scope: SyncScope,
  ): Effect.Effect<void, OverlayError> =>
    Effect.gen(function* () {
      if (closed) return
      yield* store.setIdentity({
        clientId: config.clientId,
        clientGroupId: config.clientGroupId,
        schemaVersion: config.schemaVersion,
      })
      let runtime = scopes.get(scope)
      if (runtime === undefined) {
        runtime = {
          generation: 0,
          loopRunning: false,
          state: { phase: "idle" },
          socket: null,
          forceBootstrap: false,
          lastDeltaAt: null,
          cvr: null,
        }
        scopes.set(scope, runtime)
      }
      if (runtime.loopRunning) return
      runtime.loopRunning = true
      runtime.generation += 1
      const generation = runtime.generation
      const current = runtime
      void driveScope(scope, current, generation)
        .catch((error) => {
          onTransportError?.("session", error)
        })
        .finally(() => {
          if (current.generation === generation) current.loopRunning = false
        })
      kickPush() // drain restart survivors
    })

  const unsubscribe = (scope: SyncScope): Effect.Effect<void> =>
    Effect.sync(() => {
      const runtime = scopes.get(scope)
      if (runtime === undefined) return
      runtime.generation += 1
      runtime.loopRunning = false
      runtime.forceBootstrap = false
      runtime.socket?.close()
      runtime.socket = null
      setState(scope, runtime, { phase: "idle" })
    })

  const state = (scope: SyncScope): ScopeSyncState =>
    scopes.get(scope)?.state ?? { phase: "idle" }

  const lastDeltaAt = (scope: SyncScope): number | null =>
    scopes.get(scope)?.lastDeltaAt ?? null

  const changes: Stream.Stream<SyncScope> = Stream.callback<SyncScope>(
    (queue) =>
      Effect.acquireRelease(
        Effect.sync(() =>
          overlay.subscribe((scope) => {
            Queue.offerUnsafe(queue, scope)
          }),
        ),
        (unsubscribeOverlay) => Effect.sync(() => unsubscribeOverlay()),
      ),
  )

  const mutate = <Args>(
    mutator: ClientMutator<Args>,
    args: Args,
  ): Effect.Effect<MutationId, OverlayError> =>
    Effect.tap(
      overlay.mutate(mutator, args),
      () =>
        Effect.sync(() => {
          kickPush()
        }),
    )

  const close = (): Effect.Effect<void> =>
    Effect.sync(() => {
      closed = true
      for (const [scope, runtime] of scopes) {
        runtime.generation += 1
        runtime.loopRunning = false
        runtime.socket?.close()
        runtime.socket = null
        setState(scope, runtime, { phase: "idle" })
      }
    })

  return {
    subscribe,
    unsubscribe,
    state,
    lastDeltaAt,
    pending: () => overlay.pending(),
    subscribeState: (listener) => {
      stateListeners.add(listener)
      return () => {
        stateListeners.delete(listener)
      }
    },
    changes,
    mutate,
    close,
  }
}
