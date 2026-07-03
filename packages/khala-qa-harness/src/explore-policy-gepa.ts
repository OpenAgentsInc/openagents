import { Effect, Schema as S } from "effect"

import {
  khalaCodeQaActionCoverageRefs,
  type KhalaCodeQaExplorerBrain,
  type KhalaCodeQaExplorerBrainDecision,
} from "./explorer-brain.js"
import type { KhalaCodeQaAction } from "./scenario.js"

export const KHALA_CODE_QA_GEPA_EXPLORE_POLICY_SCHEMA =
  "khala_code_qa_gepa_explore_policy_candidate.v1" as const
export const KHALA_CODE_QA_GEPA_GYM_ADMISSION_SCHEMA =
  "khala_code_qa_gepa_gym_admission.v1" as const
export const KHALA_CODE_QA_SCENARIO_PORTFOLIO_SCHEMA =
  "khala_code_qa_scenario_portfolio.v1" as const
export const KHALA_CODE_QA_GEPA_METRIC_DEFINITION_SCHEMA =
  "khala_code_qa_gepa_metric_definition.v1" as const
export const KHALA_CODE_QA_GEPA_RELEASE_GATE_REF =
  "khala_code_qa.release_gate.gepa_explore_policy.v1" as const

export const KhalaCodeQaGepaMetricDefinitionSchema = S.Struct({
  denominator: S.String,
  formula: S.String,
  metricRef: S.Literals(["new_coverage_per_action", "confirmed_bugs_per_1000_actions"]),
  numerator: S.String,
  objective: S.Literal("maximize"),
  schema: S.Literal(KHALA_CODE_QA_GEPA_METRIC_DEFINITION_SCHEMA),
  source: S.Literal("offline_explore_telemetry"),
  unit: S.String,
})

export type KhalaCodeQaGepaMetricDefinition = {
  readonly schema: typeof KHALA_CODE_QA_GEPA_METRIC_DEFINITION_SCHEMA
  readonly denominator: string
  readonly formula: string
  readonly metricRef: "new_coverage_per_action" | "confirmed_bugs_per_1000_actions"
  readonly numerator: string
  readonly objective: "maximize"
  readonly source: "offline_explore_telemetry"
  readonly unit: string
}

export const KHALA_CODE_QA_GEPA_EXPLORE_POLICY_METRIC_DEFINITIONS = [
  {
    denominator: "total offline explorer actions across admitted source runs",
    formula: "deduped_new_coverage_refs / total_action_count",
    metricRef: "new_coverage_per_action",
    numerator: "deduped coverage refs first reached by the candidate source runs",
    objective: "maximize",
    schema: KHALA_CODE_QA_GEPA_METRIC_DEFINITION_SCHEMA,
    source: "offline_explore_telemetry",
    unit: "coverage refs per action",
  },
  {
    denominator: "total offline explorer actions across admitted source runs / 1000",
    formula: "(deduped_confirmed_bug_refs * 1000) / total_action_count",
    metricRef: "confirmed_bugs_per_1000_actions",
    numerator: "deduped public-safe confirmed bug refs from the candidate source runs",
    objective: "maximize",
    schema: KHALA_CODE_QA_GEPA_METRIC_DEFINITION_SCHEMA,
    source: "offline_explore_telemetry",
    unit: "confirmed bugs per 1000 actions",
  },
] as const satisfies readonly KhalaCodeQaGepaMetricDefinition[]

export const KhalaCodeQaGepaExplorePolicyParametersSchema = S.Struct({
  actionSelectionHeuristic: S.String,
  coverageNoveltyWeight: S.Number,
  confirmedBugWeight: S.Number,
  explorationEpsilon: S.Number,
  frontierWeight: S.Number,
  goalPrompt: S.String,
  maxActionsPerScenario: S.Number,
  runtimeCostWeight: S.Number,
})

export type KhalaCodeQaGepaExplorePolicyParameters = {
  readonly actionSelectionHeuristic: string
  readonly coverageNoveltyWeight: number
  readonly confirmedBugWeight: number
  readonly explorationEpsilon: number
  readonly frontierWeight: number
  readonly goalPrompt: string
  readonly maxActionsPerScenario: number
  readonly runtimeCostWeight: number
}

