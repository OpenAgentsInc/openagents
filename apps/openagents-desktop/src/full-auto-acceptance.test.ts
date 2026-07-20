import { describe, expect, test } from "vite-plus/test"

import { Schema } from "effect"

import {
  EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
  FA_QA_CLAUDE_LANE,
  FA_QA_CODEX_LANE,
  FA_QA_MARKER_LANTERN,
  FA_QA_MARKER_ORBIT,
  FULL_AUTO_ACCEPTANCE_IDENTITY_SCHEMA,
  FULL_AUTO_ACCEPTANCE_TESTS,
  FullAutoAcceptanceIdentitySchema,
  FullAutoAcceptanceTestDefinitionSchema,
  acceptanceTitleDisposition,
  acceptanceTitleWithDisposition,
  captureFullAutoAcceptanceIdentity,
  evaluateFullAutoAcceptance,
  fullAutoAcceptanceDefinitionRevision,
  fullAutoAcceptanceTest,
  stripAcceptanceDisposition,
} from "./full-auto-acceptance.ts"
import {
  PROVIDER_HANDOFF_ENVELOPE_SCHEMA,
  PROVIDER_HANDOFF_TRANSITION_SCHEMA,
} from "./full-auto-provider-handoff.ts"
import { FULL_AUTO_RUN_REGISTRY_SCHEMA } from "./full-auto-run-registry.ts"
import { FULL_AUTO_RUN_REPORT_SCHEMA } from "./full-auto-run-report.ts"

/**
 * FA-QA-01 (#8976): the six-test definition set, the pinned identity record,
 * and the evidence->verdict evaluator. Envelope/bounded-projection internals
 * are already proven by full-auto-provider-handoff.test.ts (#8975) and are
 * deliberately NOT re-asserted here.
 */

describe("FA-QA-01 test definitions", () => {
  test("exactly the six issue-named tests exist, with the exact sidebar titles verbatim", () => {
    expect(FULL_AUTO_ACCEPTANCE_TESTS.map(definition => definition.title)).toEqual([
      "TEST 01 · Codex → Claude · context",
      "TEST 02 · Claude → Codex · context",
      "TEST 03 · Codex → Claude · objective retention",
      "TEST 04 · Full Auto · Codex · 3 turns",
      "TEST 05 · Full Auto · Claude · restart",
      "TEST 06 · Full Auto · thread pressure",
    ])
  })

  test("the issue-pinned markers are byte-exact and bound to the right directions", () => {
    expect(FA_QA_MARKER_ORBIT).toBe("ORBIT-17")
    expect(FA_QA_MARKER_LANTERN).toBe("LANTERN-42")
    const test01 = fullAutoAcceptanceTest("test-01")
    expect(test01.marker).toBe("ORBIT-17")
    expect(test01.sourceLaneRef).toBe(FA_QA_CODEX_LANE)
    expect(test01.targetLaneRef).toBe(FA_QA_CLAUDE_LANE)
    const test02 = fullAutoAcceptanceTest("test-02")
    expect(test02.marker).toBe("LANTERN-42")
    expect(test02.sourceLaneRef).toBe(FA_QA_CLAUDE_LANE)
    expect(test02.targetLaneRef).toBe(FA_QA_CODEX_LANE)
  })

  test("every definition decodes against its own schema and carries at least one typed pass rule", () => {
    for (const definition of FULL_AUTO_ACCEPTANCE_TESTS) {
      const decoded = Schema.decodeUnknownSync(FullAutoAcceptanceTestDefinitionSchema)(definition)
      expect(decoded.passRules.length).toBeGreaterThanOrEqual(1)
    }
    // Marker retention is data, not code: both context tests carry the exact
    // marker inside their pass-rule payloads.
    expect(fullAutoAcceptanceTest("test-01").passRules).toContainEqual({
      rule: "marker_retained",
      marker: "ORBIT-17",
    })
    expect(fullAutoAcceptanceTest("test-02").passRules).toContainEqual({
      rule: "marker_retained",
      marker: "LANTERN-42",
    })
  })

  test("the full-auto tests pin three turns and the pressure test pins the >5-other-chats composition", () => {
    expect(fullAutoAcceptanceTest("test-04").plannedTurns).toBe(3)
    expect(fullAutoAcceptanceTest("test-05").plannedTurns).toBe(3)
    expect(fullAutoAcceptanceTest("test-06").passRules).toContainEqual({
      rule: "thread_addressable_under_pressure",
      minOtherChats: 5,
    })
  })
})

