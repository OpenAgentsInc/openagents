import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  captureFullAutoAcceptanceIdentity,
  fullAutoAcceptanceTest,
  type FullAutoAcceptanceIdentity,
} from "./full-auto-acceptance.ts"
import {
  executeFullAutoAcceptanceTest,
  makeFixtureLaneExecutor,
  openFullAutoAcceptanceHarness,
  type AcceptanceLaneExecutor,
} from "./full-auto-acceptance-driver.ts"

/**
 * FA-QA-01 (#8976): headless fixture-mode executions of the six acceptance
 * shapes against the REAL seams (thread store, provider-lane switch
 * projection, handoff envelope/registry from #8975, full-auto reconcile
 * lease/cap machinery, run registry, report store, analyzer). Only the
 * PROVIDER is a fixture. Per the issue, these results support diagnosis and
 * prove the machinery -- they cannot replace the visible owner-profile
 * real-provider passes, which is why every identity below is
 * `profileClass: "fixture"`.
 *
 * Envelope internals (omission entries, provider-private-never-transferred,
 * bounded-projection redaction) are already pinned by
 * full-auto-provider-handoff.test.ts and are not re-asserted here.
 */

const fixtureIdentity = (): FullAutoAcceptanceIdentity => captureFullAutoAcceptanceIdentity({
  revision: "workspace-fixture",
  build: "headless-fixture",
  packagingMode: "dev",
  profileClass: "fixture",
  providerVersions: [
    { laneRef: "codex-local", runtime: "fixture-executor", version: "1", authReadiness: "unknown" },
    { laneRef: "claude-local", runtime: "fixture-executor", version: "1", authReadiness: "unknown" },
  ],
  telemetry: "disabled",
})

