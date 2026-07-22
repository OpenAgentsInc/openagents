import { execFileSync } from "node:child_process"
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { Schema } from "effect"
import { afterAll, describe, expect, test } from "vite-plus/test"

import type { FullAutoRun } from "./full-auto-run-registry.ts"
import type { FullAutoRunReport } from "./full-auto-run-report.ts"
import {
  buildFullAutoGradingBaseline,
  countVerifiedOutcomes,
  FULL_AUTO_GRADING_RECORD_SHAPE_GAPS,
  FullAutoGradingBaselineSchema,
  gradeFullAutoRun,
  readFullAutoRunReports,
  readFullAutoRuns,
  renderFullAutoGradingBaselineMarkdown,
  RUBRIC_DIMENSIONS,
  runReportComplexity,
} from "./run-grading.ts"

// -----------------------------------------------------------------------
// Synthetic record builders -- every field is public-safe fixture data.
// -----------------------------------------------------------------------

const HEX_A = "a".repeat(64)
const HEX_B = "b".repeat(64)
const NOW = () => new Date("2026-07-22T12:00:00.000Z")

type TurnSpec = Readonly<{
  disposition: "completed" | "failed" | "owner_interrupted" | null
  at: string
}>

const makeTurns = (specs: ReadonlyArray<TurnSpec>): FullAutoRunReport["turns"] =>
  specs.map((spec, index) => ({
    turnRef: `turn.fx.${index + 1}`,
    lane: "codex-local",
    phase: spec.disposition === null ? "streaming" : spec.disposition === "completed" ? "completed" : "failed",
    disposition: spec.disposition,
    createdAt: spec.at,
    updatedAt: spec.at,
    outcomeSummary: spec.disposition === null ? "turn in phase streaming" : `turn ${spec.disposition}`,
  }))

const makeReport = (overrides: Partial<FullAutoRunReport> = {}): FullAutoRunReport => ({
  schema: "openagents.desktop.full_auto_run_report.v1",
  runRef: "run.full-auto.fx",
  title: "Fixture run",
  objectiveDigest: HEX_A,
  doneConditionDigest: HEX_B,
  objectiveRevisionCount: 1,
  turnCap: 20,
  successfulAttempts: 0,
  failedAttempts: 0,
  state: "running",
  createdAt: "2026-07-22T00:00:00.000Z",
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
  updatedAt: "2026-07-22T00:00:00.000Z",
  ...overrides,
})

const makeRun = (overrides: Partial<FullAutoRun> = {}): FullAutoRun => ({
  runRef: "run.full-auto.fx",
  title: "Fixture run",
  objective: "Fixture objective.",
  objectiveSource: "user",
  doneCondition: "Fixture done condition.",
  objectiveHistory: [],
  turnCap: 20,
  successfulAttempts: 0,
  failedAttempts: 0,
  state: "running",
  stateRevision: 1,
  createdAt: "2026-07-22T00:00:00.000Z",
  transitions: [],
  ...overrides,
})

const plan = (statuses: ReadonlyArray<"pending" | "in_progress" | "done" | "blocked" | "skipped">) => ({
  schema: "openagents.desktop.full_auto_plan.v1" as const,
  steps: statuses.map((status, index) => ({
    stepRef: `s${index + 1}`,
    title: `Step ${index + 1}`,
    status,
    dependsOn: [],
  })),
  revision: 1,
  createdAt: "2026-07-22T00:00:00.000Z",
  updatedAt: "2026-07-22T00:00:00.000Z",
})

const verification = (status: "passed" | "failed" | "absent" | "error") => ({
  schema: "openagents.desktop.full_auto_verification_result.v1" as const,
  spec: { kind: "command" as const, command: "pnpm test" },
  status,
  exitCode: status === "passed" ? 0 : status === "failed" ? 1 : null,
  detail: `fixture ${status}`,
  at: "2026-07-22T00:30:00.000Z",
})

// -----------------------------------------------------------------------
// Per-dimension scoring.
// -----------------------------------------------------------------------

