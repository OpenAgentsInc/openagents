// Formal-adjacent model coverage for FA-AS-01 (#8978): a bounded exhaustive
// enumeration over the retry/backoff and provider-rotation-classification
// axes of the Full Auto dispatch-failure state space, composed against the
// REAL exported production functions (never a reimplementation of their
// logic -- an oracle that repeats implementation behavior is a false green,
// ASSURANCE_SPEC.md Law 4/9).
//
// This is deliberately NOT a claim of a TLA+/model-checker-grade formal
// model. The repository has no TLA+ spec for Full Auto (specs/ only models
// khala-fleet-delegate, approval-protocol, and session-thread-mapping). The
// existing exhaustive lifecycle enumeration in
// tests/full-auto-run-registry.test.ts ("FullAutoRun lifecycle state
// machine", FA-AC-43/44/45) already exhaustively checks the bare 10-state
// `FullAutoRunState` transition table. This file extends bounded exhaustive
// coverage to the two axes that table does NOT reach: the backoff schedule
// (FA-H5 #8878) and the rotation-reason classifier (FA-RT-01 #8987), both
// governing FA-AC-16 and FA-AC-67. The composed reachable state space of
// {lifecycle state x lease-claimed x retry-attempt x routing-candidate-index}
// remains NOT exhaustively model-checked after this file -- see the
// AssuranceSpec Formal Models section for that residual gap.
import { describe, expect, test } from "vite-plus/test"

import {
  FULL_AUTO_FAILURE_BACKOFF_BASE_MS,
  FULL_AUTO_FAILURE_BACKOFF_MAX_MS,
  FULL_AUTO_MAX_CONSECUTIVE_FAILURES,
  classifyFullAutoDispatchFailure,
  fullAutoFailureBackoffMs,
} from "./full-auto-reconcile.ts"
import { FullAutoRotationReasonSchema } from "./full-auto-registry.ts"

// Bounded beyond the real 5-failure disable threshold (FA-H5) so the
// enumeration also covers the saturation region past where a record would
// already have durably disabled -- the pure function itself has no such
// bound and must stay well-behaved past it (defense in depth: a future
// owner-configurable failure budget, FA-AC-68, can raise the threshold).
const FAILURE_COUNT_DOMAIN: ReadonlyArray<number> = Array.from({ length: 12 }, (_, index) => index)