export const KhalaCodeQaGepaExplorePolicyGovernanceSchema = S.Struct({
  authorityBoundary: S.Literal("evidence_only"),
  autoPromote: S.Literal(false),
  live: S.Literal(false),
  offlineOnly: S.Literal(true),
  promotionState: S.Literal("not_promoted"),
  releaseGateRef: S.Literal(KHALA_CODE_QA_GEPA_RELEASE_GATE_REF),
  selfPromotionAllowed: S.Literal(false),
})

export type KhalaCodeQaGepaExplorePolicyGovernance = {
  readonly authorityBoundary: "evidence_only"
  readonly autoPromote: false
  readonly live: false
  readonly offlineOnly: true
  readonly promotionState: "not_promoted"
  readonly releaseGateRef: typeof KHALA_CODE_QA_GEPA_RELEASE_GATE_REF
  readonly selfPromotionAllowed: false
}

export const KhalaCodeQaGepaGymAdmissionSchema = S.Struct({
  admittedAt: S.String,
  baselineScore: S.Number,
  blockerRefs: S.Array(S.String),
  candidateScore: S.Number,
  evaluatedScenarioIds: S.Array(S.String),
  metric: S.Literal("confirmed_bugs_and_new_coverage_per_action"),
  runRef: S.String,
  schema: S.Literal(KHALA_CODE_QA_GEPA_GYM_ADMISSION_SCHEMA),
  state: S.Literals(["admitted", "blocked"]),
})

export type KhalaCodeQaGepaGymAdmission = {
  readonly schema: typeof KHALA_CODE_QA_GEPA_GYM_ADMISSION_SCHEMA
  readonly admittedAt: string
  readonly baselineScore: number
  readonly blockerRefs: readonly string[]
  readonly candidateScore: number
  readonly evaluatedScenarioIds: readonly string[]
  readonly metric: "confirmed_bugs_and_new_coverage_per_action"
  readonly runRef: string
  readonly state: "admitted" | "blocked"
}

export const KhalaCodeQaGepaExplorePolicyEvidenceSchema = S.Struct({
  actionCount: S.Number,
  confirmedBugRefs: S.Array(S.String),
  newCoverageRefs: S.Array(S.String),
  runtimeMs: S.Number,
  sourceRunId: S.String,
  status: S.Literals(["pass", "fail"]),
})

export type KhalaCodeQaGepaExplorePolicyEvidence = {
  readonly actionCount: number
  readonly confirmedBugRefs: readonly string[]
  readonly newCoverageRefs: readonly string[]
  readonly runtimeMs: number
  readonly sourceRunId: string
  readonly status: "pass" | "fail"
}

export const KhalaCodeQaGepaExplorePolicyCandidateSchema = S.Struct({
  admission: KhalaCodeQaGepaGymAdmissionSchema,
  candidateRef: S.String,
  generatedAt: S.String,
  governance: KhalaCodeQaGepaExplorePolicyGovernanceSchema,
  metrics: S.Struct({
    confirmedBugsPerThousandActions: S.Number,
    newCoveragePerAction: S.Number,
    score: S.Number,
  }),
  metricDefinitions: S.Array(KhalaCodeQaGepaMetricDefinitionSchema),
  parameters: KhalaCodeQaGepaExplorePolicyParametersSchema,
  schema: S.Literal(KHALA_CODE_QA_GEPA_EXPLORE_POLICY_SCHEMA),
  sourceRunIds: S.Array(S.String),
})

export type KhalaCodeQaGepaExplorePolicyCandidate = {
  readonly schema: typeof KHALA_CODE_QA_GEPA_EXPLORE_POLICY_SCHEMA
  readonly admission: KhalaCodeQaGepaGymAdmission
  readonly candidateRef: string
  readonly generatedAt: string
  readonly governance: KhalaCodeQaGepaExplorePolicyGovernance
  readonly metrics: {
    readonly confirmedBugsPerThousandActions: number
    readonly newCoveragePerAction: number
    readonly score: number
  }
  readonly metricDefinitions: readonly KhalaCodeQaGepaMetricDefinition[]
  readonly parameters: KhalaCodeQaGepaExplorePolicyParameters
  readonly sourceRunIds: readonly string[]
}

