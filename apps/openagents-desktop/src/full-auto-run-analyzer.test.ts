// Oracle for FA-RUN-05 (#8973): the bounded, offline/private dogfood
// analyzer over a `FullAutoRunReport` (#8972). Asserts exact computed
// metric values (not just "runs without crashing"), reproduces the
// 2026-07-17 overnight incident's four distinct findings from a synthetic
// report shaped after the documented timeline
// (docs/fable/2026-07-17-full-auto-implementation-audit.md §2.1), and
// covers the issue's required fixture set: repetition, drift, false
// completion, missing evidence, context truncation, provider failure,
// runtime failure, and clean success.
import { describe, expect, test, vi } from "vite-plus/test"

import { Schema } from "effect"

import { FULL_AUTO_RUN_REPORT_SCHEMA, type FullAutoRunReport } from "./full-auto-run-report.ts"
import {
  FULL_AUTO_RUN_ANALYSIS_SCHEMA,
  FULL_AUTO_RUN_COMPARISON_SCHEMA,
  FULL_AUTO_RUN_MODEL_ASSISTED_REVIEW_SCHEMA,
  FullAutoRunModelAssistedReviewRequestSchema,
  analyzeFullAutoRunReport,
  compareFullAutoRunAnalyses,
  median,
  requestFullAutoRunModelAssistedReview,
  type FullAutoRunFinding,
} from "./full-auto-run-analyzer.ts"

const REF_DIGEST_A = "a".repeat(64)
const REF_DIGEST_B = "b".repeat(64)

const baseReport = (overrides: Partial<FullAutoRunReport> = {}): FullAutoRunReport => ({
  schema: FULL_AUTO_RUN_REPORT_SCHEMA,
  runRef: "run.test",
  title: "Test run",
  objectiveDigest: REF_DIGEST_A,
  doneConditionDigest: REF_DIGEST_A,
  objectiveRevisionCount: 1,
  turnCap: 20,
  successfulAttempts: 0,
  failedAttempts: 0,
  state: "running",
  createdAt: "2026-07-16T23:58:00.000Z",
  lifecycleTransitions: [],
  ownerActions: [],
  providerTransitions: [],
  livenessObservations: [],
  livenessGaps: [],
  uninterruptedIntervals: [],
  turns: [],
  verifiedRefs: [],
  progressDisposition: "unknown",
  usage: { totalTokensKnown: false, totalTokens: null, costUsdKnown: false, costUsd: null },
  rawEvidenceRef: null,
  reportRevision: 1,
  updatedAt: "2026-07-16T23:58:00.000Z",
  ...overrides,
})

const turn = (
  input: Readonly<{
    turnRef: string
    disposition: FullAutoRunReport["turns"][number]["disposition"]
    createdAt: string
    updatedAt: string
    lane?: string
  }>,
): FullAutoRunReport["turns"][number] => ({
  turnRef: input.turnRef,
  lane: input.lane ?? "codex-local",
  // `phase` is not read by the analyzer (only `disposition` is) -- any
  // valid `LocalTurnPhase` value works here.
  phase: "completed",
  disposition: input.disposition,
  createdAt: input.createdAt,
  updatedAt: input.updatedAt,
  outcomeSummary: input.disposition !== null ? `turn ${input.disposition}` : "turn in flight",
})

const NOW = () => new Date("2026-07-17T12:00:00.000Z")

describe("median()", () => {
  test("empty input is null, never 0", () => {
    expect(median([])).toBeNull()
  })
  test("single value", () => {
    expect(median([5])).toBe(5)
  })
  test("even count averages the two middle values", () => {
    expect(median([1, 3])).toBe(2)
    expect(median([4, 1, 3, 2])).toBe(2.5)
  })
  test("odd count returns the middle value", () => {
    expect(median([1, 2, 3])).toBe(2)
  })
})

