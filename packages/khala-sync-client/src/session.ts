import {
  BootstrapRequest,
  type BootstrapResponse,
  type ClientGroupId,
  type ClientId,
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
 * `random`), so tests run instantly and deterministically.
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
}

export interface KhalaSyncSession {
  /**
   * Start syncing a scope (idempotent while its loop runs). Store has a
   * durable cursor → catch up, then live; no cursor → bootstrap → catch up
   * → live. The loop reconnects forever (jittered exponential backoff)
   * until {@link unsubscribe} / {@link close}.
   */
  readonly subscribe: (
    scope: SyncScope,
  ) => Effect.Effect<void, OverlayError>
  /** Stop the scope's loop and close its socket. State returns to `idle`. */
  readonly unsubscribe: (scope: SyncScope) => Effect.Effect<void>
  readonly state: (scope: SyncScope) => ScopeSyncState
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
  ) => Effect.Effect<void, OverlayError>
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
  const backoffBaseMs = options.backoffBaseMs ?? 500
  const backoffMaxMs = options.backoffMaxMs ?? 30_000
  const maxBootstrapAttempts = options.maxBootstrapAttempts ?? 8
  const logPageLimit = options.logPageLimit ?? 500
  const pushBatchSize = options.pushBatchSize ?? 50
  const onTransportError = options.onTransportError

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
        return snapshot.cursor
      } catch (error) {
        if (stale()) return undefined
        onTransportError?.("bootstrap", error)
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
    while (!stale()) {
      try {
        const durable = await runEffect(store.cursor(scope))
        let cursor: SyncVersionWatermark = watermark(durable ?? 0)
        if (durable === null || runtime.forceBootstrap) {
          const bootstrapped = await bootstrapScope(scope, runtime, generation)
          if (bootstrapped === undefined) return // stale or parked in must_refetch
          runtime.forceBootstrap = false
          cursor = bootstrapped
        }
        const caughtUp = await catchUp(scope, runtime, generation, cursor)
        if (caughtUp === undefined) return // stale
        const outcome = await liveTail(scope, runtime, generation, caughtUp, () => {
          reconnectAttempt = 0
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
        if (isRefetchSignal(outcome.error)) {
          setState(scope, runtime, {
            phase: "must_refetch",
            reason: "cursor_behind_retained_window",
          })
          runtime.forceBootstrap = true
          continue
        }
        // Socket closed/errored: reconnect from the DURABLE cursor.
        reconnectAttempt += 1
        await backoff(reconnectAttempt)
      } catch (error) {
        if (stale()) return
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
  ): Effect.Effect<void, OverlayError> =>
    Effect.asVoid(
      Effect.tap(overlay.mutate(mutator, args), () =>
        Effect.sync(() => {
          kickPush()
        }),
      ),
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