export const KhalaCodeQaScenarioPortfolioInputSchema = S.Struct({
  confirmedBugRefs: S.optional(S.Array(S.String)),
  newCoverageRefs: S.optional(S.Array(S.String)),
  runtimeMs: S.Number,
  scenarioId: S.String,
  status: S.optional(S.Literals(["pass", "fail", "flaky"])),
})

export type KhalaCodeQaGepaPromotionDecision = {
  readonly promoted: false
  readonly reason: string
}

export type KhalaCodeQaScenarioPortfolioInput = {
  readonly confirmedBugRefs?: readonly string[]
  readonly newCoverageRefs?: readonly string[]
  readonly runtimeMs: number
  readonly scenarioId: string
  readonly status?: "pass" | "fail" | "flaky"
}

export const KhalaCodeQaScenarioPortfolioEntrySchema = S.Struct({
  confirmedBugRefs: S.Array(S.String),
  cumulativeRuntimeMs: S.Number,
  newCoverageRefs: S.Array(S.String),
  position: S.Number,
  pruneReason: S.Literals(["within_budget", "outside_budget", "zero_yield"]),
  runtimeMs: S.Number,
  scenarioId: S.String,
  status: S.Literals(["pass", "fail", "flaky"]),
  tier: S.Literals(["tier_1_pre_push", "tier_2_nightly", "pruned_review"]),
  yieldScore: S.Number,
})

export type KhalaCodeQaScenarioPortfolioEntry = {
  readonly confirmedBugRefs: readonly string[]
  readonly cumulativeRuntimeMs: number
  readonly newCoverageRefs: readonly string[]
  readonly position: number
  readonly pruneReason: "within_budget" | "outside_budget" | "zero_yield"
  readonly runtimeMs: number
  readonly scenarioId: string
  readonly status: "pass" | "fail" | "flaky"
  readonly tier: "tier_1_pre_push" | "tier_2_nightly" | "pruned_review"
  readonly yieldScore: number
}

export const KhalaCodeQaScenarioPortfolioPlanSchema = S.Struct({
  entries: S.Array(KhalaCodeQaScenarioPortfolioEntrySchema),
  generatedAt: S.String,
  nightlyScenarioIds: S.Array(S.String),
  orderedScenarioIds: S.Array(S.String),
  prePushScenarioIds: S.Array(S.String),
  prunedScenarioIds: S.Array(S.String),
  schema: S.Literal(KHALA_CODE_QA_SCENARIO_PORTFOLIO_SCHEMA),
})

export type KhalaCodeQaScenarioPortfolioPlan = {
  readonly schema: typeof KHALA_CODE_QA_SCENARIO_PORTFOLIO_SCHEMA
  readonly entries: readonly KhalaCodeQaScenarioPortfolioEntry[]
  readonly generatedAt: string
  readonly nightlyScenarioIds: readonly string[]
  readonly orderedScenarioIds: readonly string[]
  readonly prePushScenarioIds: readonly string[]
  readonly prunedScenarioIds: readonly string[]
}

export class KhalaCodeQaGepaExplorePolicyGovernanceError extends Error {
  constructor(reason: string) {
    super(`khala_code_qa_gepa_explore_policy_governance_violation: ${reason}`)
    this.name = "KhalaCodeQaGepaExplorePolicyGovernanceError"
  }
}

export const decodeKhalaCodeQaGepaExplorePolicyCandidate = (
  input: unknown,
): KhalaCodeQaGepaExplorePolicyCandidate =>
  S.decodeUnknownSync(KhalaCodeQaGepaExplorePolicyCandidateSchema)(input) as KhalaCodeQaGepaExplorePolicyCandidate

export const decodeKhalaCodeQaGepaMetricDefinitions = (
  input: unknown,
): readonly KhalaCodeQaGepaMetricDefinition[] =>
  S.decodeUnknownSync(S.Array(KhalaCodeQaGepaMetricDefinitionSchema))(input) as readonly KhalaCodeQaGepaMetricDefinition[]