describe("analyzeFullAutoRunReport: turn metrics", () => {
  test("computes exact successful-turn-rate and useful-work-disposition-rate", () => {
    const report = baseReport({
      turns: [
        turn({ turnRef: "t1", disposition: "completed", createdAt: "2026-07-17T00:00:00.000Z", updatedAt: "2026-07-17T00:01:00.000Z" }),
        turn({ turnRef: "t2", disposition: "completed", createdAt: "2026-07-17T00:02:00.000Z", updatedAt: "2026-07-17T00:03:00.000Z" }),
        turn({ turnRef: "t3", disposition: "failed", createdAt: "2026-07-17T00:04:00.000Z", updatedAt: "2026-07-17T00:05:00.000Z" }),
        turn({ turnRef: "t4", disposition: null, createdAt: "2026-07-17T00:06:00.000Z", updatedAt: "2026-07-17T00:06:00.000Z" }),
      ],
    })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    expect(analysis.turns.total).toBe(4)
    expect(analysis.turns.resolved).toBe(3)
    expect(analysis.turns.completed).toBe(2)
    expect(analysis.turns.failed).toBe(1)
    expect(analysis.turns.unresolved).toBe(1)
    // 2 completed / 3 resolved
    expect(analysis.turns.successfulTurnRate).toBeCloseTo(2 / 3, 10)
    // 2 completed / 4 total
    expect(analysis.turns.usefulWorkDispositionRate).toBe(0.5)
  })

  test("zero turns yields null rates, never fabricated zero", () => {
    const analysis = analyzeFullAutoRunReport(baseReport({ turns: [] }), NOW)
    expect(analysis.turns.successfulTurnRate).toBeNull()
    expect(analysis.turns.usefulWorkDispositionRate).toBeNull()
  })
})

describe("analyzeFullAutoRunReport: autonomy metrics", () => {
  test("consecutive-turn streaks split exactly on owner-attributed actions, with median", () => {
    // 3 turns, owner action, 1 turn, owner action, 5 turns -> streaks [3,1,5]
    const turns = [
      ...["t1", "t2", "t3"].map((ref, i) =>
        turn({ turnRef: ref, disposition: "completed", createdAt: `2026-07-17T00:0${i}:00.000Z`, updatedAt: `2026-07-17T00:0${i}:30.000Z` }),
      ),
      turn({ turnRef: "t4", disposition: "completed", createdAt: "2026-07-17T00:10:00.000Z", updatedAt: "2026-07-17T00:10:30.000Z" }),
      ...["t5", "t6", "t7", "t8", "t9"].map((ref, i) =>
        turn({ turnRef: ref, disposition: "completed", createdAt: `2026-07-17T00:2${i}:00.000Z`, updatedAt: `2026-07-17T00:2${i}:30.000Z` }),
      ),
    ]
    const report = baseReport({
      turns,
      ownerActions: [
        { from: "running", to: "pausing", actor: "owner_ui", at: "2026-07-17T00:05:00.000Z", reason: "owner paused" },
        { from: "running", to: "pausing", actor: "owner_ui", at: "2026-07-17T00:15:00.000Z", reason: "owner paused again" },
      ],
    })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    expect(analysis.autonomy.autonomousStreaks).toEqual([3, 1, 5])
    expect(analysis.autonomy.longestAutonomousStreak).toBe(5)
    expect(analysis.autonomy.medianAutonomousStreak).toBe(3)
  })

  test("no owner actions -> a single streak covering every turn", () => {
    const report = baseReport({
      turns: [
        turn({ turnRef: "t1", disposition: "completed", createdAt: "2026-07-17T00:00:00.000Z", updatedAt: "2026-07-17T00:01:00.000Z" }),
        turn({ turnRef: "t2", disposition: "completed", createdAt: "2026-07-17T00:02:00.000Z", updatedAt: "2026-07-17T00:03:00.000Z" }),
      ],
    })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    expect(analysis.autonomy.autonomousStreaks).toEqual([2])
    expect(analysis.autonomy.longestAutonomousStreak).toBe(2)
  })

  test("time-to-first-useful-outcome is measured from startedAt (falling back to createdAt) to the first completed turn, honestly unknown when there is none", () => {
    const report = baseReport({
      startedAt: "2026-07-17T00:00:00.000Z",
      turns: [
        turn({ turnRef: "t1", disposition: "failed", createdAt: "2026-07-17T00:01:00.000Z", updatedAt: "2026-07-17T00:01:30.000Z" }),
        turn({ turnRef: "t2", disposition: "completed", createdAt: "2026-07-17T00:05:00.000Z", updatedAt: "2026-07-17T00:06:00.000Z" }),
      ],
    })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    expect(analysis.autonomy.timeToFirstUsefulOutcomeKnown).toBe(true)
    expect(analysis.autonomy.timeToFirstUsefulOutcomeMs).toBe(5 * 60_000)

    const neverCompleted = analyzeFullAutoRunReport(
      baseReport({
        startedAt: "2026-07-17T00:00:00.000Z",
        turns: [turn({ turnRef: "t1", disposition: "failed", createdAt: "2026-07-17T00:01:00.000Z", updatedAt: "2026-07-17T00:01:30.000Z" })],
      }),
      NOW,
    )
    expect(neverCompleted.autonomy.timeToFirstUsefulOutcomeKnown).toBe(false)
    expect(neverCompleted.autonomy.timeToFirstUsefulOutcomeMs).toBeNull()
  })
})