describe("FA-QA-01 pinned test identity", () => {
  const capture = () => captureFullAutoAcceptanceIdentity({
    revision: "f7da63e20a",
    build: "0.1.0-rc.18",
    packagingMode: "dev",
    profileClass: "fixture",
    providerVersions: [
      { laneRef: "codex-local", runtime: "codex-fixture", version: "0.0.0", authReadiness: "unknown" },
    ],
    telemetry: "disabled",
    now: () => new Date("2026-07-17T00:00:00.000Z"),
  })

  test("schema revisions are captured mechanically from the modules' own exported constants", () => {
    const identity = capture()
    expect(identity.schemaRevisions.handoffEnvelope).toBe(PROVIDER_HANDOFF_ENVELOPE_SCHEMA)
    expect(identity.schemaRevisions.handoffTransition).toBe(PROVIDER_HANDOFF_TRANSITION_SCHEMA)
    expect(identity.schemaRevisions.runRegistry).toBe(FULL_AUTO_RUN_REGISTRY_SCHEMA)
    expect(identity.schemaRevisions.runReport).toBe(FULL_AUTO_RUN_REPORT_SCHEMA)
    expect(identity.os).toBe(process.platform)
    expect(identity.arch).toBe(process.arch)
    expect(identity.schema).toBe(FULL_AUTO_ACCEPTANCE_IDENTITY_SCHEMA)
    expect(Schema.decodeUnknownSync(FullAutoAcceptanceIdentitySchema)(identity)).toEqual(identity)
  })

  test("the test-definition revision is a stable sha256 over the canonical definition set", () => {
    const identity = capture()
    expect(identity.testDefinitionRevision).toBe(fullAutoAcceptanceDefinitionRevision())
    expect(identity.testDefinitionRevision).toMatch(/^[0-9a-f]{64}$/)
    // Deterministic: the same definitions always digest identically.
    expect(fullAutoAcceptanceDefinitionRevision()).toBe(fullAutoAcceptanceDefinitionRevision())
  })

  test("profileClass structurally separates fixture machinery proof from owner-armed real runs", () => {
    expect(capture().profileClass).toBe("fixture")
    expect(() =>
      Schema.decodeUnknownSync(FullAutoAcceptanceIdentitySchema)({
        ...capture(),
        profileClass: "anything_else",
      }),
    ).toThrow()
  })
})