export const decodeKhalaCodeQaScenarioPortfolioPlan = (
  input: unknown,
): KhalaCodeQaScenarioPortfolioPlan =>
  S.decodeUnknownSync(KhalaCodeQaScenarioPortfolioPlanSchema)(input) as KhalaCodeQaScenarioPortfolioPlan

const defaultGovernance = (): KhalaCodeQaGepaExplorePolicyGovernance => ({
  authorityBoundary: "evidence_only",
  autoPromote: false,
  live: false,
  offlineOnly: true,
  promotionState: "not_promoted",
  releaseGateRef: KHALA_CODE_QA_GEPA_RELEASE_GATE_REF,
  selfPromotionAllowed: false,
})

export const defaultKhalaCodeQaGepaExplorePolicyParameters = (): KhalaCodeQaGepaExplorePolicyParameters => ({
  actionSelectionHeuristic: "frontier-weighted-seeded-choice",
  coverageNoveltyWeight: 1,
  confirmedBugWeight: 4,
  explorationEpsilon: 0.15,
  frontierWeight: 3,
  goalPrompt: "Explore Khala Code offline for new coverage and confirmed bugs; distill replayable scenarios only.",
  maxActionsPerScenario: 64,
  runtimeCostWeight: 1,
})

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, Number.isFinite(value) ? value : min))

const normalizeParameters = (
  input: Partial<KhalaCodeQaGepaExplorePolicyParameters> = {},
): KhalaCodeQaGepaExplorePolicyParameters => {
  const defaults = defaultKhalaCodeQaGepaExplorePolicyParameters()
  return {
    actionSelectionHeuristic: nonBlank(input.actionSelectionHeuristic, defaults.actionSelectionHeuristic),
    coverageNoveltyWeight: clamp(input.coverageNoveltyWeight ?? defaults.coverageNoveltyWeight, 0, 100),
    confirmedBugWeight: clamp(input.confirmedBugWeight ?? defaults.confirmedBugWeight, 0, 100),
    explorationEpsilon: clamp(input.explorationEpsilon ?? defaults.explorationEpsilon, 0, 1),
    frontierWeight: clamp(input.frontierWeight ?? defaults.frontierWeight, 0, 100),
    goalPrompt: nonBlank(input.goalPrompt, defaults.goalPrompt),
    maxActionsPerScenario: Math.max(1, Math.trunc(input.maxActionsPerScenario ?? defaults.maxActionsPerScenario)),
    runtimeCostWeight: clamp(input.runtimeCostWeight ?? defaults.runtimeCostWeight, 0, 100),
  }
}

export const assertKhalaCodeQaGepaExplorePolicyGoverned = (
  candidate: Pick<KhalaCodeQaGepaExplorePolicyCandidate, "admission" | "governance">,
): void => {
  const governance = candidate.governance
  if (governance.authorityBoundary !== "evidence_only") {
    throw new KhalaCodeQaGepaExplorePolicyGovernanceError("authorityBoundary must be evidence_only")
  }
  if (governance.offlineOnly !== true) {
    throw new KhalaCodeQaGepaExplorePolicyGovernanceError("GEPA explore policy candidates must be offline-only")
  }
  if (governance.selfPromotionAllowed !== false || governance.autoPromote !== false) {
    throw new KhalaCodeQaGepaExplorePolicyGovernanceError("GEPA explore policy candidates must never self-promote")
  }
  if (governance.live !== false || governance.promotionState !== "not_promoted") {
    throw new KhalaCodeQaGepaExplorePolicyGovernanceError("GEPA explore policy candidates must not be live")
  }
  if (governance.releaseGateRef !== KHALA_CODE_QA_GEPA_RELEASE_GATE_REF) {
    throw new KhalaCodeQaGepaExplorePolicyGovernanceError("unexpected release gate ref")
  }
  if (candidate.admission.state !== "admitted") {
    throw new KhalaCodeQaGepaExplorePolicyGovernanceError("candidate must be Gym-admitted before it can drive offline exploration")
  }
}