describe("analyzeFullAutoRunReport: liveness metrics", () => {
  test("sums closed-gap durations exactly and estimates an ongoing gap separately", () => {
    const report = baseReport({
      livenessGaps: [
        { enteredAt: "2026-07-17T00:00:00.000Z", exitedAt: "2026-07-17T00:10:00.000Z", durationMs: 600_000, cause: "dispatch_overdue" },
        { enteredAt: "2026-07-17T01:00:00.000Z", exitedAt: null, durationMs: null, cause: "stale_lease" },
      ],
      livenessObservations: [
        { at: "2026-07-17T00:00:00.000Z", projectedState: "stalled", cause: "dispatch_overdue", recoveryAction: "retry_now", sinceLastProgressMs: 600_000 },
        { at: "2026-07-17T01:00:00.000Z", projectedState: "stalled", cause: "stale_lease", recoveryAction: "stop_only", sinceLastProgressMs: 900_000 },
      ],
      updatedAt: "2026-07-17T01:30:00.000Z",
    })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    expect(analysis.liveness.gapCount).toBe(2)
    expect(analysis.liveness.closedGapCount).toBe(1)
    expect(analysis.liveness.totalStalledMs).toBe(600_000)
    // ongoing gap estimated from its enteredAt to `now` (2026-07-17T12:00:00Z passed as the `now` fn)
    expect(analysis.liveness.ongoingGapEstimateMs).toBe(Date.parse("2026-07-17T12:00:00.000Z") - Date.parse("2026-07-17T01:00:00.000Z"))
    expect(analysis.liveness.longestClosedGapMs).toBe(600_000)
    expect(analysis.liveness.gapCountByCause.dispatch_overdue).toBe(1)
    expect(analysis.liveness.gapCountByCause.stale_lease).toBe(1)
    expect(analysis.liveness.recoveryActionCounts.retry_now).toBe(1)
    expect(analysis.liveness.recoveryActionCounts.stop_only).toBe(1)
  })

  test("no gaps at all -> zero stalled time, no ongoing estimate", () => {
    const analysis = analyzeFullAutoRunReport(baseReport({ livenessGaps: [] }), NOW)
    expect(analysis.liveness.totalStalledMs).toBe(0)
    expect(analysis.liveness.ongoingGapEstimateMs).toBeNull()
    expect(analysis.liveness.longestClosedGapMs).toBeNull()
  })
})