describe("run grading dimensions (META-3 #9182)", () => {
  test("D1: plan-tracked run scores 3; multi-turn without a plan scores 2; ceiling is 3", () => {
    const turns = makeTurns([
      { disposition: "completed", at: "2026-07-22T00:01:00.000Z" },
      { disposition: "completed", at: "2026-07-22T00:02:00.000Z" },
    ])
    const withPlan = gradeFullAutoRun({
      report: makeReport({ turns }),
      run: makeRun({ autonomy: { enabled: true, plan: plan(["done", "pending"]) } }),
      now: NOW,
    })
    expect(withPlan.dimensions.D1_complexity).toMatchObject({ measured: true, score: 3, mechanicalCeiling: 3 })
    const withoutPlan = gradeFullAutoRun({ report: makeReport({ turns }), run: makeRun(), now: NOW })
    expect(withoutPlan.dimensions.D1_complexity).toMatchObject({ measured: true, score: 2 })
  })

  test("D1/D2: a run with zero turns is not_measured, never fabricated", () => {
    const grade = gradeFullAutoRun({ report: makeReport(), run: makeRun(), now: NOW })
    expect(grade.dimensions.D1_complexity).toMatchObject({ measured: false, reason: "no_turns_recorded" })
    expect(grade.dimensions.D2_coherence).toMatchObject({ measured: false, reason: "no_turns_recorded" })
  })

  test("D2: objective drift lowers coherence to 1; drift-free plan advancement scores 3", () => {
    const turns = makeTurns([
      { disposition: "completed", at: "2026-07-22T00:01:00.000Z" },
      { disposition: "completed", at: "2026-07-22T00:02:00.000Z" },
    ])
    const drifted = gradeFullAutoRun({
      report: makeReport({ turns, objectiveRevisionCount: 3 }),
      run: makeRun(),
      now: NOW,
    })
    expect(drifted.dimensions.D2_coherence).toMatchObject({ measured: true, score: 1 })
    const planned = gradeFullAutoRun({
      report: makeReport({ turns }),
      run: makeRun({ autonomy: { enabled: true, plan: plan(["done", "pending"]) } }),
      now: NOW,
    })
    expect(planned.dimensions.D2_coherence).toMatchObject({ measured: true, score: 3, mechanicalCeiling: 3 })
  })

  test("D3: no plan is myopic (1); unadvanced plan is 2; advanced plan is 3; no run record is not_measured", () => {
    const report = makeReport({ turns: makeTurns([{ disposition: "completed", at: "2026-07-22T00:01:00.000Z" }]) })
    expect(gradeFullAutoRun({ report, run: makeRun(), now: NOW }).dimensions.D3_foresight).toMatchObject({
      measured: true,
      score: 1,
    })
    expect(
      gradeFullAutoRun({
        report,
        run: makeRun({ autonomy: { enabled: true, plan: plan(["pending", "pending"]) } }),
        now: NOW,
      }).dimensions.D3_foresight,
    ).toMatchObject({ measured: true, score: 2 })
    expect(
      gradeFullAutoRun({
        report,
        run: makeRun({ autonomy: { enabled: true, plan: plan(["done", "in_progress"]) } }),
        now: NOW,
      }).dimensions.D3_foresight,
    ).toMatchObject({ measured: true, score: 3 })
    expect(gradeFullAutoRun({ report, run: null, now: NOW }).dimensions.D3_foresight).toMatchObject({
      measured: false,
      reason: "run_record_unavailable",
    })
  })

  test("D4: verified refs score 3, claimed-only 2, completed-with-none 1, nothing not_measured", () => {
    const completedTurn = makeTurns([{ disposition: "completed", at: "2026-07-22T00:01:00.000Z" }])
    const verified = gradeFullAutoRun({
      report: makeReport({
        turns: completedTurn,
        verifiedRefs: [{ ref: "receipt.x", kind: "receipt", verification: "verified" }],
      }),
      now: NOW,
    })
    expect(verified.dimensions.D4_groundedness).toMatchObject({ measured: true, score: 3, mechanicalCeiling: 3 })
    const claimed = gradeFullAutoRun({
      report: makeReport({
        turns: completedTurn,
        verifiedRefs: [{ ref: "0".repeat(40), kind: "commit", verification: "claimed", turnRef: "turn.fx.1" }],
      }),
      now: NOW,
    })
    expect(claimed.dimensions.D4_groundedness).toMatchObject({ measured: true, score: 2 })
    const bare = gradeFullAutoRun({ report: makeReport({ turns: completedTurn }), now: NOW })
    expect(bare.dimensions.D4_groundedness).toMatchObject({ measured: true, score: 1 })
    const idle = gradeFullAutoRun({ report: makeReport(), now: NOW })
    expect(idle.dimensions.D4_groundedness).toMatchObject({
      measured: false,
      reason: "no_completed_turns_and_no_refs",
    })
  })

  test("D5: owner-supplied objective is 0, legacy generic objective is 1, ceiling is 1 (no system_selected literal exists)", () => {
    const report = makeReport()
    const owner = gradeFullAutoRun({ report, run: makeRun({ objectiveSource: "user" }), now: NOW })
    expect(owner.dimensions.D5_selectivity).toMatchObject({ measured: true, score: 0, mechanicalCeiling: 1 })
    const legacy = gradeFullAutoRun({ report, run: makeRun({ objectiveSource: "legacy_migration" }), now: NOW })
    expect(legacy.dimensions.D5_selectivity).toMatchObject({ measured: true, score: 1, mechanicalCeiling: 1 })
    const unjoined = gradeFullAutoRun({ report, run: null, now: NOW })
    expect(unjoined.dimensions.D5_selectivity).toMatchObject({ measured: false, reason: "run_record_unavailable" })
  })

  test("D6: failed host verdict holding a run out of completed scores 4; passed scores 3; self-report ranks 1-2", () => {
    const completedTurn = makeTurns([{ disposition: "completed", at: "2026-07-22T00:01:00.000Z" }])
    const blocked = gradeFullAutoRun({
      report: makeReport({ turns: completedTurn, state: "running" }),
      run: makeRun({ autonomy: { enabled: true, lastVerification: verification("failed") } }),
      now: NOW,
    })
    expect(blocked.dimensions.D6_self_verification).toMatchObject({ measured: true, score: 4, mechanicalCeiling: 4 })
    const passed = gradeFullAutoRun({
      report: makeReport({ turns: completedTurn, state: "completed" }),
      run: makeRun({ autonomy: { enabled: true, lastVerification: verification("passed") } }),
      now: NOW,
    })
    expect(passed.dimensions.D6_self_verification).toMatchObject({ measured: true, score: 3 })
    const riskyCompletion = gradeFullAutoRun({
      report: makeReport({ turns: completedTurn, state: "completed" }),
      run: makeRun(),
      now: NOW,
    })
    expect(riskyCompletion.dimensions.D6_self_verification).toMatchObject({ measured: true, score: 1 })
    const withRefs = gradeFullAutoRun({
      report: makeReport({
        turns: completedTurn,
        state: "completed",
        verifiedRefs: [{ ref: "1".repeat(40), kind: "commit", verification: "claimed" }],
      }),
      run: makeRun(),
      now: NOW,
    })
    expect(withRefs.dimensions.D6_self_verification).toMatchObject({ measured: true, score: 2 })
    const neverCompleted = gradeFullAutoRun({ report: makeReport(), run: makeRun(), now: NOW })
    expect(neverCompleted.dimensions.D6_self_verification).toMatchObject({
      measured: false,
      reason: "no_completion_observed_and_no_host_verification",
    })
  })

  test("D7: no blocker is not_measured; recovery after failure is 3; typed forced stop is 2; bare failure is 1", () => {
    const clean = gradeFullAutoRun({
      report: makeReport({ turns: makeTurns([{ disposition: "completed", at: "2026-07-22T00:01:00.000Z" }]) }),
      now: NOW,
    })
    expect(clean.dimensions.D7_recoverability).toMatchObject({ measured: false, reason: "no_blocker_observed" })
    const recovered = gradeFullAutoRun({
      report: makeReport({
        turns: makeTurns([
          { disposition: "failed", at: "2026-07-22T00:01:00.000Z" },
          { disposition: "completed", at: "2026-07-22T00:02:00.000Z" },
        ]),
      }),
      now: NOW,
    })
    expect(recovered.dimensions.D7_recoverability).toMatchObject({ measured: true, score: 3, mechanicalCeiling: 3 })
    const forcedStop = gradeFullAutoRun({
      report: makeReport({
        state: "stopped",
        turns: makeTurns([{ disposition: "failed", at: "2026-07-22T00:01:00.000Z" }]),
        lifecycleTransitions: [
          {
            from: "running",
            to: "stopped",
            actor: "dispatch_failure_limit",
            at: "2026-07-22T00:05:00.000Z",
            reason: "failure limit",
          },
        ],
      }),
      now: NOW,
    })
    expect(forcedStop.dimensions.D7_recoverability).toMatchObject({ measured: true, score: 2 })
    const unrecovered = gradeFullAutoRun({
      report: makeReport({ turns: makeTurns([{ disposition: "failed", at: "2026-07-22T00:01:00.000Z" }]) }),
      now: NOW,
    })
    expect(unrecovered.dimensions.D7_recoverability).toMatchObject({ measured: true, score: 1 })
  })
})

