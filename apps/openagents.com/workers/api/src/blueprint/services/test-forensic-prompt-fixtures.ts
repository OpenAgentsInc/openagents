/**
 * Shared fixtures for the forensic prompt compiler, governance store, and
 * governance route tests. They are deliberately synthetic: nothing here is a
 * campaign result, and no value in this file is evidence about any real
 * forensic target.
 */
import {
  FORENSIC_FINDING_VERSION,
  FORENSIC_HYPOTHESIS_VERSION,
  FORENSIC_PROMPT_ARTIFACT_VERSION,
  FORENSIC_SCORECARD_VERSION,
  FORENSIC_WORKER_PLACEMENT_VERSION,
  forensicPromptArtifactDigest,
  forensicSha256Digest,
  strictDecode,
  ForensicScorecardSchema,
  ForensicWorkerPlacementSchema,
  type ForensicPromptArtifact,
  type ForensicPromptIr,
} from "@openagentsinc/forensic-contract";

import type { BlueprintReleaseGate } from "../schemas/release-gate";
import { FORENSIC_PROMPT_PARETO_AXES } from "../schemas/forensic-prompt-optimization";
import type {
  ForensicPromptActiveTransition,
  ForensicPromptDatasetRevision,
  ForensicPromptEvaluationEvidence,
  ForensicPromptGovernanceState,
  ForensicPromptMetricFreeze,
  ForensicPromptParetoAxis,
} from "../schemas/forensic-prompt-optimization";
import { compileForensicPromptCandidates } from "./forensic-prompt-compiler";

export const digest = (character: string) => `sha256:${character.repeat(64)}`;

export const promptIr: ForensicPromptIr = {
  role: "Find source-grounded security invariant violations.",
  threatModel: "Trace attacker-controlled and entropy-sensitive inputs.",
  vulnerabilityClasses: ["entropy downgrade"],
  securityInvariants: ["Wallet secrets require certified entropy."],
  evidenceRequirements: ["Cite every causal link."],
  dependencyExplorationPolicy: "Inspect every mounted dependency needed by the causal path.",
  uncertaintyPolicy: "Use a typed hypothesis when evidence is incomplete.",
  toolPolicyRefs: ["tool.source.read"],
  findingSchemaRef: FORENSIC_FINDING_VERSION,
  hypothesisSchemaRef: FORENSIC_HYPOTHESIS_VERSION,
  pocPolicy: "Prefer deterministic fixture-bound reproduction.",
  severityPolicy: "Severity follows demonstrated impact.",
  contextPolicy: "Prioritize entropy-sensitive paths.",
  budgetPolicyRef: "budget.admitted.forensic.v1",
};

export const sourceArtifact = (): ForensicPromptArtifact => {
  const digestInput = {
    promptIr,
    exampleRefs: ["example.train.1"],
    parameterRefs: ["parameter.reasoning.high"],
    datasetRevisionRef: "dataset.forensic.visible.v1",
    compatibilityRefs: ["compatibility.loupe.v1"],
  };
  return {
    schema: FORENSIC_PROMPT_ARTIFACT_VERSION,
    promptArtifactRef: "prompt.forensic.baseline.v1",
    ...digestInput,
    canonicalDigest: forensicPromptArtifactDigest(digestInput),
    createdAt: "2026-08-01T16:00:00.000Z",
  };
};

export const datasets = (): ReadonlyArray<ForensicPromptDatasetRevision> => [
  {
    datasetRef: "dataset.train.v1",
    digest: digest("1"),
    exampleRefs: ["example.train.1"],
    optimizerVisibility: "optimizer_visible",
    split: "train",
  },
  {
    datasetRef: "dataset.development.v1",
    digest: digest("2"),
    exampleRefs: ["example.development.1"],
    optimizerVisibility: "optimizer_visible",
    split: "development",
  },
  {
    datasetRef: "dataset.holdout.v1",
    digest: digest("3"),
    exampleRefs: ["example.holdout.secret"],
    optimizerVisibility: "evaluator_only",
    split: "holdout",
  },
  {
    datasetRef: "dataset.clean-holdout.v1",
    digest: digest("4"),
    exampleRefs: ["example.clean-holdout.secret"],
    optimizerVisibility: "evaluator_only",
    split: "clean_holdout",
  },
];