describe("analyzeFullAutoRunReport: control (pause/stop) metrics", () => {
  test("pause latency and reliability from pausing->paused pairs; system-forced stops counted separately from owner stops", () => {
    const report = baseReport({
      lifecycleTransitions: [
        { from: "running", to: "pausing", actor: "owner_ui", at: "2026-07-17T00:00:00.000Z", reason: "owner requested pause" },
        { from: "pausing", to: "paused", actor: "turn_resolution", at: "2026-07-17T00:00:30.000Z", reason: "in-flight turn resolved" },
        { from: "paused", to: "running", actor: "owner_ui", at: "2026-07-17T00:01:00.000Z", reason: "resumed" },
        { from: "running", to: "pausing", actor: "control_api", at: "2026-07-17T00:02:00.000Z", reason: "second pause request" },
        { from: "pausing", to: "stalled", actor: "liveness_monitor", at: "2026-07-17T00:12:00.000Z", reason: "never resolved -- stalled instead" },
        { from: "stalled", to: "stopped", actor: "dispatch_failure_limit", at: "2026-07-17T00:20:00.000Z", reason: "failure cap" },
        { from: "running", to: "stopped", actor: "owner_ui", at: "2026-07-17T00:30:00.000Z", reason: "owner stop" },
      ],
    })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    expect(analysis.control.pauseRequestCount).toBe(2)
    expect(analysis.control.pauseResolvedCount).toBe(1)
    expect(analysis.control.pauseReliabilityRate).toBe(0.5)
    expect(analysis.control.pauseLatenciesMs).toEqual([30_000])
    expect(analysis.control.medianPauseLatencyMs).toBe(30_000)
    expect(analysis.control.ownerStopRequestCount).toBe(1)
    expect(analysis.control.systemForcedStopCount).toBe(1)
  })
})

describe("analyzeFullAutoRunReport: provider metrics", () => {
  test("tallies disposition counts and computes truncated/refused rates", () => {
    const report = baseReport({
      providerTransitions: [
        { handoffRef: "h1", from: "codex-local", to: "claude-local", actor: "control_api", at: "2026-07-17T00:00:00.000Z", reason: "r1", disposition: "complete_within_bounds", truncated: false },
        { handoffRef: "h2", from: "claude-local", to: "codex-local", actor: "control_api", at: "2026-07-17T00:01:00.000Z", reason: "r2", disposition: "truncated_with_confirmation", truncated: true },
        { handoffRef: "h3", from: "codex-local", to: "grok-acp", actor: "control_api", at: "2026-07-17T00:02:00.000Z", reason: "r3", disposition: "refused", truncated: false, refusalReason: "missing_auth" },
        { handoffRef: "h4", from: "codex-local", to: "grok-acp", actor: "control_api", at: "2026-07-17T00:03:00.000Z", reason: "r4", disposition: "refused", truncated: false, refusalReason: "capability_mismatch" },
      ],
    })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    expect(analysis.provider.transitionCount).toBe(4)
    expect(analysis.provider.dispositionCounts.complete_within_bounds).toBe(1)
    expect(analysis.provider.dispositionCounts.truncated_with_confirmation).toBe(1)
    expect(analysis.provider.dispositionCounts.refused).toBe(2)
    expect(analysis.provider.truncatedRate).toBe(0.25)
    expect(analysis.provider.refusedRate).toBe(0.5)
  })
})

describe("analyzeFullAutoRunReport: evidence and objective metrics", () => {
  test("unverified claim rate and completion risk", () => {
    const report = baseReport({
      state: "completed",
      verifiedRefs: [
        { ref: "commit.abc", kind: "commit", verification: "verified" },
        { ref: "artifact.xyz", kind: "artifact", verification: "claimed" },
      ],
    })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    expect(analysis.evidence.verifiedRefCount).toBe(1)
    expect(analysis.evidence.claimedRefCount).toBe(1)
    expect(analysis.evidence.unverifiedClaimRate).toBe(0.5)
    expect(analysis.evidence.unverifiedCompletionRisk).toBe(false)
  })

  test("objective retention proxy from objectiveRevisionCount", () => {
    const retained = analyzeFullAutoRunReport(baseReport({ objectiveRevisionCount: 1 }), NOW)
    expect(retained.objective.retained).toBe(true)
    expect(retained.objective.revisionCount).toBe(0)

    const revised = analyzeFullAutoRunReport(baseReport({ objectiveRevisionCount: 3 }), NOW)
    expect(revised.objective.retained).toBe(false)
    expect(revised.objective.revisionCount).toBe(2)
  })
})