// -----------------------------------------------------------------------
// COH-01 reuse, verified outcomes, and cost honesty.
// -----------------------------------------------------------------------

describe("coherence@complexity and cost per verified outcome", () => {
  test("complexity reuses COH-01 computeComplexity as an explicit lower bound and never claims the coherence screen ran", () => {
    const report = makeReport({
      turns: makeTurns([
        { disposition: "completed", at: "2026-07-22T00:01:00.000Z" },
        { disposition: "completed", at: "2026-07-22T00:02:00.000Z" },
        { disposition: "completed", at: "2026-07-22T00:03:00.000Z" },
      ]),
    })
    const assessment = runReportComplexity(report)
    expect(assessment.score).toBeGreaterThan(0)
    const grade = gradeFullAutoRun({ report, now: NOW })
    expect(grade.complexity).toMatchObject({
      metric: "coherence-screen-v2",
      lowerBound: true,
      score: assessment.score,
      tier: assessment.tier,
      coherenceScreenMeasured: false,
      coherenceScreenReason: "transcript_signals_not_in_run_records",
    })
  })

  test("verified outcomes count host-verified refs plus a passed host verification -- never claimed refs or self-reports", () => {
    const report = makeReport({
      verifiedRefs: [
        { ref: "receipt.x", kind: "receipt", verification: "verified" },
        { ref: "2".repeat(40), kind: "commit", verification: "claimed" },
      ],
    })
    expect(countVerifiedOutcomes(report, null)).toBe(1)
    expect(
      countVerifiedOutcomes(report, makeRun({ autonomy: { enabled: true, lastVerification: verification("passed") } })),
    ).toBe(2)
    expect(
      countVerifiedOutcomes(report, makeRun({ autonomy: { enabled: true, lastVerification: verification("failed") } })),
    ).toBe(1)
  })

  test("cost is measured only with known usage AND verified outcomes; each absence is a typed reason", () => {
    const usageKnown = { totalTokensKnown: true, totalTokens: 120000, costUsdKnown: true, costUsd: 3.6 }
    const measuredGrade = gradeFullAutoRun({
      report: makeReport({
        usage: usageKnown,
        startedAt: "2026-07-22T00:00:00.000Z",
        endedAt: "2026-07-22T01:00:00.000Z",
        verifiedRefs: [{ ref: "receipt.x", kind: "receipt", verification: "verified" }],
      }),
      run: makeRun({ autonomy: { enabled: true, lastVerification: verification("passed") } }),
      now: NOW,
    })
    expect(measuredGrade.costPerVerifiedOutcome).toMatchObject({
      measured: true,
      verifiedOutcomes: 2,
      totalTokens: 120000,
      tokensPerVerifiedOutcome: 60000,
      costUsdPerVerifiedOutcome: 1.8,
      wallClockMsPerVerifiedOutcome: 1800000,
    })
    const usageUnknown = gradeFullAutoRun({
      report: makeReport({ verifiedRefs: [{ ref: "receipt.x", kind: "receipt", verification: "verified" }] }),
      now: NOW,
    })
    expect(usageUnknown.costPerVerifiedOutcome).toMatchObject({ measured: false, reason: "usage_unknown" })
    const zeroOutcomes = gradeFullAutoRun({ report: makeReport({ usage: usageKnown }), now: NOW })
    expect(zeroOutcomes.costPerVerifiedOutcome).toMatchObject({
      measured: false,
      reason: "zero_verified_outcomes",
      verifiedOutcomes: 0,
    })
  })
})

