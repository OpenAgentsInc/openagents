import { describe, expect, test } from "bun:test"

import { createIntentQueue, decodeSubmittedWorkIntent } from "../node/intent-intake.js"
import { createCoordinatorRuntime } from "./coordinator-runtime.js"

const nowIso = "2026-07-05T00:00:00.000Z"

// Regression coverage for the #8282 Promise.all landmine audit
// (docs/2026-07-05-promise-all-cron-landmine-audit.md, Lane 2 #1): the
// coordinator's `reconcile()` used to call `Promise.all(refs.map(deps.sessionState))`
// with no per-ref isolation. Because `tick()` reconciles EVERY queued
// "fanning_out" intent in a `for...of` loop each cycle, one flaky
// `sessionState` read anywhere would throw out of `reconcile()`, abort the
// `for...of` loop in `tick()`, and silently skip reconciliation for every
// OTHER unrelated intent still queued in that same tick.
describe("coordinator runtime session-state reconcile isolation", () => {
  test("a flaky sessionState read for one intent's session does not abort reconcile for a sibling intent in the same tick", async () => {
    const intentQueue = createIntentQueue()

    // Intent A fans out into two sessions (two checklist lines) so this also
    // proves per-ref isolation *within* a single reconcile() call: session
    // "s2" always throws, but session "s1" ("completed") must still be read.
    const intentA = decodeSubmittedWorkIntent({
      intentId: "intent-a",
      title: "Intent A",
      body: "- part one\n- part two",
      submittedByClientRef: "test-client",
      createdAt: nowIso,
    })
    // Intent B is a single-session intent whose reconcile must complete in
    // the SAME tick even though intent A's reconcile hit a throw.
    const intentB = decodeSubmittedWorkIntent({
      intentId: "intent-b",
      title: "Intent B",
      body: "single unit of work, no checklist",
      submittedByClientRef: "test-client",
      createdAt: nowIso,
    })

    intentQueue.enqueue(intentA)
    intentQueue.enqueue(intentB)

    let spawnCount = 0
    const sessionStateCalls: string[] = []
    let s2ShouldThrow = true

    const logs: string[] = []

    const runtime = createCoordinatorRuntime({
      intentQueue,
      spawnSession: async () => {
        spawnCount += 1
        return { sessionRef: `s${spawnCount}` }
      },
      sessionState: async (ref) => {
        sessionStateCalls.push(ref)
        if (ref === "s2" && s2ShouldThrow) {
          throw new Error("flaky session-state backend read")
        }
        // s1 (intent A's other session) and s3 (intent B's only session)
        // both report completed as soon as they're observed.
        return "completed"
      },
      createWorktree: async () => "/tmp/fake-worktree",
      log: (message) => logs.push(message),
    })

    // Tick 1: dispatches both intents (received -> planning -> fanning_out),
    // spawning s1+s2 for intent A and s3 for intent B. Neither intent has
    // sessions to reconcile yet this same tick (reconcile only runs for
    // intents that were ALREADY "fanning_out" before this tick started).
    await runtime.tick()
    expect(intentQueue.get("intent-a")?.status).toBe("fanning_out")
    expect(intentQueue.get("intent-b")?.status).toBe("fanning_out")

    // Tick 2: both intents are now "fanning_out" and get reconciled in the
    // same for...of loop. Intent A's reconcile hits the throwing "s2" read;
    // intent B's reconcile must still run and complete in this same tick.
    await runtime.tick()

    expect(sessionStateCalls).toContain("s1")
    expect(sessionStateCalls).toContain("s2")
    expect(sessionStateCalls).toContain("s3")

    // Intent A stays in fanning_out (deferred, self-healing next tick) —
    // NOT crashed, NOT marked failed just because one session read errored.
    expect(intentQueue.get("intent-a")?.status).toBe("fanning_out")
    // Intent B's reconcile ran in the SAME tick and completed successfully —
    // proof the sibling intent's failure never aborted the tick loop.
    expect(intentQueue.get("intent-b")?.status).toBe("shipped")
    expect(logs.some((line) => line.includes("session state read failed for s2"))).toBe(true)

    // Tick 3: once the flaky session-state backend recovers, intent A's
    // deferred sessions are now both observable and it reconciles normally.
    s2ShouldThrow = false
    await runtime.tick()
    expect(intentQueue.get("intent-a")?.status).toBe("shipped")
  })
})