// ---------------------------------------------------------------------
// Required fixture: the 2026-07-17 overnight incident, synthetically
// reconstructed from the documented timeline (never the real private
// transcript) -- "successful packet, failed continuation, six-hour-
// equivalent liveness gap, and missing owner diagnosis as different
// findings."
// ---------------------------------------------------------------------
describe("overnight-incident fixture (2026-07-17)", () => {
  test("identifies the successful packet, failed continuation, six-hour liveness gap, and missing owner diagnosis as four separate findings", () => {
    const report = baseReport({
      runRef: "run.overnight",
      createdAt: "2026-07-16T23:58:00.000Z",
      startedAt: "2026-07-16T23:58:00.000Z",
      state: "running",
      turns: [
        // "The first autonomous packet completed successfully after about
        // 14m 40s."
        turn({
          turnRef: "turn.packet-1",
          disposition: "completed",
          createdAt: "2026-07-16T23:58:00.000Z",
          updatedAt: "2026-07-17T00:12:40.000Z",
        }),
        // The next continuation dispatch failed ("Turn failed" / "That
        // conversation no longer exists").
        turn({
          turnRef: "turn.continuation-2",
          disposition: "failed",
          createdAt: "2026-07-17T00:12:41.000Z",
          updatedAt: "2026-07-17T00:12:50.000Z",
        }),
      ],
      // "Roughly a six-hour silent loss of autonomy" -- exactly 6h here so
      // the assertion is exact, not approximate.
      livenessGaps: [
        {
          enteredAt: "2026-07-17T00:12:50.000Z",
          exitedAt: "2026-07-17T06:12:50.000Z",
          durationMs: 6 * 60 * 60_000,
          cause: "host_thread_missing",
        },
      ],
      // No owner-attributed transition falls inside the gap window -- "No
      // run-level diagnosis."
      ownerActions: [],
      lifecycleTransitions: [
        { from: "running", to: "stalled", actor: "liveness_monitor", at: "2026-07-17T00:12:50.000Z", reason: "no continuation accepted" },
        { from: "stalled", to: "running", actor: "liveness_monitor", at: "2026-07-17T06:12:50.000Z", reason: "continuation resumed" },
      ],
    })

    const analysis = analyzeFullAutoRunReport(report, NOW)
    const kinds = (kind: FullAutoRunFinding["kind"]) => analysis.findings.filter((finding) => finding.kind === kind)

    expect(kinds("successful_packet")).toHaveLength(1)
    expect(kinds("successful_packet")[0]!.evidenceRefs).toEqual(["turn.packet-1"])

    expect(kinds("failed_continuation")).toHaveLength(1)
    expect(kinds("failed_continuation")[0]!.evidenceRefs).toEqual(["turn.continuation-2"])

    expect(kinds("liveness_gap")).toHaveLength(1)
    expect(kinds("liveness_gap")[0]!.severity).toBe("concerning")
    expect(analysis.liveness.totalStalledMs).toBe(6 * 60 * 60_000)

    expect(kinds("missing_owner_diagnosis")).toHaveLength(1)

    // Four genuinely distinct finding kinds, not one collapsed summary.
    const distinctKinds = new Set(analysis.findings.map((finding) => finding.kind))
    expect(distinctKinds.has("successful_packet")).toBe(true)
    expect(distinctKinds.has("failed_continuation")).toBe(true)
    expect(distinctKinds.has("liveness_gap")).toBe(true)
    expect(distinctKinds.has("missing_owner_diagnosis")).toBe(true)
  })
})