/**
 * Every axis binds a metric the frozen forensic metric registry actually
 * defines. Two of these — `metric.causal_chain_coverage.v1` and
 * `metric.cost_to_identification.v1` — are defined in the registry but are not
 * emitted per run by `rebuildForensicScorecard`, which is recorded as a live
 * blocker in the governance document rather than papered over here.
 */
const AXIS_METRIC_REFS: Readonly<Record<ForensicPromptParetoAxis, string>> = {
  hit_rate: "metric.qualified_hit.v1",
  causal_coverage: "metric.causal_chain_coverage.v1",
  time: "metric.analysis_time_to_identification.v1",
  tokens: "metric.tokens_to_identification.v1",
  cost: "metric.cost_to_identification.v1",
  false_positives: "metric.control_false_positive.v1",
  reviewer_load: "metric.reviewer_minutes_per_qualified_finding.v1",
};

export const metricRef = (axis: ForensicPromptParetoAxis) => AXIS_METRIC_REFS[axis];

export const paretoAxes = FORENSIC_PROMPT_PARETO_AXES.map((axis) => ({
  axis,
  direction: ["hit_rate", "causal_coverage"].includes(axis)
    ? ("maximize" as const)
    : ("minimize" as const),
  metricRef: metricRef(axis),
}));

export const metricFreeze: ForensicPromptMetricFreeze = {
  censoringDefinitionDigest: digest("5"),
  eligibilityDefinitionDigest: digest("6"),
  frozenAt: "2026-08-01T16:00:00.000Z",
  metricDefinitionRevisionDigest: digest("7"),
  paretoAxes,
  t5DefinitionDigest: digest("8"),
};

export const compilerInput = () => ({
  candidateInputs: [
    {
      exampleRefs: ["example.train.1"],
      parameterRefs: ["parameter.reasoning.high"],
      promptIr: { ...promptIr, contextPolicy: "Start with entropy-sensitive paths." },
      summaryRef: "summary.forensic.entropy-first",
    },
  ],
  compilerRef: "compiler.forensic.offline.v1",
  datasets: datasets(),
  generatedAt: "2026-08-01T16:10:00.000Z",
  metricFreeze,
  optimizerConfiguration: {
    configurationDigest: digest("9"),
    generatorIdentityRef: "identity.optimizer.generator",
    integrationReceiptRefs: [],
    kind: "instruction_grid" as const,
    maxCandidates: 8,
  },
  optimizerRunRef: "optimizer-run.forensic.1",
  retainedFailureRefs: ["run.failure.coldcard.1", "finding.failure.coldcard.1"],
  sourceArtifact: sourceArtifact(),
});

/** Baseline measurements for every frozen Pareto axis. */
export const BASELINE_AXIS_VALUES: Readonly<Record<ForensicPromptParetoAxis, number>> = {
  hit_rate: 0.4,
  causal_coverage: 0.5,
  time: 900_000,
  tokens: 120_000,
  cost: 40_000,
  false_positives: 3,
  reviewer_load: 12,
};

/** A candidate that is better on discovery and no worse on any cost axis. */
export const DOMINATING_AXIS_VALUES: Readonly<Record<ForensicPromptParetoAxis, number>> = {
  hit_rate: 0.7,
  causal_coverage: 0.6,
  time: 800_000,
  tokens: 120_000,
  cost: 40_000,
  false_positives: 2,
  reviewer_load: 12,
};

export const metricValues = (
  values: Partial<Record<ForensicPromptParetoAxis, number>>,
): ReadonlyArray<unknown> =>
  FORENSIC_PROMPT_PARETO_AXES.filter((axis) => values[axis] !== undefined).map((axis) => ({
    metricRef: metricRef(axis),
    numericValue: values[axis],
    exactness: "exact",
    sourceEventRefs: [`event.metric.${axis}`],
    sourceReceiptRefs: [],
  }));