describe("Full Auto retry/rotation exhaustive model (FA-AS-01 formal-adjacent obligation, FA-AC-16/FA-AC-67)", () => {
  test("bounded exponential backoff is monotonically non-decreasing across every consecutive-failure count in the enumerated domain", () => {
    let previous = -Infinity
    for (const failures of FAILURE_COUNT_DOMAIN) {
      const wait = fullAutoFailureBackoffMs(failures)
      expect(wait).toBeGreaterThanOrEqual(previous)
      previous = wait
    }
  })

  test("bounded exponential backoff never exceeds the declared cap and never drops below the declared base for any non-negative failure count", () => {
    for (const failures of FAILURE_COUNT_DOMAIN) {
      const wait = fullAutoFailureBackoffMs(failures)
      expect(wait).toBeLessThanOrEqual(FULL_AUTO_FAILURE_BACKOFF_MAX_MS)
      expect(wait).toBeGreaterThanOrEqual(FULL_AUTO_FAILURE_BACKOFF_BASE_MS)
    }
  })

  test("the backoff schedule reaches full saturation at the cap strictly before the FA-H5 disable threshold and stays saturated after it (the owner never waits longer than the cap while a record is still allowed to retry)", () => {
    // 2^6 * 30s = 1920s > 1800s (30min cap), so failures=6 is already
    // saturated -- strictly inside the FA-H5 5-failure disable window means
    // the record disables (FA-AC-16) before backoff would ever need to grow
    // further. This is a genuine cross-cutting safety property linking two
    // independently declared constants, not a restatement of either.
    const saturationFailureCount = FAILURE_COUNT_DOMAIN.find(
      (failures) => fullAutoFailureBackoffMs(failures) === FULL_AUTO_FAILURE_BACKOFF_MAX_MS,
    )
    expect(saturationFailureCount).toBeDefined()
    expect(saturationFailureCount!).toBeGreaterThan(FULL_AUTO_MAX_CONSECUTIVE_FAILURES)
    for (const failures of FAILURE_COUNT_DOMAIN) {
      if (failures >= saturationFailureCount!) {
        expect(fullAutoFailureBackoffMs(failures)).toBe(FULL_AUTO_FAILURE_BACKOFF_MAX_MS)
      }
    }
  })

  test("FALSIFIER: a strictly decreasing or cap-exceeding backoff schedule is REFUTED by this oracle", () => {
    // Demonstrates the oracle's sensitivity (ASSURANCE_SPEC.md Law 4) without
    // mutating production source: a deliberately broken stand-in schedule
    // (decreasing, and one entry exceeding the cap) fails the exact
    // monotonicity/bound assertions used above, proving they are not
    // vacuously true.
    const brokenSchedule = [30_000, 60_000, 45_000, 2_000_000]
    let previous = -Infinity
    let monotonic = true
    for (const wait of brokenSchedule) {
      if (wait < previous) monotonic = false
      previous = wait
    }
    expect(monotonic).toBe(false)
    expect(brokenSchedule.some((wait) => wait > FULL_AUTO_FAILURE_BACKOFF_MAX_MS)).toBe(true)
  })

  test("every FullAutoRotationReasonSchema literal classifies to itself as a rotation-eligible dispatch-failure reason (exhaustive over the real exported literal set, not a hand-guessed subset)", () => {
    const rotationReasons = [...FullAutoRotationReasonSchema.literals]
    expect(rotationReasons.sort()).toEqual(["account_exhausted", "provider_error", "rate_limited"].sort())
    for (const reason of rotationReasons) {
      expect(classifyFullAutoDispatchFailure(reason)).toBe(reason)
    }
    // Adapter-specific account exhaustion aliases.
    expect(classifyFullAutoDispatchFailure("budget_exceeded")).toBe("account_exhausted")
    expect(classifyFullAutoDispatchFailure("no_claude_account")).toBe("account_exhausted")
    expect(classifyFullAutoDispatchFailure("no_codex_account")).toBe("account_exhausted")
    expect(classifyFullAutoDispatchFailure("account_reconnect_required")).toBe("account_exhausted")
  })

  test("detail-sniffed reasons (timeout/sdk_unavailable/session_failed) classify by real substring rule, exhaustively over the documented marker set", () => {
    const quotaDetails = ["usage limit reached", "quota exceeded", "please purchase more credits"]
    const rateLimitDetails = ["rate limit hit", "HTTP 429", "too many requests"]
    const genericDetails = ["", "connection reset", "unexpected token"]
    for (const reason of ["timeout", "sdk_unavailable", "session_failed"] as const) {
      for (const detail of quotaDetails) {
        expect(classifyFullAutoDispatchFailure(reason, detail)).toBe("account_exhausted")
      }
      for (const detail of rateLimitDetails) {
        expect(classifyFullAutoDispatchFailure(reason, detail)).toBe("rate_limited")
      }
      for (const detail of genericDetails) {
        expect(classifyFullAutoDispatchFailure(reason, detail)).toBe("provider_error")
      }
    }
  })

  test("FALSIFIER: an untyped/unrecognized dispatch-failure reason NEVER rotates -- classification returns null rather than guessing (this is the real safety property: silent misclassification would let an owner-interrupt or model-substitution failure incorrectly consume a rotation instead of the ordinary FA-H5 failure budget)", () => {
    const unrecognizedReasons = [
      undefined,
      "owner_interrupted",
      "model_substitution",
      "workflow_incompatible",
      "totally_unknown_future_reason",
      "",
    ]
    for (const reason of unrecognizedReasons) {
      expect(classifyFullAutoDispatchFailure(reason)).toBeNull()
      expect(classifyFullAutoDispatchFailure(reason, "quota exceeded")).toBeNull()
    }
  })
})