// ---------------------------------------------------------------------
// Issue-required fixture coverage: repetition, drift, false completion,
// missing evidence, context truncation, provider failure, runtime
// failure, clean success.
// ---------------------------------------------------------------------
describe("analyzer fixture coverage", () => {
  test("repetition: 3+ consecutive same-disposition turns with no owner action produces repeated_disposition_pattern", () => {
    const report = baseReport({
      turns: [
        turn({ turnRef: "t1", disposition: "failed", createdAt: "2026-07-17T00:00:00.000Z", updatedAt: "2026-07-17T00:00:10.000Z" }),
        turn({ turnRef: "t2", disposition: "failed", createdAt: "2026-07-17T00:01:00.000Z", updatedAt: "2026-07-17T00:01:10.000Z" }),
        turn({ turnRef: "t3", disposition: "failed", createdAt: "2026-07-17T00:02:00.000Z", updatedAt: "2026-07-17T00:02:10.000Z" }),
        turn({ turnRef: "t4", disposition: "failed", createdAt: "2026-07-17T00:03:00.000Z", updatedAt: "2026-07-17T00:03:10.000Z" }),
      ],
    })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    const findings = analysis.findings.filter((f) => f.kind === "repeated_disposition_pattern")
    expect(findings).toHaveLength(1)
    expect(findings[0]!.evidenceRefs).toEqual(["t1", "t4"])
  })

  test("drift: objective revised after creation flags objective_drift_revision", () => {
    const analysis = analyzeFullAutoRunReport(baseReport({ objectiveRevisionCount: 2 }), NOW)
    expect(analysis.findings.some((f) => f.kind === "objective_drift_revision")).toBe(true)
  })

  test("false completion: completed state with zero verified/claimed refs flags unverified_completion_risk", () => {
    const analysis = analyzeFullAutoRunReport(baseReport({ state: "completed", verifiedRefs: [] }), NOW)
    expect(analysis.findings.some((f) => f.kind === "unverified_completion_risk")).toBe(true)
    expect(analysis.evidence.unverifiedCompletionRisk).toBe(true)
  })

  test("missing evidence: successful attempts with no refs, and unknown usage, each flag missing_evidence", () => {
    const analysis = analyzeFullAutoRunReport(
      baseReport({
        state: "running",
        successfulAttempts: 2,
        verifiedRefs: [],
        usage: { totalTokensKnown: false, totalTokens: null, costUsdKnown: false, costUsd: null },
      }),
      NOW,
    )
    const findings = analysis.findings.filter((f) => f.kind === "missing_evidence")
    expect(findings).toHaveLength(2)
  })

  test("context truncation: truncated_with_confirmation handoff flags context_truncated", () => {
    const analysis = analyzeFullAutoRunReport(
      baseReport({
        providerTransitions: [
          { handoffRef: "h1", from: "codex-local", to: "claude-local", actor: "control_api", at: "2026-07-17T00:00:00.000Z", reason: "r", disposition: "truncated_with_confirmation", truncated: true },
        ],
      }),
      NOW,
    )
    expect(analysis.findings.some((f) => f.kind === "context_truncated")).toBe(true)
  })

  test("provider failure: refused handoff flags provider_refused", () => {
    const analysis = analyzeFullAutoRunReport(
      baseReport({
        providerTransitions: [
          { handoffRef: "h1", from: "codex-local", to: "claude-local", actor: "control_api", at: "2026-07-17T00:00:00.000Z", reason: "r", disposition: "refused", truncated: false, refusalReason: "unadmitted_peer" },
        ],
      }),
      NOW,
    )
    const findings = analysis.findings.filter((f) => f.kind === "provider_refused")
    expect(findings).toHaveLength(1)
    expect(findings[0]!.summary).toContain("unadmitted_peer")
  })

  test("runtime failure: a failed turn flags failed_continuation", () => {
    const analysis = analyzeFullAutoRunReport(
      baseReport({
        turns: [turn({ turnRef: "t1", disposition: "failed", createdAt: "2026-07-17T00:00:00.000Z", updatedAt: "2026-07-17T00:00:10.000Z" })],
      }),
      NOW,
    )
    expect(analysis.findings.some((f) => f.kind === "failed_continuation")).toBe(true)
  })

  test("clean success: all turns completed, no gaps/truncation/refusal/evidence gaps -> clean_success, and no concerning findings", () => {
    const analysis = analyzeFullAutoRunReport(
      baseReport({
        state: "completed",
        turns: [
          turn({ turnRef: "t1", disposition: "completed", createdAt: "2026-07-17T00:00:00.000Z", updatedAt: "2026-07-17T00:01:00.000Z" }),
          turn({ turnRef: "t2", disposition: "completed", createdAt: "2026-07-17T00:02:00.000Z", updatedAt: "2026-07-17T00:03:00.000Z" }),
        ],
        verifiedRefs: [{ ref: "commit.abc", kind: "commit", verification: "verified" }],
        usage: { totalTokensKnown: true, totalTokens: 1200, costUsdKnown: true, costUsd: 0.02 },
      }),
      NOW,
    )
    expect(analysis.findings.some((f) => f.kind === "clean_success")).toBe(true)
    const concerning = analysis.findings.filter((f) => f.severity === "concerning")
    expect(concerning).toHaveLength(0)
  })
})