const withHarness = async (
  prefix: string,
  run: (root: string) => Promise<void>,
): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), prefix))
  try {
    await run(root)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

describe("TEST 01 · Codex → Claude · context (headless fixture mode)", () => {
  test("passes end-to-end through the real switch seam: one thread, ORBIT-17 retained, one durable transition, title prefixed only after evaluation", async () => {
    await withHarness("oa-fa-qa01-t01-", async root => {
      const harness = await openFullAutoAcceptanceHarness(root)
      const execution = await executeFullAutoAcceptanceTest({
        definition: fullAutoAcceptanceTest("test-01"),
        harness,
        executor: makeFixtureLaneExecutor(),
        identity: fixtureIdentity(),
      })

      expect(execution.verdict.disposition).toBe("PASS")
      expect(execution.verdict.reasons).toEqual([])
      expect(execution.evidence.markerEstablishedInSource).toBe(true)
      expect(execution.evidence.markerStatedByTarget).toBe(true)
      expect(execution.evidence.targetMarkerStatement).toContain("ORBIT-17")
      expect(execution.evidence.stepTwoUsedPriorResult).toBe(true)
      expect(execution.evidence.threadRefsTouched).toEqual([execution.threadRef])

      // Exactly one durable provider-transition receipt, read back from the
      // real handoff registry, in the Codex -> Claude direction.
      expect(execution.evidence.transitions).toHaveLength(1)
      expect(execution.evidence.transitions[0]!.from).toBe("codex-local")
      expect(execution.evidence.transitions[0]!.to).toBe("claude-local")

      // The sidebar row carries the disposition prefix (applied strictly
      // after evaluation) and the durable thread agrees.
      expect(execution.finalTitle).toBe("PASS · TEST 01 · Codex → Claude · context")
      const thread = harness.store.open(execution.threadRef!)
      expect(thread?.title).toBe("PASS · TEST 01 · Codex → Claude · context")
      // The visible in-thread transition event exists alongside the receipt.
      expect(thread?.notes.some(note =>
        note.role === "system" && note.text.includes("codex-local → claude-local"),
      )).toBe(true)
      // The lane selection durably moved to the target lane.
      expect(harness.laneRegistry.selection(execution.threadRef!)).toBe("claude-local")
    })
  })

  test("the marker-retention rule genuinely bites: a broken handoff (dropped host-owned history) FAILS, and the row is renamed FAIL, not deleted", async () => {
    await withHarness("oa-fa-qa01-t01-broken-", async root => {
      const harness = await openFullAutoAcceptanceHarness(root)
      const execution = await executeFullAutoAcceptanceTest({
        definition: fullAutoAcceptanceTest("test-01"),
        harness,
        executor: makeFixtureLaneExecutor(),
        identity: fixtureIdentity(),
        sabotage: { dropHandoffHistory: true },
      })

      expect(execution.verdict.disposition).toBe("FAIL")
      expect(execution.evidence.markerEstablishedInSource).toBe(true)
      expect(execution.evidence.markerStatedByTarget).toBe(false)
      expect(execution.evidence.targetMarkerStatement).toContain("I do not have the marker")
      expect(
        execution.verdict.reasons.some(reason => reason.startsWith("marker_retained:")),
      ).toBe(true)
      // Failure handling per the issue: renamed, never deleted.
      expect(execution.finalTitle).toBe("FAIL · TEST 01 · Codex → Claude · context")
      expect(harness.store.open(execution.threadRef!)).not.toBeNull()
    })
  })

  test("a provider outage is BLOCKED, never PASS and never product FAIL", async () => {
    await withHarness("oa-fa-qa01-t01-outage-", async root => {
      const harness = await openFullAutoAcceptanceHarness(root)
      const fixture = makeFixtureLaneExecutor()
      const outageOnTarget: AcceptanceLaneExecutor = async input =>
        input.laneRef === "claude-local"
          ? { ok: false, reason: "provider runtime unavailable" }
          : fixture(input)
      const execution = await executeFullAutoAcceptanceTest({
        definition: fullAutoAcceptanceTest("test-01"),
        harness,
        executor: outageOnTarget,
        identity: fixtureIdentity(),
      })
      expect(execution.verdict.disposition).toBe("BLOCKED")
      expect(execution.finalTitle).toBe("BLOCKED · TEST 01 · Codex → Claude · context")
      expect(execution.evidence.blockedReason).toContain("provider runtime unavailable")
    })
  })
})

describe("TEST 02 · Claude → Codex · context (headless fixture mode)", () => {
  test("passes in the mirrored direction with LANTERN-42", async () => {
    await withHarness("oa-fa-qa01-t02-", async root => {
      const harness = await openFullAutoAcceptanceHarness(root)
      const execution = await executeFullAutoAcceptanceTest({
        definition: fullAutoAcceptanceTest("test-02"),
        harness,
        executor: makeFixtureLaneExecutor(),
        identity: fixtureIdentity(),
      })
      expect(execution.verdict.disposition).toBe("PASS")
      expect(execution.evidence.targetMarkerStatement).toContain("LANTERN-42")
      expect(execution.evidence.transitions).toHaveLength(1)
      expect(execution.evidence.transitions[0]!.from).toBe("claude-local")
      expect(execution.evidence.transitions[0]!.to).toBe("codex-local")
      expect(execution.finalTitle).toBe("PASS · TEST 02 · Claude → Codex · context")
    })
  })

  test("the mirrored direction fails identically under a broken handoff", async () => {
    await withHarness("oa-fa-qa01-t02-broken-", async root => {
      const harness = await openFullAutoAcceptanceHarness(root)
      const execution = await executeFullAutoAcceptanceTest({
        definition: fullAutoAcceptanceTest("test-02"),
        harness,
        executor: makeFixtureLaneExecutor(),
        identity: fixtureIdentity(),
        sabotage: { dropHandoffHistory: true },
      })
      expect(execution.verdict.disposition).toBe("FAIL")
      expect(
        execution.verdict.reasons.some(reason => reason.startsWith("marker_retained:")),
      ).toBe(true)
    })
  })
})

describe("TEST 03 · objective retention (headless fixture mode)", () => {
  test("the run's objective and acceptance rule ride the durable priority channel while the transcript projection truncates, and the truncation is surfaced + confirmed", async () => {
    await withHarness("oa-fa-qa01-t03-", async root => {
      const harness = await openFullAutoAcceptanceHarness(root)
      const execution = await executeFullAutoAcceptanceTest({
        definition: fullAutoAcceptanceTest("test-03"),
        harness,
        executor: makeFixtureLaneExecutor(),
        identity: fixtureIdentity(),
      })
      expect(execution.evidence.contextTruncated).toBe(true)
      expect(execution.evidence.objectiveDeliveredToTarget).toBe(true)
      expect(execution.evidence.acceptanceRuleDeliveredToTarget).toBe(true)
      expect(execution.evidence.truncationAcknowledged).toBe(true)
      expect(execution.evidence.truncationConfirmationRecorded).toBe(true)
      expect(execution.evidence.transitions).toHaveLength(1)
      expect(execution.evidence.transitions[0]!.disposition).toBe("truncated_with_confirmation")
      expect(execution.verdict.disposition).toBe("PASS")
      expect(execution.finalTitle).toBe("PASS · TEST 03 · Codex → Claude · objective retention")
    })
  })
})

describe("TEST 04 · Full Auto · Codex · 3 turns (headless fixture mode)", () => {
  test("three autonomous turns with no manual message, a synced report, an analyzer result, and an explicit final reason", async () => {
    await withHarness("oa-fa-qa01-t04-", async root => {
      const harness = await openFullAutoAcceptanceHarness(root)
      const execution = await executeFullAutoAcceptanceTest({
        definition: fullAutoAcceptanceTest("test-04"),
        harness,
        executor: makeFixtureLaneExecutor(),
        identity: fixtureIdentity(),
      })
      expect(execution.evidence.autonomousTurnsCompleted).toBe(3)
      expect(execution.evidence.manualMessagesBetweenTurns).toBe(0)
      expect(execution.evidence.continuationDispatchCounts).toEqual([1, 1, 1])
      expect(execution.evidence.duplicateDispatchCount).toBe(0)
      expect(execution.evidence.finalStateReason).toContain("done condition met")
      expect(execution.report).not.toBeNull()
      expect(execution.report!.turns.length).toBe(3)
      expect(execution.report!.state).toBe("completed")
      expect(execution.analysis).not.toBeNull()
      expect(execution.analysis!.runRef).toBe(execution.report!.runRef)
      expect(execution.verdict.disposition).toBe("PASS")
      expect(execution.finalTitle).toBe("PASS · TEST 04 · Full Auto · Codex · 3 turns")
    })
  })
})

describe("TEST 05 · Full Auto · Claude · restart (headless fixture mode)", () => {
  test("the same run resumes across a re-open of every durable file, with no duplicate dispatch and one report spanning the restart", async () => {
    await withHarness("oa-fa-qa01-t05-", async root => {
      const harness = await openFullAutoAcceptanceHarness(root)
      const execution = await executeFullAutoAcceptanceTest({
        definition: fullAutoAcceptanceTest("test-05"),
        harness,
        executor: makeFixtureLaneExecutor(),
        identity: fixtureIdentity(),
        reopenHarness: () => openFullAutoAcceptanceHarness(root),
      })
      expect(execution.evidence.restartBoundariesObserved).toBe(1)
      expect(execution.evidence.initialRunRef).not.toBeNull()
      expect(execution.evidence.resumedRunRef).toBe(execution.evidence.initialRunRef)
      expect(execution.evidence.runFieldsContinuous).toBe(true)
      expect(execution.evidence.autonomousTurnsCompleted).toBe(3)
      expect(execution.evidence.duplicateDispatchCount).toBe(0)
      // Cycle 2 raced a startup pass against a completion pass -- the lease
      // still permitted exactly one dispatch (dispatchInvocations counts
      // reconcile passes that REACHED dispatch, and only one did).
      expect(execution.evidence.continuationDispatchCounts).toEqual([1, 1, 1])
      expect(execution.evidence.reportSpansRestart).toBe(true)
      expect(execution.report).not.toBeNull()
      expect(execution.analysis).not.toBeNull()
      expect(execution.verdict.disposition).toBe("PASS")
      expect(execution.finalTitle).toBe("PASS · TEST 05 · Full Auto · Claude · restart")
    })
  })

  test("without the relaunch seam the slice is honestly BLOCKED, not silently green", async () => {
    await withHarness("oa-fa-qa01-t05-noreopen-", async root => {
      const harness = await openFullAutoAcceptanceHarness(root)
      const execution = await executeFullAutoAcceptanceTest({
        definition: fullAutoAcceptanceTest("test-05"),
        harness,
        executor: makeFixtureLaneExecutor(),
        identity: fixtureIdentity(),
      })
      expect(execution.verdict.disposition).toBe("BLOCKED")
      expect(execution.evidence.blockedReason).toContain("reopenHarness")
    })
  })
})

describe("TEST 06 · Full Auto · thread pressure (headless fixture mode)", () => {
  test("six other chats against the five-slot bounded cache: the run thread stays addressable and every continuation starts exactly once", async () => {
    await withHarness("oa-fa-qa01-t06-", async root => {
      const harness = await openFullAutoAcceptanceHarness(root)
      const execution = await executeFullAutoAcceptanceTest({
        definition: fullAutoAcceptanceTest("test-06"),
        harness,
        executor: makeFixtureLaneExecutor(),
        identity: fixtureIdentity(),
      })
      expect(execution.evidence.otherChatsOpened).toBe(6)
      expect(execution.evidence.threadAddressableUnderPressure).toBe(true)
      expect(execution.evidence.continuationDispatchCounts).toEqual([1, 1, 1])
      expect(execution.evidence.duplicateDispatchCount).toBe(0)
      expect(execution.evidence.autonomousTurnsCompleted).toBe(3)
      // Real eviction pressure was exceeded, not simulated: the bounded
      // cache holds 5 of the 7 threads that exist.
      expect(harness.store.list().length).toBe(5)
      expect(execution.report).not.toBeNull()
      expect(execution.analysis).not.toBeNull()
      expect(execution.verdict.disposition).toBe("PASS")
      expect(execution.finalTitle).toBe("PASS · TEST 06 · Full Auto · thread pressure")
    })
  })
})
