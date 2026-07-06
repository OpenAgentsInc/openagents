import { afterEach, describe, expect, mock, test } from "bun:test"

import {
  clearActiveSyncRuntimeCloseForTests,
  closeActiveSyncRuntimeBeforeReload,
  registerActiveSyncRuntimeClose,
} from "../src/sync/khala-mobile-sync-runtime-registry"

// Oracle for khala_mobile.sync.reload_drains_sqlite_runtime_first.v1
describe("khala-mobile-sync-runtime-registry", () => {
  afterEach(() => {
    clearActiveSyncRuntimeCloseForTests()
  })

  test("no runtime registered (e.g. signed out, still on the sign-in screen): instant no-op, never calls anything", async () => {
    await closeActiveSyncRuntimeBeforeReload(50)
    // Nothing to assert beyond "resolves promptly" — covered by the test
    // timeout itself; this is the common case OTA reload fires in.
  })

  test("a registered runtime is closed before the drain resolves", async () => {
    const close = mock(async () => undefined)
    registerActiveSyncRuntimeClose(close)

    await closeActiveSyncRuntimeBeforeReload(1000)

    expect(close).toHaveBeenCalledTimes(1)
  })

  test("a close() that rejects is swallowed — draining never blocks/crashes the reload", async () => {
    const close = mock(async () => {
      throw new Error("store.close() failed")
    })
    registerActiveSyncRuntimeClose(close)

    await expect(closeActiveSyncRuntimeBeforeReload(1000)).resolves.toBeUndefined()
  })

  test("a close() that hangs is bounded by the timeout, not awaited forever", async () => {
    const close = mock(() => new Promise<void>(() => {}))
    registerActiveSyncRuntimeClose(close)

    const start = Date.now()
    await closeActiveSyncRuntimeBeforeReload(30)
    expect(Date.now() - start).toBeLessThan(500)
  })

  test("unregister only clears the registry if it still holds THAT close (no stale-clear race)", async () => {
    const firstClose = mock(async () => undefined)
    const unregisterFirst = registerActiveSyncRuntimeClose(firstClose)

    const secondClose = mock(async () => undefined)
    registerActiveSyncRuntimeClose(secondClose)

    // A late-arriving unmount cleanup for the FIRST runtime must not clobber
    // the second runtime's now-active registration.
    unregisterFirst()

    await closeActiveSyncRuntimeBeforeReload(1000)
    expect(secondClose).toHaveBeenCalledTimes(1)
    expect(firstClose).not.toHaveBeenCalled()
  })
})