// -----------------------------------------------------------------------
// Baseline aggregation.
// -----------------------------------------------------------------------

describe("baseline aggregation", () => {
  test("aggregates decode against the schema, keep not_measured out of means, and carry the record-shape gaps", () => {
    const baseline = buildFullAutoGradingBaseline({
      entries: [
        { report: makeReport({ runRef: "run.full-auto.fx-1" }), run: makeRun({ runRef: "run.full-auto.fx-1" }) },
        {
          report: makeReport({
            runRef: "run.full-auto.fx-2",
            turns: makeTurns([
              { disposition: "completed", at: "2026-07-22T00:01:00.000Z" },
              { disposition: "completed", at: "2026-07-22T00:02:00.000Z" },
            ]),
          }),
          run: null,
        },
      ],
      now: NOW,
    })
    expect(() => Schema.decodeUnknownSync(FullAutoGradingBaselineSchema)(baseline)).not.toThrow()
    expect(baseline.runCount).toBe(2)
    expect(baseline.runRecordJoinedCount).toBe(1)
    const d1 = baseline.dimensions.find((entry) => entry.dimension === "D1_complexity")
    expect(d1).toMatchObject({ measuredCount: 1, notMeasuredCount: 1, meanScore: 2, minScore: 2, maxScore: 2 })
    expect(d1?.notMeasuredReasons).toMatchObject({ no_turns_recorded: 1 })
    const d5 = baseline.dimensions.find((entry) => entry.dimension === "D5_selectivity")
    expect(d5).toMatchObject({ measuredCount: 1, notMeasuredCount: 1 })
    expect(baseline.coherenceAtComplexity.coherenceScreenMeasured).toBe(false)
    expect(baseline.recordShapeGaps).toEqual(FULL_AUTO_GRADING_RECORD_SHAPE_GAPS)
    expect(baseline.dimensions).toHaveLength(RUBRIC_DIMENSIONS.length)
  })

  test("cost aggregate divides exact tokens by verified outcomes over measured runs only", () => {
    const usageKnown = { totalTokensKnown: true, totalTokens: 90000, costUsdKnown: false, costUsd: null }
    const baseline = buildFullAutoGradingBaseline({
      entries: [
        {
          report: makeReport({
            runRef: "run.full-auto.fx-cost",
            usage: usageKnown,
            verifiedRefs: [
              { ref: "receipt.a", kind: "receipt", verification: "verified" },
              { ref: "receipt.b", kind: "receipt", verification: "verified" },
              { ref: "receipt.c", kind: "receipt", verification: "verified" },
            ],
          }),
        },
        { report: makeReport({ runRef: "run.full-auto.fx-unknown" }) },
      ],
      now: NOW,
    })
    expect(baseline.cost).toMatchObject({
      runsWithKnownUsage: 1,
      runsWithVerifiedOutcomes: 1,
      totalVerifiedOutcomes: 3,
      tokensPerVerifiedOutcome: 30000,
      costUsdPerVerifiedOutcome: null,
    })
  })

  test("markdown rendering is bounded and public-safe: digests and refs, never objective text", () => {
    const baseline = buildFullAutoGradingBaseline({
      entries: [{ report: makeReport({ runRef: "run.full-auto.fx-md" }), run: makeRun({ runRef: "run.full-auto.fx-md" }) }],
      now: NOW,
    })
    const markdown = renderFullAutoGradingBaselineMarkdown(baseline)
    expect(markdown).toContain("run.full-auto.fx-md")
    expect(markdown).toContain("not_measured")
    expect(markdown).not.toContain("Fixture objective")
  })
})