export const admitKhalaCodeQaGepaExplorePolicyToGym = (input: {
  readonly admittedAt?: string
  readonly baselineScore: number
  readonly candidateScore: number
  readonly evaluatedScenarioIds: readonly string[]
  readonly offline: boolean
  readonly runRef: string
}): KhalaCodeQaGepaGymAdmission => {
  const evaluatedScenarioIds = dedupe(input.evaluatedScenarioIds)
  const runRef = input.runRef.trim()
  const blockerRefs = [
    ...(input.offline ? [] : ["blocker.khala_qa_gepa.live_input"]),
    ...(runRef === "" ? ["blocker.khala_qa_gepa.missing_gym_run_ref"] : []),
    ...(evaluatedScenarioIds.length === 0 ? ["blocker.khala_qa_gepa.no_evaluated_scenarios"] : []),
    ...(input.candidateScore > input.baselineScore ? [] : ["blocker.khala_qa_gepa.metric_not_improved"]),
  ]
  return {
    admittedAt: input.admittedAt ?? new Date().toISOString(),
    baselineScore: roundMetric(input.baselineScore),
    blockerRefs,
    candidateScore: roundMetric(input.candidateScore),
    evaluatedScenarioIds,
    metric: "confirmed_bugs_and_new_coverage_per_action",
    runRef,
    schema: KHALA_CODE_QA_GEPA_GYM_ADMISSION_SCHEMA,
    state: blockerRefs.length === 0 ? "admitted" : "blocked",
  }
}

export const proposeKhalaCodeQaGepaExplorePolicyCandidate = (input: {
  readonly admission: KhalaCodeQaGepaGymAdmission
  readonly evidence: readonly KhalaCodeQaGepaExplorePolicyEvidence[]
  readonly generatedAt?: string
  readonly parameters?: Partial<KhalaCodeQaGepaExplorePolicyParameters>
}): KhalaCodeQaGepaExplorePolicyCandidate => {
  const parameters = normalizeParameters(input.parameters)
  const actionCount = Math.max(1, input.evidence.reduce((sum, entry) => sum + Math.max(0, entry.actionCount), 0))
  const confirmedBugRefs = dedupe(input.evidence.flatMap((entry) => entry.confirmedBugRefs))
  const newCoverageRefs = dedupe(input.evidence.flatMap((entry) => entry.newCoverageRefs))
  const confirmedBugsPerThousandActions = confirmedBugRefs.length * 1000 / actionCount
  const newCoveragePerAction = newCoverageRefs.length / actionCount
  const score = scoreExplorePolicyMetrics({
    confirmedBugsPerThousandActions,
    newCoveragePerAction,
    parameters,
  })
  const roundedScore = roundMetric(score)
  if (input.admission.state === "admitted" && input.admission.candidateScore !== roundedScore) {
    throw new KhalaCodeQaGepaExplorePolicyGovernanceError(
      "Gym admission candidateScore must match the candidate metrics score",
    )
  }
  const candidateSeed = stableStringify({
    admissionRunRef: input.admission.runRef,
    parameters,
    sourceRunIds: input.evidence.map((entry) => entry.sourceRunId).sort(),
  })
  const candidate: KhalaCodeQaGepaExplorePolicyCandidate = {
    admission: input.admission,
    candidateRef: `khala_code_qa.gepa_explore_policy.${fnv1a32(candidateSeed).toString(16).padStart(8, "0")}`,
    generatedAt: input.generatedAt ?? new Date().toISOString(),
    governance: defaultGovernance(),
    metrics: {
      confirmedBugsPerThousandActions: roundMetric(confirmedBugsPerThousandActions),
      newCoveragePerAction: roundMetric(newCoveragePerAction),
      score: roundedScore,
    },
    metricDefinitions: KHALA_CODE_QA_GEPA_EXPLORE_POLICY_METRIC_DEFINITIONS,
    parameters,
    schema: KHALA_CODE_QA_GEPA_EXPLORE_POLICY_SCHEMA,
    sourceRunIds: dedupe(input.evidence.map((entry) => entry.sourceRunId)),
  }
  assertKhalaCodeQaGepaExplorePolicyGoverned(candidate)
  return candidate
}

export const evaluateKhalaCodeQaGepaExplorePolicyAutoPromotion = (
  candidate: KhalaCodeQaGepaExplorePolicyCandidate,
): KhalaCodeQaGepaPromotionDecision => {
  assertKhalaCodeQaGepaExplorePolicyGoverned(candidate)
  return {
    promoted: false,
    reason: `auto_promotion_disabled: ${candidate.candidateRef} remains evidence-only until ${candidate.governance.releaseGateRef} approves a separate promotion.`,
  }
}

