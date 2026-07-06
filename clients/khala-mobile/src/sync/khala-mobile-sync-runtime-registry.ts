/** Module-level (not React context) registry for the currently-open Khala
 * Mobile Sync runtime's `close()` — needed because `OtaUpdateGate` is
 * mounted at the app ROOT, above `KhalaAuthProvider`/the sync runtime
 * provider, specifically so OTA checking still works on the sign-in screen
 * (see `ota-update-gate.tsx`). That means it has no React-tree path to the
 * runtime to close it before reloading.
 *
 * Why this matters (real production crash, 2026-07-06, build 11): a
 * confirmed, reproducible `EXC_BAD_ACCESS`/`SIGSEGV` inside expo-sqlite's
 * native `AsyncQueue` (`pthread_mutex_lock`) hit three times in a row right
 * around `Updates.reloadAsync()` — see
 * docs/khala-mobile/2026-07-06-crash-triage-runbook.md. This is a known
 * expo-sqlite race (github.com/expo/expo issues #33754, #38168): a fresh
 * async DB request landing before a prior one has resolved, or a reload
 * tearing down the JS context while the native queue still has in-flight
 * work, corrupts the connection. `reloadAsync()` does not wait for
 * anything on its own — it tears down the JS runtime immediately. Draining
 * the sync runtime's own SQLite session/store closed FIRST, and only then
 * reloading, gives the native queue a chance to finish before the JS
 * context (and the SQLite native module bound to it) goes away. */

type CloseFn = () => Promise<void>

let activeClose: CloseFn | null = null

/** Called by the sync runtime provider once it has a runtime open. Returns
 * an unregister function to call from the SAME effect's cleanup — never
 * leaves a stale reference to a runtime that has already closed. */
export const registerActiveSyncRuntimeClose = (close: CloseFn): (() => void) => {
  activeClose = close
  return () => {
    if (activeClose === close) activeClose = null
  }
}

/** Only for tests: force a clean slate between cases. */
export const clearActiveSyncRuntimeCloseForTests = (): void => {
  activeClose = null
}

/** Best-effort, bounded drain before a reload. If no runtime is open
 * (signed out, still on the sign-in screen), this is an instant no-op —
 * exactly the common case an OTA reload most often fires in. A close that
 * hangs must never block the reload forever, hence the timeout race. */
export const closeActiveSyncRuntimeBeforeReload = async (
  timeoutMs = 2000,
): Promise<void> => {
  const close = activeClose
  if (close === null) return
  await Promise.race([
    close().catch(() => undefined),
    new Promise<void>(resolve => setTimeout(resolve, timeoutMs)),
  ])
}
