import { describe, expect, test } from "bun:test"

import { resolveScopeEntitiesStatusAndError } from "../src/sync/use-khala-sync-scope-entities"

// Oracle for khala_mobile.sync.must_refetch_never_stuck_loading.v1
describe("resolveScopeEntitiesStatusAndError", () => {
  test("denied: error, with the access-denied message", () => {
    expect(resolveScopeEntitiesStatusAndError("denied", 0)).toEqual({
      error: "Khala Sync scope access was denied",
      status: "error",
    })
  })

  test("must_refetch with zero items: error, never silently 'loading' forever", () => {
    const result = resolveScopeEntitiesStatusAndError("must_refetch", 0)
    expect(result.status).toBe("error")
    expect(result.error).not.toBeNull()
  })

  test("must_refetch even with cached items already on-device: still error, not ready", () => {
    // A stale local cache existing does not mean the scope is actually
    // caught up — must_refetch means the session gave up trying.
    const result = resolveScopeEntitiesStatusAndError("must_refetch", 3)
    expect(result.status).toBe("error")
  })

  test("live phase with zero items: ready (a confirmed-empty scope, not stuck)", () => {
    expect(resolveScopeEntitiesStatusAndError("live", 0)).toEqual({ error: null, status: "ready" })
  })

  test("bootstrapping/catching_up with items already cached: ready (local-first read)", () => {
    expect(resolveScopeEntitiesStatusAndError("bootstrapping", 2)).toEqual({
      error: null,
      status: "ready",
    })
    expect(resolveScopeEntitiesStatusAndError("catching_up", 2)).toEqual({
      error: null,
      status: "ready",
    })
  })

  test("bootstrapping/idle with zero items: loading", () => {
    expect(resolveScopeEntitiesStatusAndError("idle", 0)).toEqual({ error: null, status: "loading" })
    expect(resolveScopeEntitiesStatusAndError("bootstrapping", 0)).toEqual({
      error: null,
      status: "loading",
    })
  })
})
