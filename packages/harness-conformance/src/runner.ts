/**
 * The enum-driven harness conformance suite runner (MH-1, issue #8582).
 *
 * `runHarnessConformance(fixture)` emits a green bun-test block only when the
 * harness proves all five capabilities against the REAL shared contracts:
 * `khala.chat_turn_event.v1` (decoded with the schema's own decoder), the
 * `MarginalCostClass` vocabulary, and the mandatory account-capacity failure
 * classes. Pending kinds instead emit `test.todo` via `todoHarnessConformance`
 * so the redness is visible without failing the normal sweep.
 */
import { describe, expect, test } from "bun:test"
import { decodeKhalaChatTurnEventV1 } from "@openagentsinc/agent-runtime-schema"
import type { KhalaChatTurnEventKind } from "@openagentsinc/agent-runtime-schema"
import { marginalCostClasses } from "@openagentsinc/khala-fleet-intents"
import {
  requiredFailureClasses,
  type HarnessConformanceFixture,
  type HarnessUsageSnapshot,
} from "./contract.ts"

type EventType = KhalaChatTurnEventKind

const decodeAll = (events: ReadonlyArray<unknown>): ReadonlyArray<{ type: EventType }> =>
  events.map((event) => decodeKhalaChatTurnEventV1(event) as { type: EventType })

const types = (events: ReadonlyArray<{ type: EventType }>): ReadonlyArray<EventType> =>
  events.map((event) => event.type)

const assertMeteringHonest = (sample: HarnessUsageSnapshot): void => {
  expect(marginalCostClasses).toContain(sample.marginalCostClass)
  expect(sample.wallClockMs).toBeGreaterThanOrEqual(0)

  const tokenFields = [
    sample.inputTokens,
    sample.outputTokens,
    sample.totalTokens,
    sample.reasoningTokens,
  ]
  const anyTokenPresent = tokenFields.some((value) => value !== undefined)

  if (sample.metering === "not_measured") {
    // Honesty: a not_measured sample must NEVER carry synthesized tokens.
    expect(anyTokenPresent).toBe(false)
    expect(sample.marginalCostClass).not.toBe("api_metered")
    return
  }

  // exact: at least one real token field, and a consistent total when derivable.
  expect(anyTokenPresent).toBe(true)
  if (
    sample.inputTokens !== undefined &&
    sample.outputTokens !== undefined &&
    sample.totalTokens !== undefined
  ) {
    expect(sample.totalTokens).toBeGreaterThanOrEqual(
      sample.inputTokens + sample.outputTokens,
    )
  }
  for (const value of tokenFields) {
    if (value !== undefined) {
      expect(value).toBeGreaterThanOrEqual(0)
    }
  }
}