describe("compareFullAutoRunAnalyses", () => {
  const makeAnalysisPair = (livenessGaps: FullAutoRunReport["livenessGaps"]) => {
    const report = baseReport({ runRef: "run.compare", livenessGaps })
    return { report, analysis: analyzeFullAutoRunReport(report, NOW) }
  }

  test("refuses a false comparison when objectiveDigest differs", () => {
    const baseline = makeAnalysisPair([])
    const candidateReport = baseReport({ runRef: "run.other", objectiveDigest: REF_DIGEST_B })
    const candidate = { report: candidateReport, analysis: analyzeFullAutoRunReport(candidateReport, NOW) }

    const comparison = compareFullAutoRunAnalyses({ baseline, candidate, now: NOW })
    expect(comparison.schema).toBe(FULL_AUTO_RUN_COMPARISON_SCHEMA)
    expect(comparison.comparable).toBe(false)
    expect(comparison.refusalReason).toBe("objective_mismatch")
    expect(comparison.metrics).toEqual([])
  })

  test("refuses when doneConditionDigest differs even if objectiveDigest matches", () => {
    const baseline = makeAnalysisPair([])
    const candidateReport = baseReport({ runRef: "run.other", doneConditionDigest: REF_DIGEST_B })
    const candidate = { report: candidateReport, analysis: analyzeFullAutoRunReport(candidateReport, NOW) }

    const comparison = compareFullAutoRunAnalyses({ baseline, candidate, now: NOW })
    expect(comparison.comparable).toBe(false)
    expect(comparison.refusalReason).toBe("done_condition_mismatch")
  })

  test("shows before/after metrics with correct improve/regress direction when identities match", () => {
    const baseline = makeAnalysisPair([
      { enteredAt: "2026-07-17T00:00:00.000Z", exitedAt: "2026-07-17T00:10:00.000Z", durationMs: 600_000, cause: "dispatch_overdue" },
    ])
    const candidate = makeAnalysisPair([])

    const comparison = compareFullAutoRunAnalyses({
      baseline,
      candidate,
      testDefinitionRef: "sidebar-test-1",
      sourceRevisionRange: "abc123..def456",
      now: NOW,
    })
    expect(comparison.comparable).toBe(true)
    expect(comparison.testDefinitionRef).toBe("sidebar-test-1")
    expect(comparison.sourceRevisionRange).toBe("abc123..def456")

    const stalledMetric = comparison.metrics.find((m) => m.metric === "totalStalledMs")!
    expect(stalledMetric.baselineValue).toBe(600_000)
    expect(stalledMetric.candidateValue).toBe(0)
    expect(stalledMetric.deltaValue).toBe(-600_000)
    expect(stalledMetric.direction).toBe("improved") // lower stalled time is better

    const gapCountMetric = comparison.metrics.find((m) => m.metric === "gapCount")!
    expect(gapCountMetric.direction).toBe("improved")

    expect(comparison.recommendations).toEqual([])
  })

  test("recommends investigating a regressed metric", () => {
    const baseline = makeAnalysisPair([])
    const candidate = makeAnalysisPair([
      { enteredAt: "2026-07-17T00:00:00.000Z", exitedAt: "2026-07-17T00:10:00.000Z", durationMs: 600_000, cause: "dispatch_overdue" },
    ])
    const comparison = compareFullAutoRunAnalyses({ baseline, candidate, now: NOW })
    expect(comparison.recommendations.some((r) => r.includes("totalStalledMs"))).toBe(true)
  })
})

