import { SyncScope } from "@openagentsinc/khala-sync"
import type { KhalaSyncSession } from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

/**
 * Per-(session, scope) subscriber reference counting for the mobile app.
 *
 * Multiple independent React hooks legitimately observe the SAME Khala Sync
 * scope at once: the thread-list `chat_thread` hook and the credits-chip
 * `credit_balance` hook both watch `personalScope(ownerUserId)`, and the
 * thread-messages screen mounts several `useKhalaSyncScopeEntities` hooks on
 * one `threadScope(threadId)`.
 *
 * The session's own `unsubscribe(scope)`
 * (`packages/khala-sync-client/src/session.ts`) is a DESTRUCTIVE,
 * non-refcounted teardown: it bumps the scope generation, closes the live
 * socket, and resets the scope's phase to `idle`. So when one of those hooks
 * unmounted, its cleanup tore the shared scope down out from under every other
 * hook still mounted.
 *
 * Confirmed on-device (2026-07-06, build 15, owner user
 * `scope.user.github:14167547`): the `chat_thread` scope drove all the way to
 * `live`/`ready`, the `credit_balance` hook attached, and ~7s later — when the
 * chip unmounted — the shared scope snapped back to `idle`, dropping the
 * thread list into an endless "Loading threads" spinner. Timeline:
 *
 *   chat_thread    idle -> bootstrapping -> catching_up -> live (ready)
 *   credit_balance live (ready)              // 2nd observer attaches
 *   chat_thread    idle (loading)            // chip unmounts -> shared teardown
 *
 * Reference counting here means the real `session.unsubscribe` only fires on
 * the last observer's release (N->0), so a scope any other hook still watches
 * stays live. This is intentionally kept in the mobile client layer rather
 * than the session: other callers (`khala-sync-db-collection`, the desktop
 * service) deliberately call `subscribe` several times per single
 * `unsubscribe` ("idempotent while the loop runs"), which a naive
 * session-level count would break.
 */

const sessionScopeCounts = new WeakMap<KhalaSyncSession, Map<string, number>>()

const countsFor = (session: KhalaSyncSession): Map<string, number> => {
  let counts = sessionScopeCounts.get(session)
  if (counts === undefined) {
    counts = new Map<string, number>()
    sessionScopeCounts.set(session, counts)
  }
  return counts
}

/**
 * Register one more observer of `scope`. The underlying
 * `session.subscribe(scope)` runs only on the 0->1 transition; later observers
 * attach to the already-running loop. Rejects with the same `OverlayError`
 * `session.subscribe` would, so the first observer can surface a subscription
 * failure. The count is incremented synchronously (before the first await) so
 * observers mounting in the same tick are counted correctly.
 */
export const acquireScopeSubscription = async (
  session: KhalaSyncSession,
  scope: SyncScope,
): Promise<void> => {
  const counts = countsFor(session)
  const key = String(scope)
  const next = (counts.get(key) ?? 0) + 1
  counts.set(key, next)
  if (next === 1) {
    await Effect.runPromise(session.subscribe(scope))
  }
}

/**
 * Drop one observer of `scope`. The underlying `session.unsubscribe(scope)`
 * (which closes the socket and resets the scope to `idle`) runs only on the
 * 1->0 transition, so a scope other hooks are still observing is never torn
 * down under them.
 */
export const releaseScopeSubscription = (
  session: KhalaSyncSession,
  scope: SyncScope,
): void => {
  const counts = countsFor(session)
  const key = String(scope)
  const current = counts.get(key) ?? 0
  if (current === 0) {
    // Never acquired (or already fully released): nothing to tear down. Do
    // NOT call the destructive `session.unsubscribe` for a scope we never
    // took a reference on.
    return
  }
  if (current === 1) {
    counts.delete(key)
    void Effect.runPromise(session.unsubscribe(scope))
    return
  }
  counts.set(key, current - 1)
}
