import { describe, expect, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"
import { Effect } from "effect"
import { SyncScope } from "@openagentsinc/khala-sync"

import {
  useKhalaSyncScopeEntities,
  type KhalaSyncScopeEntitiesState,
} from "../src/sync/use-khala-sync-scope-entities"

/**
 * Real production bug (2026-07-06, reported on build 13): a scope can sit
 * in the sync session's "bootstrapping"/"catching_up" phase indefinitely —
 * a genuine hang, never rejecting, never reaching "live", and never
 * reaching the session's own "must_refetch" give-up phase either (that
 * phase is set only once the session's bounded retries are EXHAUSTED; a
 * request that never settles at all never gets there). The already-shipped
 * `must_refetch` fix (resolve-scope-entities-status.test.ts) does not cover
 * this — this test proves the hard client-side watchdog backstop does.
 *
 * Oracle for khala_mobile.sync.stuck_loading_watchdog.v1
 */
const tick = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0))
const flush = async (ticks = 5): Promise<void> => {
  for (let i = 0; i < ticks; i++) await tick()
}

describe("useKhalaSyncScopeEntities watchdog", () => {
  test("a scope stuck loading forever (never live, never must_refetch) force-errors after watchdogMs — and the error is STICKY through the session's retry-cycle churn", async () => {
    const scope = "scope.user.watchdog-test"
    let notifyStateChange: ((changedScope: unknown) => void) | undefined

    const session = {
      state: () => ({ phase: "catching_up" as const }),
      subscribe: () => Effect.succeed(undefined),
      subscribeState: (callback: (changedScope: unknown) => void) => {
        notifyStateChange = callback
        return () => undefined
      },
      unsubscribe: () => Effect.succeed(undefined),
    }
    const store = {
      readEntities: () => Effect.succeed([]),
    }
    const overlay = {
      subscribe: () => () => undefined,
    }

    const states: Array<KhalaSyncScopeEntitiesState<unknown>> = []
    const Harness = () => {
      const state = useKhalaSyncScopeEntities({
        decode: value => value,
        entityType: "chat_thread",
        overlay: overlay as never,
        scope,
        session: session as never,
        store: store as never,
        watchdogMs: 20,
      })
      states.push(state)
      return null
    }

    await act(async () => {
      createTestRenderer(React.createElement(Harness))
      await flush(3)
    })

    // Before the watchdog fires: stuck loading, exactly the reported bug.
    expect(states[states.length - 1]!.status).toBe("loading")

    await act(async () => {
      await new Promise(resolve => setTimeout(resolve, 40))
    })

    const latest = states[states.length - 1]!
    expect(latest.status).toBe("error")
    expect(latest.error).not.toBeNull()
    expect(latest.items).toEqual([])

    // STICKINESS (the observed build-13 regression of the first watchdog
    // attempt): the session's infinite reconnect loop re-enters
    // catching_up on every retry and notifies subscribeState — each
    // notification triggers refresh(), which previously recomputed
    // "loading" from the still-unresolved phase and OVERWROTE the watchdog
    // error within a second ("error flashes for 1s, then Loading threads
    // again"). After the watchdog has fired, retry churn must never flip
    // the surfaced error back to loading.
    await act(async () => {
      notifyStateChange?.(SyncScope.make(scope))
      await flush(3)
    })
    const afterChurn = states[states.length - 1]!
    expect(afterChurn.status).toBe("error")
    expect(afterChurn.error).not.toBeNull()
  })

  test("resolving to live BEFORE the watchdog fires never force-errors afterward", async () => {
    const scope = "scope.user.watchdog-resolves"
    let phase: "bootstrapping" | "live" = "bootstrapping"
    let notifyStateChange: ((changedScope: unknown) => void) | undefined

    const session = {
      state: () => ({ phase }),
      subscribe: () => Effect.succeed(undefined),
      subscribeState: (callback: (changedScope: unknown) => void) => {
        notifyStateChange = callback
        return () => undefined
      },
      unsubscribe: () => Effect.succeed(undefined),
    }
    const store = {
      readEntities: () => Effect.succeed([]),
    }
    const overlay = {
      subscribe: () => () => undefined,
    }

    const states: Array<KhalaSyncScopeEntitiesState<unknown>> = []
    const Harness = () => {
      const state = useKhalaSyncScopeEntities({
        decode: value => value,
        entityType: "chat_thread",
        overlay: overlay as never,
        scope,
        session: session as never,
        store: store as never,
        // Real production items resolve as "ready" only via item count or
        // "live" phase (resolve-scope-entities-status.test.ts); "live" with
        // zero items still counts as resolved here, matching a confirmed-
        // empty scope.
        watchdogMs: 30,
      })
      states.push(state)
      return null
    }

    await act(async () => {
      createTestRenderer(React.createElement(Harness))
      await flush(2)
    })
    expect(states[states.length - 1]!.status).toBe("loading")

    // Real resolution: phase flips to "live" and the session notifies its
    // subscribeState listener, exactly like a real driveScope() completing
    // — well before the 30ms watchdog fires.
    phase = "live"
    await act(async () => {
      notifyStateChange?.(SyncScope.make(scope))
      await flush(2)
    })
    expect(states[states.length - 1]!.status).toBe("ready")

    // Watchdog window passes; the already-resolved state must NOT flip
    // back to error.
    await act(async () => {
      await new Promise(resolve => setTimeout(resolve, 50))
    })
    const latest = states[states.length - 1]!
    expect(latest.status).toBe("ready")
    expect(latest.error).toBeNull()
  })
})