describe("requestFullAutoRunModelAssistedReview: optional, pinned, bounded, cost-visible, advisory-only", () => {
  test("a request cannot decode without armed: true", () => {
    expect(() =>
      Schema.decodeUnknownSync(FullAutoRunModelAssistedReviewRequestSchema)({
        runRef: "run.test",
        evaluatorVersion: "fa-review-v1",
        armed: false,
        costConsent: { acknowledgedBy: "owner_ui", acknowledgedAt: "2026-07-17T00:00:00.000Z" },
      }),
    ).toThrow()
  })

  test("never invoked automatically by analyzeFullAutoRunReport", () => {
    const report = baseReport({})
    const analysis = analyzeFullAutoRunReport(report, NOW)
    // The analysis result has no field that could carry or trigger a model
    // call -- structurally, not just by convention.
    expect(Object.keys(analysis)).not.toContain("modelAssistedReview")
  })

  test("calls the injected invoker exactly once and returns an advisory-only, cost-visible result", async () => {
    const report = baseReport({ runRef: "run.reviewed" })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    const invoke = vi.fn(async () => ({
      costUsd: 0.014,
      findings: [{ category: "drift" as const, summary: "Packet choice drifted after turn 2.", evidenceRefs: ["turn.packet-1"] }],
    }))

    const result = await requestFullAutoRunModelAssistedReview({
      request: {
        runRef: "run.reviewed",
        evaluatorVersion: "fa-review-v1",
        armed: true,
        costConsent: { acknowledgedBy: "owner_ui", acknowledgedAt: "2026-07-17T00:00:00.000Z" },
      },
      report,
      analysis,
      invoke,
      now: NOW,
    })

    expect(invoke).toHaveBeenCalledTimes(1)
    expect(result.schema).toBe(FULL_AUTO_RUN_MODEL_ASSISTED_REVIEW_SCHEMA)
    expect(result.advisory).toBe(true)
    expect(result.costUsd).toBe(0.014)
    expect(result.findings).toHaveLength(1)
    expect(result.evaluatorVersion).toBe("fa-review-v1")
  })

  test("refuses to run against a mismatched report", async () => {
    const report = baseReport({ runRef: "run.a" })
    const analysis = analyzeFullAutoRunReport(report, NOW)
    const invoke = vi.fn(async () => ({ costUsd: 0, findings: [] }))

    await expect(
      requestFullAutoRunModelAssistedReview({
        request: {
          runRef: "run.b",
          evaluatorVersion: "fa-review-v1",
          armed: true,
          costConsent: { acknowledgedBy: "owner_ui", acknowledgedAt: "2026-07-17T00:00:00.000Z" },
        },
        report,
        analysis,
        invoke,
        now: NOW,
      }),
    ).rejects.toThrow()
    expect(invoke).not.toHaveBeenCalled()
  })
})

test("FULL_AUTO_RUN_ANALYSIS_SCHEMA constant matches the analysis payload's own schema field", () => {
  const analysis = analyzeFullAutoRunReport(baseReport({}), NOW)
  expect(analysis.schema).toBe(FULL_AUTO_RUN_ANALYSIS_SCHEMA)
})