export const makeKhalaCodeQaGepaExplorePolicyBrain = (
  candidate: KhalaCodeQaGepaExplorePolicyCandidate,
): KhalaCodeQaExplorerBrain => {
  assertKhalaCodeQaGepaExplorePolicyGoverned(candidate)
  return {
    nextAction: (context) => {
      const decision = chooseKhalaCodeQaGepaExplorePolicyAction({
        actionSpace: context.actionSpace,
        frontierRefs: [
          ...context.frontier.missing.rpcMethods.map((item) => `rpcMethods:${item}`),
          ...context.frontier.missing.hotbarPanels.map((item) => `hotbarPanels:${item}`),
          ...context.frontier.missing.slashCommands.map((item) => `slashCommands:${item}`),
          ...context.frontier.missing.selectors.map((item) => `selectors:${item}`),
          ...context.frontier.missing.settingsKeys.map((item) => `settingsKeys:${item}`),
          ...context.frontier.missing.approvalDecisionKinds.map((item) => `approvalDecisionKinds:${item}`),
          ...context.frontier.missing.threadItemVariants.map((item) => `threadItemVariants:${item}`),
        ],
        parameters: candidate.parameters,
        prngState: context.prngState,
        stepIndex: context.stepIndex,
      })
      return Effect.succeed(decision)
    },
    tier: "deterministic_fixture",
  }
}

export const chooseKhalaCodeQaGepaExplorePolicyAction = (input: {
  readonly actionSpace: readonly KhalaCodeQaAction[]
  readonly frontierRefs: readonly string[]
  readonly parameters: KhalaCodeQaGepaExplorePolicyParameters
  readonly prngState: number
  readonly stepIndex: number
}): KhalaCodeQaExplorerBrainDecision => {
  if (input.actionSpace.length === 0) {
    throw new KhalaCodeQaGepaExplorePolicyGovernanceError("action space must not be empty")
  }
  if (prngUnit(input.prngState) < input.parameters.explorationEpsilon) {
    const action = pick(input.actionSpace, input.prngState + input.stepIndex)
    return {
      action,
      rationale: "gepa-policy:epsilon-seeded-exploration",
      tier: "deterministic_fixture",
    }
  }

  const frontier = new Set(input.frontierRefs)
  const scored = input.actionSpace.map((action) => {
    const matchedRefs = khalaCodeQaActionCoverageRefs(action).filter((ref) => frontier.has(ref))
    const score = matchedRefs.length === 0
      ? 0
      : input.parameters.frontierWeight + matchedRefs.length * input.parameters.coverageNoveltyWeight
    return { action, matchedRefs, score }
  })
  const bestScore = Math.max(...scored.map((entry) => entry.score))
  if (bestScore <= 0) {
    const action = pick(input.actionSpace, input.prngState + input.stepIndex)
    return {
      action,
      rationale: "gepa-policy:frontier-exhausted-seeded-fallback",
      tier: "deterministic_fixture",
    }
  }
  const best = scored.filter((entry) => entry.score === bestScore)
  const selected = pick(best, input.prngState + input.stepIndex)
  return {
    action: selected.action,
    frontierRef: selected.matchedRefs[0],
    rationale: `gepa-policy:${input.parameters.actionSelectionHeuristic}:${selected.matchedRefs.join(",")}`,
    tier: "deterministic_fixture",
  }
}