export const scorecard = (
  datasetRevisionDigest: string,
  candidateDigest: string,
  split: "holdout" | "clean_holdout",
  holdoutHit = true,
  values: Partial<Record<ForensicPromptParetoAxis, number>> = {},
  scorecardRef = `scorecard.${split}.v1`,
) =>
  strictDecode(ForensicScorecardSchema, {
    schema: FORENSIC_SCORECARD_VERSION,
    scorecardRef,
    datasetRevisionDigest,
    metricDefinitionRevisionDigest: metricFreeze.metricDefinitionRevisionDigest,
    evaluatorRevisionDigest: digest("a"),
    candidateDigest,
    hardGates: [
      {
        gateRef: `gate.${split}.complete`,
        passed: true,
        evidenceRefs: [`evidence.${split}.complete`],
        blockerRefs: [],
      },
    ],
    runs: [
      {
        runDigest: split === "holdout" ? digest("b") : digest("c"),
        armRef: `arm.${split}.1`,
        datasetSplit: split,
        population: split === "holdout" ? "vulnerable" : "clean_control",
        coverageStatus: "complete",
        outcome: split === "holdout" ? (holdoutHit ? "hit" : "miss") : "not_eligible",
        eligibleForIdentification: split === "holdout",
        censored: split === "holdout" && !holdoutHit,
        miss: split === "holdout" && !holdoutHit,
        ...(split === "holdout" && !holdoutHit
          ? { censorAt: { milliseconds: 60_000, exactness: "exact" } }
          : {}),
        spentUsage: { exactness: "unavailable", unavailableReasonRef: "usage.unavailable.test" },
        ...(split === "holdout" && holdoutHit
          ? {
              qualifiedFindingEventRef: "event.finding.holdout.1",
              qualifiedFindingObservedAt: "2026-08-01T16:20:00.000Z",
            }
          : {}),
        values: metricValues(values),
        failureRefs: [],
      },
    ],
    populationGroups: [
      {
        datasetSplit: split,
        population: split === "holdout" ? "vulnerable" : "clean_control",
        runCount: 1,
        hitCount: split === "holdout" && holdoutHit ? 1 : 0,
        missCount: split === "holdout" && !holdoutHit ? 1 : 0,
        censorCount: split === "holdout" && !holdoutHit ? 1 : 0,
      },
    ],
    distributionRefs: [],
    censorCount: split === "holdout" && !holdoutHit ? 1 : 0,
    missCount: split === "holdout" && !holdoutHit ? 1 : 0,
    confidenceIntervalRefs: [],
    cost: { exactness: "unavailable", unavailableReasonRef: "cost.unavailable.test" },
    eventDigest: digest("d"),
    receiptDigest: digest("e"),
    generatedAt: "2026-08-01T16:30:00.000Z",
  });

export const releaseGate = (targetRef: string): BlueprintReleaseGate => ({
  decidedByRef: "identity.operator.release",
  decision: "approved",
  decisionReasonRef: "reason.holdout-improved",
  fixturePassState: "passed",
  fixtureRefs: ["fixture.holdout", "fixture.clean-holdout"],
  id: "release-gate.forensic.prompt.1",
  policyState: "compliant",
  receiptRefs: ["receipt.holdout.evaluation", "receipt.rollback.ready"],
  reviewState: "approved",
  rollbackPosture: "verified",
  scorecardRef: "scorecard.holdout.v1",
  selfPromotionAttempt: false,
  targetKind: "module_version",
  targetRef,
});