// -----------------------------------------------------------------------
// Read-only loaders + CLI smoke over the committed fixture root.
// -----------------------------------------------------------------------

const FIXTURE_ROOT = path.join(import.meta.dirname, "..", "scripts", "fixtures", "full-auto-grading")
const CLI = path.join(import.meta.dirname, "..", "scripts", "grade-runs.ts")

const tempDirs: Array<string> = []
afterAll(() => {
  for (const dir of tempDirs) rmSync(dir, { recursive: true, force: true })
})

describe("read-only loaders and CLI smoke", () => {
  test("loaders decode the committed fixture stores read-only and report a missing root honestly", () => {
    const reports = readFullAutoRunReports(FIXTURE_ROOT)
    expect(reports.issue).toBe("none")
    expect(reports.values.length).toBe(6)
    const runs = readFullAutoRuns(FIXTURE_ROOT)
    expect(runs.issue).toBe("none")
    expect(runs.values.length).toBe(5)
    const missing = readFullAutoRunReports(path.join(FIXTURE_ROOT, "does-not-exist"))
    expect(missing.issue).toBe("missing")
    expect(missing.values).toEqual([])
  })

  test("an undecodable store is surfaced, never quarantined or mutated", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-grading-bad-"))
    tempDirs.push(root)
    const storeDir = path.join(root, "full-auto")
    execFileSync("mkdir", ["-p", storeDir])
    const filePath = path.join(storeDir, "run-reports.json")
    const before = '{"schema":"openagents.desktop.full_auto_run_report.v1","reports":[{"broken":true}]}'
    execFileSync("bash", ["-c", `printf '%s' '${before}' > ${JSON.stringify(filePath)}`])
    const load = readFullAutoRunReports(root)
    expect(load.issue).toBe("undecodable")
    expect(readFileSync(filePath, "utf8")).toBe(before)
    expect(readdirSync(storeDir)).toEqual(["run-reports.json"])
  })

  test("CLI smoke: grades the fixture root and writes a dated JSON + Markdown baseline artifact", () => {
    const out = mkdtempSync(path.join(tmpdir(), "oa-grading-out-"))
    tempDirs.push(out)
    const stdout = execFileSync(
      "node",
      [
        "--import",
        "tsx",
        CLI,
        "--user-data",
        FIXTURE_ROOT,
        "--out",
        out,
        "--now",
        "2026-07-22T12:00:00.000Z",
        "--json",
      ],
      { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
    )
    const baseline = JSON.parse(stdout) as Record<string, unknown>
    expect(() => Schema.decodeUnknownSync(FullAutoGradingBaselineSchema)(baseline)).not.toThrow()
    const decoded = Schema.decodeUnknownSync(FullAutoGradingBaselineSchema)(baseline)
    expect(decoded.runCount).toBe(6)
    expect(decoded.runRecordJoinedCount).toBe(5)
    // Signal spot checks across the fixture corpus.
    const byRef = new Map(decoded.grades.map((grade) => [grade.runRef, grade]))
    expect(byRef.get("run.full-auto.fx-autonomy")?.dimensions.D6_self_verification).toMatchObject({
      measured: true,
      score: 3,
    })
    expect(byRef.get("run.full-auto.fx-autonomy")?.costPerVerifiedOutcome).toMatchObject({
      measured: true,
      tokensPerVerifiedOutcome: 60000,
    })
    expect(byRef.get("run.full-auto.fx-verify-blocked")?.dimensions.D6_self_verification).toMatchObject({
      measured: true,
      score: 4,
    })
    expect(byRef.get("run.full-auto.fx-drift")?.dimensions.D2_coherence).toMatchObject({ measured: true, score: 1 })
    expect(byRef.get("run.full-auto.fx-drift")?.dimensions.D5_selectivity).toMatchObject({ measured: true, score: 1 })
    expect(byRef.get("run.full-auto.fx-stalled")?.dimensions.D7_recoverability).toMatchObject({
      measured: true,
      score: 2,
    })
    expect(byRef.get("run.full-auto.fx-stalled")?.dimensions.D5_selectivity).toMatchObject({
      measured: false,
      reason: "run_record_unavailable",
    })
    expect(byRef.get("run.full-auto.fx-empty")?.dimensions.D1_complexity).toMatchObject({
      measured: false,
      reason: "no_turns_recorded",
    })
    const written = readdirSync(out).toSorted()
    expect(written).toEqual([
      "full-auto-grading-baseline-2026-07-22T120000Z.json",
      "full-auto-grading-baseline-2026-07-22T120000Z.md",
    ])
    const markdown = readFileSync(path.join(out, written[1]!), "utf8")
    expect(markdown).toContain("# Full Auto grading baseline (full-auto-decision-v1)")
    expect(markdown).not.toContain("Fixture objective")
    // The fixture stores themselves were never mutated by grading.
    expect(existsSync(path.join(FIXTURE_ROOT, "full-auto", "run-reports.json.quarantined"))).toBe(false)
  })
})