export const rankKhalaCodeQaScenarioPortfolioByYield = (input: {
  readonly generatedAt?: string
  readonly nightlyBudgetMs: number
  readonly prePushBudgetMs: number
  readonly scenarios: readonly KhalaCodeQaScenarioPortfolioInput[]
}): KhalaCodeQaScenarioPortfolioPlan => {
  const ordered = input.scenarios
    .map((scenario) => ({
      confirmedBugRefs: dedupe(scenario.confirmedBugRefs ?? []),
      newCoverageRefs: dedupe(scenario.newCoverageRefs ?? []),
      runtimeMs: Math.max(1, Math.trunc(scenario.runtimeMs)),
      scenarioId: scenario.scenarioId,
      status: scenario.status ?? "pass",
    }))
    .map((scenario) => ({
      ...scenario,
      yieldScore: scenarioYieldScore(scenario),
    }))
    .sort((left, right) =>
      right.yieldScore - left.yieldScore ||
      coverageYieldScore(right) - coverageYieldScore(left) ||
      left.runtimeMs - right.runtimeMs ||
      left.scenarioId.localeCompare(right.scenarioId)
    )

  let cumulativeRuntimeMs = 0
  const entries = ordered.map((scenario, index): KhalaCodeQaScenarioPortfolioEntry => {
    cumulativeRuntimeMs += scenario.runtimeMs
    const zeroYield = scenario.yieldScore <= 0
    const tier = zeroYield
      ? "pruned_review"
      : cumulativeRuntimeMs <= input.prePushBudgetMs
        ? "tier_1_pre_push"
        : cumulativeRuntimeMs <= input.nightlyBudgetMs
          ? "tier_2_nightly"
          : "pruned_review"
    return {
      confirmedBugRefs: scenario.confirmedBugRefs,
      cumulativeRuntimeMs,
      newCoverageRefs: scenario.newCoverageRefs,
      position: index + 1,
      pruneReason: tier === "pruned_review"
        ? zeroYield ? "zero_yield" : "outside_budget"
        : "within_budget",
      runtimeMs: scenario.runtimeMs,
      scenarioId: scenario.scenarioId,
      status: scenario.status,
      tier,
      yieldScore: roundMetric(scenario.yieldScore),
    }
  })
  return {
    entries,
    generatedAt: input.generatedAt ?? new Date().toISOString(),
    nightlyScenarioIds: entries
      .filter((entry) => entry.tier === "tier_1_pre_push" || entry.tier === "tier_2_nightly")
      .map((entry) => entry.scenarioId),
    orderedScenarioIds: entries.map((entry) => entry.scenarioId),
    prePushScenarioIds: entries
      .filter((entry) => entry.tier === "tier_1_pre_push")
      .map((entry) => entry.scenarioId),
    prunedScenarioIds: entries
      .filter((entry) => entry.tier === "pruned_review")
      .map((entry) => entry.scenarioId),
    schema: KHALA_CODE_QA_SCENARIO_PORTFOLIO_SCHEMA,
  }
}

const scenarioYieldScore = (scenario: {
  readonly confirmedBugRefs: readonly string[]
  readonly newCoverageRefs: readonly string[]
  readonly runtimeMs: number
}): number => {
  return scenario.confirmedBugRefs.length * 1000 / scenario.runtimeMs
}

const coverageYieldScore = (scenario: {
  readonly newCoverageRefs: readonly string[]
  readonly runtimeMs: number
}): number => {
  return scenario.newCoverageRefs.length / scenario.runtimeMs
}

const scoreExplorePolicyMetrics = (input: {
  readonly confirmedBugsPerThousandActions: number
  readonly newCoveragePerAction: number
  readonly parameters: KhalaCodeQaGepaExplorePolicyParameters
}): number =>
  input.confirmedBugsPerThousandActions * input.parameters.confirmedBugWeight +
  input.newCoveragePerAction * input.parameters.coverageNoveltyWeight

const nonBlank = (value: string | undefined, fallback: string): string =>
  value === undefined || value.trim() === "" ? fallback : value.trim()

const roundMetric = (value: number): number =>
  Math.round(value * 1_000_000) / 1_000_000

const dedupe = (values: readonly string[]): readonly string[] =>
  [...new Set(values.filter((value) => value.trim() !== ""))].sort()

const pick = <A>(values: readonly A[], state: number): A =>
  values[Math.abs(state) % values.length] as A

const prngUnit = (state: number): number =>
  (state >>> 0) / 0x100000000

const fnv1a32 = (value: string): number => {
  let hash = 0x811c9dc5
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193)
  }
  return hash >>> 0
}

const stableStringify = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`
  if (value === null || typeof value !== "object") return JSON.stringify(value)
  const record = value as Record<string, unknown>
  return `{${Object.keys(record).sort().map((key) => `${JSON.stringify(key)}:${stableStringify(record[key])}`).join(",")}}`
}