export const placement = (generation: number) =>
  strictDecode(ForensicWorkerPlacementSchema, {
    schema: FORENSIC_WORKER_PLACEMENT_VERSION,
    placementRef: `placement.openagents-cloud.${generation}`,
    ownerRef: "owner.forensic.operator",
    tenantRef: "tenant.openagents",
    workUnitRef: `work-unit.holdout.${generation}`,
    sandboxRef: `sandbox.holdout.${generation}`,
    attachmentGeneration: 1,
    resourceGeneration: generation,
    targetClass: "openagents_managed",
    provider: "google_cloud",
    adapterRef: "adapter.oa-codex-control.gce.v1",
    isolation: "gce_vm",
    regionRef: "region.google-cloud.us-central1",
    imageDigest: forensicSha256Digest(`worker-image-${generation}`),
    profileDigest: forensicSha256Digest(`worker-profile-${generation}`),
    networkPolicyRef: "network-policy-ref://openagents/managed-sandbox/broker-only-v1",
    leaseRef: `lease.holdout.${generation}`,
    budgetRef: "budget.forensic.holdout.v1",
    capabilityRefs: [`capability.holdout.${generation}`],
    state: "worker_ready",
    admissionReceiptRef: `receipt.admission.${generation}`,
    readinessReceiptRef: `receipt.readiness.${generation}`,
    updatedAt: "2026-08-01T16:19:00.000Z",
  });

export const evaluationEvidence = (candidateDigest: string): ForensicPromptEvaluationEvidence => ({
  candidateDigest,
  evaluationRef: "evaluation.forensic.candidate.1",
  mechanicalEvidence: (
    ["scorecard_rebuilt", "source_state_resolved", "worker_lifecycle_resolved"] as const
  ).map((evidenceType, index) => ({
    evidenceRef: `evidence.mechanical.${index + 1}`,
    evidenceType,
    observedAt: "2026-08-01T16:31:00.000Z",
    receiptDigest: forensicSha256Digest(`mechanical-${index + 1}`),
    scorecardRef: "scorecard.holdout.v1",
  })),
  recordedAt: "2026-08-01T16:32:00.000Z",
  schema: "openagents.blueprint.forensic_prompt_evaluation_evidence.v1",
  sourceStates: [
    {
      datasetDigest: digest("3"),
      materializationReceiptRef: "receipt.materialization.holdout",
      observedAt: "2026-08-01T16:18:00.000Z",
      sourceDigest: forensicSha256Digest("source-holdout"),
      sourceStateRef: "source-state.holdout.1",
    },
    {
      datasetDigest: digest("4"),
      materializationReceiptRef: "receipt.materialization.clean",
      observedAt: "2026-08-01T16:18:00.000Z",
      sourceDigest: forensicSha256Digest("source-clean"),
      sourceStateRef: "source-state.clean.1",
    },
  ],
  workerPlacements: [placement(1), placement(2)],
});

export const evaluationInputFor = (
  candidate: ReturnType<typeof compileForensicPromptCandidates>["candidates"][number],
  candidateAxisValues: Partial<
    Record<ForensicPromptParetoAxis, number>
  > = DOMINATING_AXIS_VALUES,
) => ({
  baselineHoldoutScorecard: scorecard(
    candidate.holdoutDatasetDigest,
    digest("0"),
    "holdout" as const,
    false,
    BASELINE_AXIS_VALUES,
    "scorecard.holdout.baseline.v1",
  ),
  candidate,
  cleanHoldoutScorecard: scorecard(
    candidate.cleanHoldoutDatasetDigest,
    candidate.candidateDigest,
    "clean_holdout" as const,
  ),
  evaluationRef: "evaluation.forensic.candidate.1",
  evaluatorIdentityRef: "identity.release.evaluator",
  evidence: evaluationEvidence(candidate.candidateDigest),
  holdoutScorecard: scorecard(
    candidate.holdoutDatasetDigest,
    candidate.candidateDigest,
    "holdout" as const,
    true,
    candidateAxisValues,
  ),
  metricFreeze,
});

export const genesisState = (): ForensicPromptGovernanceState => ({
  activePromptDigest: null,
  history: [],
  ownerRef: "owner.forensic.operator",
  revision: 0,
  schema: "openagents.blueprint.forensic_prompt_governance_state.v1",
});

export const stateAfter = (
  ...transitions: ReadonlyArray<ForensicPromptActiveTransition>
): ForensicPromptGovernanceState => ({
  activePromptDigest: transitions.at(-1)?.activePromptDigest ?? null,
  history: transitions,
  ownerRef: "owner.forensic.operator",
  revision: transitions.length,
  schema: "openagents.blueprint.forensic_prompt_governance_state.v1",
});
