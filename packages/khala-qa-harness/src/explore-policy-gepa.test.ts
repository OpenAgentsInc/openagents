import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  admitKhalaCodeQaGepaExplorePolicyToGym,
  decodeKhalaCodeQaGepaExplorePolicyCandidate,
  decodeKhalaCodeQaScenarioPortfolioPlan,
  evaluateKhalaCodeQaGepaExplorePolicyAutoPromotion,
  makeKhalaCodeQaGepaExplorePolicyBrain,
  proposeKhalaCodeQaGepaExplorePolicyCandidate,
  rankKhalaCodeQaScenarioPortfolioByYield,
} from "./explore-policy-gepa.js"
import {
  collectKhalaCodeQaCoverageLedger,
  khalaCodeQaCoverageFrontierReport,
  KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
} from "./index.js"

const admittedCandidate = () => {
  const admission = admitKhalaCodeQaGepaExplorePolicyToGym({
    admittedAt: "2026-07-02T00:00:00.000Z",
    baselineScore: 99,
    candidateScore: 100.05,
    evaluatedScenarioIds: ["scenario.khala_code.monkey.fixture_smoke.alpha.v1"],
    offline: true,
    runRef: "gym.run.khala_code_qa_gepa.offline.alpha",
  })
  return proposeKhalaCodeQaGepaExplorePolicyCandidate({
    admission,
    evidence: [{
      actionCount: 40,
      confirmedBugRefs: ["bug.public.claim_invariant"],
      newCoverageRefs: ["rpcMethods:fleetRunStatus", "hotbarPanels:fleet"],
      runtimeMs: 10_000,
      sourceRunId: "monkey.fixture_smoke.alpha",
      status: "fail",
    }],
    generatedAt: "2026-07-02T00:00:00.000Z",
    parameters: {
      explorationEpsilon: 0,
      frontierWeight: 10,
    },
  })
}

describe("Khala Code QA GEPA explore policy loop", () => {
  test("emits an offline Gym-admitted candidate that cannot auto-promote", () => {
    const candidate = admittedCandidate()
    const decoded = decodeKhalaCodeQaGepaExplorePolicyCandidate(candidate)
    const promotion = evaluateKhalaCodeQaGepaExplorePolicyAutoPromotion(candidate)

    expect(decoded.schema).toBe("khala_code_qa_gepa_explore_policy_candidate.v1")
    expect(candidate.admission.state).toBe("admitted")
    expect(candidate.governance).toMatchObject({
      authorityBoundary: "evidence_only",
      autoPromote: false,
      live: false,
      offlineOnly: true,
      promotionState: "not_promoted",
      selfPromotionAllowed: false,
    })
    expect(candidate.metrics.confirmedBugsPerThousandActions).toBe(25)
    expect(promotion).toMatchObject({
      promoted: false,
    })
  })

  test("blocks live or metric-regressed candidates before they can drive exploration", () => {
    const blocked = admitKhalaCodeQaGepaExplorePolicyToGym({
      baselineScore: 2,
      candidateScore: 1,
      evaluatedScenarioIds: [],
      offline: false,
      runRef: "",
    })

    expect(blocked.state).toBe("blocked")
    expect(blocked.blockerRefs).toContain("blocker.khala_qa_gepa.live_input")
    expect(blocked.blockerRefs).toContain("blocker.khala_qa_gepa.no_evaluated_scenarios")
    expect(blocked.blockerRefs).toContain("blocker.khala_qa_gepa.metric_not_improved")
    expect(() =>
      proposeKhalaCodeQaGepaExplorePolicyCandidate({
        admission: blocked,
        evidence: [],
      })
    ).toThrow(/Gym-admitted/)
  })

  test("admitted policy brain remains offline and selects frontier-weighted actions", async () => {
    const candidate = admittedCandidate()
    const brain = makeKhalaCodeQaGepaExplorePolicyBrain(candidate)
    const ledger = collectKhalaCodeQaCoverageLedger({
      generatedAt: "2026-07-02T00:00:00.000Z",
      observations: [{
        action: { kind: "hotbar", target: "chat" },
        label: "hotbar:chat",
        ok: true,
      }],
      runId: "partial-ledger",
    })
    const decision = await Effect.runPromise(
      brain.nextAction({
        actionLog: [],
        actionSpace: [
          { kind: "type", target: "composer", text: "no coverage frontier" },
          { kind: "hotbar", target: "fleet" },
        ],
        frontier: khalaCodeQaCoverageFrontierReport({
          generatedAt: "2026-07-02T00:00:00.000Z",
          ledger,
          manifest: KHALA_CODE_QA_SEED_CORPUS_MANIFEST,
        }),
        prngState: 0xfffffff0,
        stepIndex: 0,
      }),
    )

    expect(brain.tier).toBe("deterministic_fixture")
    expect(decision.action).toEqual({ kind: "hotbar", target: "fleet" })
    expect(decision.frontierRef).toBe("hotbarPanels:fleet")
    expect(decision.rationale).toContain("gepa-policy")
  })

  test("ranks and prunes the scenario portfolio by bug yield under runtime budgets", () => {
    const plan = rankKhalaCodeQaScenarioPortfolioByYield({
      generatedAt: "2026-07-02T00:00:00.000Z",
      nightlyBudgetMs: 12_000,
      prePushBudgetMs: 1_500,
      scenarios: [
        {
          confirmedBugRefs: [],
          newCoverageRefs: [],
          runtimeMs: 100,
          scenarioId: "scenario.zero_yield",
        },
        {
          confirmedBugRefs: ["bug.public.one"],
          newCoverageRefs: ["hotbarPanels:fleet"],
          runtimeMs: 1_000,
          scenarioId: "scenario.cheap_bug",
          status: "fail",
        },
        {
          confirmedBugRefs: ["bug.public.two", "bug.public.three"],
          newCoverageRefs: [],
          runtimeMs: 10_000,
          scenarioId: "scenario.expensive_bug",
          status: "fail",
        },
        {
          confirmedBugRefs: [],
          newCoverageRefs: ["rpcMethods:codexThreadRead"],
          runtimeMs: 500,
          scenarioId: "scenario.coverage_only",
        },
      ],
    })
    const decoded = decodeKhalaCodeQaScenarioPortfolioPlan(plan)

    expect(decoded.schema).toBe("khala_code_qa_scenario_portfolio.v1")
    expect(plan.orderedScenarioIds).toEqual([
      "scenario.cheap_bug",
      "scenario.expensive_bug",
      "scenario.coverage_only",
      "scenario.zero_yield",
    ])
    expect(plan.prePushScenarioIds).toEqual(["scenario.cheap_bug"])
    expect(plan.nightlyScenarioIds).toEqual([
      "scenario.cheap_bug",
      "scenario.expensive_bug",
    ])
    expect(plan.prunedScenarioIds).toEqual(["scenario.coverage_only", "scenario.zero_yield"])
    expect(plan.entries.find((entry) => entry.scenarioId === "scenario.zero_yield")?.pruneReason).toBe("zero_yield")
    expect(plan.entries.find((entry) => entry.scenarioId === "scenario.coverage_only")?.pruneReason).toBe("zero_yield")
  })
})
