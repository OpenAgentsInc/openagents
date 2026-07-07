import { describe, expect, test } from "bun:test"
import { SyncScope } from "@openagentsinc/khala-sync"
import type { KhalaSyncSession } from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

import {
  acquireScopeSubscription,
  releaseScopeSubscription,
} from "../src/sync/scope-subscription-refcount"

// Oracle for the confirmed on-device regression (2026-07-06, build 15): two
// hooks observing the SAME scope, one unmounting, must NOT tear the shared
// scope down while the other is still mounted. The real `session.unsubscribe`
// resets a live scope to `idle`; refcounting defers it to the last release.

const fakeSession = () => {
  const subscribed: Array<string> = []
  const unsubscribed: Array<string> = []
  const session = {
    subscribe: (scope: SyncScope) =>
      Effect.sync(() => {
        subscribed.push(String(scope))
      }),
    unsubscribe: (scope: SyncScope) =>
      Effect.sync(() => {
        unsubscribed.push(String(scope))
      }),
  } as unknown as KhalaSyncSession
  return { session, subscribed, unsubscribed }
}

const SCOPE = SyncScope.make("scope.user.github:14167547")

describe("scope-subscription-refcount", () => {
  test("subscribes only once for the first of several observers on one scope", async () => {
    const { session, subscribed } = fakeSession()

    await acquireScopeSubscription(session, SCOPE)
    await acquireScopeSubscription(session, SCOPE)
    await acquireScopeSubscription(session, SCOPE)

    expect(subscribed).toEqual(["scope.user.github:14167547"])
  })

  test("does NOT unsubscribe while other observers remain (the shared-teardown bug)", async () => {
    const { session, unsubscribed } = fakeSession()

    await acquireScopeSubscription(session, SCOPE) // thread-list chat_thread
    await acquireScopeSubscription(session, SCOPE) // credits chip credit_balance

    // The credits chip unmounts first — must NOT reset the shared scope.
    releaseScopeSubscription(session, SCOPE)
    expect(unsubscribed).toEqual([])

    // The thread list unmounts last — now the real teardown may fire.
    releaseScopeSubscription(session, SCOPE)
    expect(unsubscribed).toEqual(["scope.user.github:14167547"])
  })

  test("a fresh acquire after full release subscribes again", async () => {
    const { session, subscribed, unsubscribed } = fakeSession()

    await acquireScopeSubscription(session, SCOPE)
    releaseScopeSubscription(session, SCOPE)
    await acquireScopeSubscription(session, SCOPE)

    expect(subscribed).toEqual([
      "scope.user.github:14167547",
      "scope.user.github:14167547",
    ])
    expect(unsubscribed).toEqual(["scope.user.github:14167547"])
  })

  test("release with no outstanding acquire is a clean no-op (no teardown)", () => {
    const { session, unsubscribed } = fakeSession()
    releaseScopeSubscription(session, SCOPE)
    expect(unsubscribed).toEqual([])
  })

  test("counts are isolated per session instance", async () => {
    const a = fakeSession()
    const b = fakeSession()

    await acquireScopeSubscription(a.session, SCOPE)
    await acquireScopeSubscription(b.session, SCOPE)

    // Releasing session A's only observer tears down A but never touches B.
    releaseScopeSubscription(a.session, SCOPE)
    expect(a.unsubscribed).toEqual(["scope.user.github:14167547"])
    expect(b.unsubscribed).toEqual([])
  })
})