export const runHarnessConformance = (fixture: HarnessConformanceFixture): void => {
  describe(`harness conformance [${fixture.harnessKind}]`, () => {
    describe("(a) chat runtime -> khala.chat_turn_event.v1", () => {
      test("startThread + startTurn maps to a decodable thread_ready -> message lifecycle", () => {
        const events = fixture.chatRuntime.startThreadTurn()
        const decoded = decodeAll(events)
        const seen = types(decoded)
        expect(seen[0]).toBe("thread_ready")
        expect(seen).toContain("message_start")
        expect(seen).toContain("message_delta")
        expect(seen).toContain("message_done")
        // message_done must not precede message_start.
        expect(seen.indexOf("message_start")).toBeLessThan(seen.indexOf("message_done"))
      })

      test("interrupt maps to a decodable turn (assistant partial preserved)", () => {
        const events = fixture.chatRuntime.interruptTurn()
        const decoded = decodeAll(events)
        expect(decoded.length).toBeGreaterThan(0)
        // An interrupted turn still closes its message rather than dangling.
        expect(types(decoded)).toContain("message_done")
      })

      test("resume maps to a decodable fresh thread_ready", () => {
        const events = fixture.chatRuntime.resumeThread()
        const decoded = decodeAll(events)
        expect(types(decoded)[0]).toBe("thread_ready")
      })
    })

    describe("(b) worker executor: claim -> pinned worktree -> closeout with verify", () => {
      const { claim, closeout } = fixture.workerExecutor

      test("claim pins an exact repo/commit/branch worktree + verify command", () => {
        expect(claim.claimRef.length).toBeGreaterThan(0)
        expect(claim.workUnitRef.length).toBeGreaterThan(0)
        expect(claim.runRef.length).toBeGreaterThan(0)
        expect(claim.repo.length).toBeGreaterThan(0)
        expect(claim.commit.length).toBeGreaterThan(0)
        expect(claim.branch.length).toBeGreaterThan(0)
        expect(claim.verifyCommand.length).toBeGreaterThan(0)
        expect(claim.cwd.length).toBeGreaterThan(0)
      })

      test("closeout is bound to the claim, passed verify, and honors own-capacity no-spend settlement", () => {
        expect(closeout.claimRef).toBe(claim.claimRef)
        expect(closeout.ok).toBe(true)
        expect(closeout.verifyPassed).toBe(true)
        // Own-capacity coding delegation is no-spend: never payout-claimable.
        expect(closeout.paymentMode).toBe("no-spend")
        expect(closeout.settlementState).toBe("not_applicable")
        expect(closeout.payoutClaimAllowed).toBe(false)
      })
    })

    describe("(c) capacity/readiness probe", () => {
      test("probe reports typed readiness + non-negative capacity refs", () => {
        const readiness = fixture.readinessProbe()
        expect(typeof readiness.ready).toBe("boolean")
        expect(readiness.harness.length).toBeGreaterThan(0)
        for (const ref of [
          readiness.capacityAvailable,
          readiness.capacityReady,
          readiness.busy,
          readiness.queued,
        ]) {
          expect(Number.isFinite(ref)).toBe(true)
          expect(ref).toBeGreaterThanOrEqual(0)
        }
        // A ready harness advertises at least one available slot.
        if (readiness.ready) {
          expect(readiness.capacityAvailable).toBeGreaterThan(0)
        }
      })
    })

    describe("(d) metering honesty", () => {
      test("provides both an exact and a not_measured sample", () => {
        const labels = new Set(fixture.meteringSamples.map((sample) => sample.metering))
        expect(labels.has("exact")).toBe(true)
        expect(labels.has("not_measured")).toBe(true)
      })

      test("every sample is honest (exact carries real tokens; not_measured invents none)", () => {
        expect(fixture.meteringSamples.length).toBeGreaterThan(0)
        for (const sample of fixture.meteringSamples) {
          assertMeteringHonest(sample)
        }
      })
    })

    describe("(e) typed failure classes", () => {
      test("provides the three mandatory account-capacity failure classes", () => {
        for (const required of requiredFailureClasses) {
          const make = fixture.typedFailures[required]
          expect(make, `missing typed failure fixture: ${required}`).toBeDefined()
          const sample = make!()
          // The typed class must be specific, not a generic execution error.
          expect(sample.failureClass).toBe(required)
          expect(sample.errorDigestRef.length).toBeGreaterThan(0)
        }
      })
    })
  })
}

/**
 * Emit the shape of the suite for a harness kind that has NOT yet proven its
 * fixtures. Every capability is a `test.todo`, so the redness is visible in the
 * test output without failing the sweep — the effect-native "red until proven"
 * posture. When the owning lane lands real fixtures, it flips the registry
 * entry to `proven` and these todos become the real green suite above.
 */
export const todoHarnessConformance = (
  harnessKind: string,
  reasonRef: string,
  ownerLane: string,
): void => {
  describe(`harness conformance [${harnessKind}] — PENDING (${ownerLane})`, () => {
    test(`red by design: ${reasonRef}`, () => {
      // This asserts the pending state is a known, tracked gap — not silence.
      expect(reasonRef.length).toBeGreaterThan(0)
      expect(ownerLane.length).toBeGreaterThan(0)
    })
    const pending = () => {}
    test.todo("(a) chat runtime -> khala.chat_turn_event.v1", pending)
    test.todo("(b) worker executor: claim -> pinned worktree -> closeout with verify", pending)
    test.todo("(c) capacity/readiness probe", pending)
    test.todo("(d) metering honesty (exact | not_measured; never synthesized)", pending)
    test.todo(
      "(e) typed failure classes (account_exhausted, account_rate_limited, account_quota_exhausted)",
      pending,
    )
  })
}