describe("FA-QA-01 verdict evaluation", () => {
  const passingContextEvidence = (marker: string) => ({
    ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
    threadRef: "thread-1",
    threadRefsTouched: ["thread-1"],
    markerEstablishedInSource: true,
    markerStatedByTarget: true,
    targetMarkerStatement: `The marker is ${marker}. STEP-TWO-COMPLETE(${marker})`,
    stepTwoUsedPriorResult: true,
    transitions: [{
      handoffRef: "handoff.provider.x.y",
      threadRef: "thread-1",
      from: "codex-local",
      to: "claude-local",
      actor: "owner_ui" as const,
      at: "2026-07-17T00:00:00.000Z",
      reason: "scripted switch",
      disposition: "complete_within_bounds" as const,
      truncated: false,
    }],
  })

  test("PASS requires every rule to hold; reasons are empty exactly then", () => {
    const verdict = evaluateFullAutoAcceptance(
      fullAutoAcceptanceTest("test-01"),
      passingContextEvidence("ORBIT-17"),
    )
    expect(verdict.disposition).toBe("PASS")
    expect(verdict.reasons).toEqual([])
    expect(verdict.ruleResults.every(result => result.holds)).toBe(true)
  })

  test("a target that cannot state the exact marker FAILS marker_retained with a named reason", () => {
    const verdict = evaluateFullAutoAcceptance(fullAutoAcceptanceTest("test-01"), {
      ...passingContextEvidence("ORBIT-17"),
      markerStatedByTarget: false,
      targetMarkerStatement: "I do not have the marker.",
      stepTwoUsedPriorResult: false,
    })
    expect(verdict.disposition).toBe("FAIL")
    expect(verdict.reasons.some(reason => reason.startsWith("marker_retained:"))).toBe(true)
    expect(verdict.reasons.some(reason => reason.startsWith("step_two_uses_prior_result:"))).toBe(true)
  })

  test("a paraphrased marker (right shape, wrong token) is not retention", () => {
    const verdict = evaluateFullAutoAcceptance(fullAutoAcceptanceTest("test-02"), {
      ...passingContextEvidence("LANTERN-42"),
      targetMarkerStatement: "The marker is LANTERN-43. STEP-TWO-COMPLETE(LANTERN-43)",
    })
    expect(verdict.disposition).toBe("FAIL")
  })

  test("zero or two transition events both fail the exactly-one visible-transition rule", () => {
    const base = passingContextEvidence("ORBIT-17")
    expect(
      evaluateFullAutoAcceptance(fullAutoAcceptanceTest("test-01"), { ...base, transitions: [] })
        .disposition,
    ).toBe("FAIL")
    expect(
      evaluateFullAutoAcceptance(fullAutoAcceptanceTest("test-01"), {
        ...base,
        transitions: [...base.transitions, ...base.transitions],
      }).disposition,
    ).toBe("FAIL")
  })

  test("a hidden repair fails even when the marker made it across", () => {
    const verdict = evaluateFullAutoAcceptance(fullAutoAcceptanceTest("test-01"), {
      ...passingContextEvidence("ORBIT-17"),
      hiddenRepairCount: 1,
    })
    expect(verdict.disposition).toBe("FAIL")
    expect(verdict.reasons.some(reason => reason.startsWith("no_hidden_repair:"))).toBe(true)
  })

  test("BLOCKED takes precedence over rule outcomes -- an outage is never PASS and never FAIL", () => {
    const verdict = evaluateFullAutoAcceptance(fullAutoAcceptanceTest("test-01"), {
      ...passingContextEvidence("ORBIT-17"),
      blockedReason: "target lane claude-local unavailable: provider outage",
    })
    expect(verdict.disposition).toBe("BLOCKED")
    expect(verdict.reasons).toEqual([
      "blocked: target lane claude-local unavailable: provider outage",
    ])
  })

  test("TEST 03: silent truncation fails; surfaced + confirmed truncation passes", () => {
    const base = {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: "thread-3",
      threadRefsTouched: ["thread-3"],
      objectiveDeliveredToTarget: true,
      acceptanceRuleDeliveredToTarget: true,
      transitions: passingContextEvidence("ORBIT-17").transitions,
    }
    const silent = evaluateFullAutoAcceptance(fullAutoAcceptanceTest("test-03"), {
      ...base,
      contextTruncated: true,
      truncationAcknowledged: false,
      truncationConfirmationRecorded: false,
    })
    expect(silent.disposition).toBe("FAIL")
    expect(
      silent.reasons.some(reason => reason.startsWith("truncation_acknowledged_when_present:")),
    ).toBe(true)
    const confirmed = evaluateFullAutoAcceptance(fullAutoAcceptanceTest("test-03"), {
      ...base,
      contextTruncated: true,
      truncationAcknowledged: true,
      truncationConfirmationRecorded: true,
    })
    expect(confirmed.disposition).toBe("PASS")
  })

  test("TEST 05: a different resumed runRef (duplicate dispatch shape) fails restart continuity", () => {
    const evidence = {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: "thread-5",
      threadRefsTouched: ["thread-5"],
      autonomousTurnsCompleted: 3,
      restartBoundariesObserved: 1,
      initialRunRef: "run.full-auto.a",
      resumedRunRef: "run.full-auto.b",
      runFieldsContinuous: false,
      duplicateDispatchCount: 1,
      continuationDispatchCounts: [1, 2, 1],
      reportPresent: true,
      analysisPresent: true,
      reportSpansRestart: false,
      finalStateReason: "done",
    }
    const verdict = evaluateFullAutoAcceptance(fullAutoAcceptanceTest("test-05"), evidence)
    expect(verdict.disposition).toBe("FAIL")
    expect(verdict.reasons.some(reason => reason.startsWith("resumed_same_run_across_restart:"))).toBe(true)
    expect(verdict.reasons.some(reason => reason.startsWith("no_duplicate_dispatch:"))).toBe(true)
    expect(verdict.reasons.some(reason => reason.startsWith("report_spans_restart:"))).toBe(true)
  })

  test("TEST 06: a lost continuation (a cycle that dispatched zero times) fails exactly-once", () => {
    const evidence = {
      ...EMPTY_FULL_AUTO_ACCEPTANCE_EVIDENCE,
      threadRef: "thread-6",
      threadRefsTouched: ["thread-6"],
      autonomousTurnsCompleted: 2,
      otherChatsOpened: 6,
      threadAddressableUnderPressure: false,
      continuationDispatchCounts: [1, 1, 0],
      reportPresent: true,
      analysisPresent: true,
    }
    const verdict = evaluateFullAutoAcceptance(fullAutoAcceptanceTest("test-06"), evidence)
    expect(verdict.disposition).toBe("FAIL")
    expect(
      verdict.reasons.some(reason => reason.startsWith("continuation_started_exactly_once:")),
    ).toBe(true)
  })
})

describe("FA-QA-01 title-prefix discipline", () => {
  test("the prefix mint produces the exact sidebar forms and never double-prefixes", () => {
    const title = "TEST 01 · Codex → Claude · context"
    expect(acceptanceTitleWithDisposition(title, "PASS")).toBe(`PASS · ${title}`)
    expect(acceptanceTitleWithDisposition(title, "FAIL")).toBe(`FAIL · ${title}`)
    expect(acceptanceTitleWithDisposition(title, "BLOCKED")).toBe(`BLOCKED · ${title}`)
    // A rerun after a FAIL re-prefixes cleanly rather than stacking.
    expect(acceptanceTitleWithDisposition(`FAIL · ${title}`, "PASS")).toBe(`PASS · ${title}`)
  })

  test("disposition parsing round-trips and an unprefixed title reads as no verdict yet", () => {
    const title = "TEST 06 · Full Auto · thread pressure"
    expect(acceptanceTitleDisposition(title)).toBeNull()
    expect(acceptanceTitleDisposition(`BLOCKED · ${title}`)).toBe("BLOCKED")
    expect(stripAcceptanceDisposition(`BLOCKED · ${title}`)).toBe(title)
    expect(stripAcceptanceDisposition(title)).toBe(title)
  })
})
