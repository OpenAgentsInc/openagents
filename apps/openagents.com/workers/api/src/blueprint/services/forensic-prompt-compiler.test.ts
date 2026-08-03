import { describe, expect, test } from "vitest";

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
import {
  compileForensicPromptCandidates,
  forensicPromptTransitionDigestMatches,
  promoteForensicPrompt,
  rollbackForensicPrompt,
  validateForensicPromptEvaluation,
} from "./forensic-prompt-compiler";

const digest = (character: string) => `sha256:${character.repeat(64)}`;

const promptIr: ForensicPromptIr = {
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

const sourceArtifact = (): ForensicPromptArtifact => {
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

const datasets = (): ReadonlyArray<ForensicPromptDatasetRevision> => [
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

const metricRef = (axis: ForensicPromptParetoAxis) => `metric.forensic.${axis}`;

const paretoAxes = FORENSIC_PROMPT_PARETO_AXES.map((axis) => ({
  axis,
  direction: ["hit_rate", "causal_coverage"].includes(axis)
    ? ("maximize" as const)
    : ("minimize" as const),
  metricRef: metricRef(axis),
}));

const metricFreeze: ForensicPromptMetricFreeze = {
  censoringDefinitionDigest: digest("5"),
  eligibilityDefinitionDigest: digest("6"),
  frozenAt: "2026-08-01T16:00:00.000Z",
  metricDefinitionRevisionDigest: digest("7"),
  paretoAxes,
  t5DefinitionDigest: digest("8"),
};

const compilerInput = () => ({
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
const BASELINE_AXIS_VALUES: Readonly<Record<ForensicPromptParetoAxis, number>> = {
  hit_rate: 0.4,
  causal_coverage: 0.5,
  time: 900_000,
  tokens: 120_000,
  cost: 40_000,
  false_positives: 3,
  reviewer_load: 12,
};

/** A candidate that is better on discovery and no worse on any cost axis. */
const DOMINATING_AXIS_VALUES: Readonly<Record<ForensicPromptParetoAxis, number>> = {
  hit_rate: 0.7,
  causal_coverage: 0.6,
  time: 800_000,
  tokens: 120_000,
  cost: 40_000,
  false_positives: 2,
  reviewer_load: 12,
};

const metricValues = (
  values: Partial<Record<ForensicPromptParetoAxis, number>>,
): ReadonlyArray<unknown> =>
  FORENSIC_PROMPT_PARETO_AXES.filter((axis) => values[axis] !== undefined).map((axis) => ({
    metricRef: metricRef(axis),
    numericValue: values[axis],
    exactness: "exact",
    sourceEventRefs: [`event.metric.${axis}`],
    sourceReceiptRefs: [],
  }));

const scorecard = (
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

const releaseGate = (targetRef: string): BlueprintReleaseGate => ({
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

const placement = (generation: number) =>
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

const evaluationEvidence = (candidateDigest: string): ForensicPromptEvaluationEvidence => ({
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

const evaluationInputFor = (
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

const genesisState = (): ForensicPromptGovernanceState => ({
  activePromptDigest: null,
  history: [],
  ownerRef: "owner.forensic.operator",
  revision: 0,
  schema: "openagents.blueprint.forensic_prompt_governance_state.v1",
});

const stateAfter = (
  ...transitions: ReadonlyArray<ForensicPromptActiveTransition>
): ForensicPromptGovernanceState => ({
  activePromptDigest: transitions.at(-1)?.activePromptDigest ?? null,
  history: transitions,
  ownerRef: "owner.forensic.operator",
  revision: transitions.length,
  schema: "openagents.blueprint.forensic_prompt_governance_state.v1",
});

describe("forensic prompt compiler and Blueprint governance", () => {
  test("compiles immutable evidence-only candidates whose identity covers every input class", () => {
    const run = compileForensicPromptCandidates(compilerInput());
    const candidate = run.candidates[0];
    expect(candidate?.blueprintModuleVersion).toMatchObject({
      moduleKind: "optimizer_candidate",
      releaseState: "unpromoted",
      status: "candidate",
    });
    expect(candidate?.promptArtifact.parentPromptArtifactRef).toBe(
      sourceArtifact().promptArtifactRef,
    );
    expect(Object.isFrozen(run)).toBe(true);

    const changed = compilerInput();
    changed.optimizerConfiguration.configurationDigest = digest("f");
    expect(compileForensicPromptCandidates(changed).candidates[0]?.candidateDigest).not.toBe(
      candidate?.candidateDigest,
    );
  });

  test("keeps holdouts evaluator-only and freezes T5, censoring, eligibility, and metrics", () => {
    const holdoutRead = compilerInput();
    holdoutRead.candidateInputs[0]!.exampleRefs = ["example.holdout.secret"];
    expect(() => compileForensicPromptCandidates(holdoutRead)).toThrow(/holdout examples/);

    const original = compilerInput();
    const overlap = {
      ...original,
      datasets: original.datasets.map((dataset, index) =>
        index === 0 ? { ...dataset, exampleRefs: ["example.holdout.secret"] } : dataset,
      ),
    };
    expect(() => compileForensicPromptCandidates(overlap)).toThrow(/overlap/);
  });

  test("freezes every Pareto axis before any result is known", () => {
    const input = compilerInput();
    const partial = {
      ...input,
      metricFreeze: {
        ...metricFreeze,
        paretoAxes: paretoAxes.slice(0, 3),
      },
    };
    expect(() => compileForensicPromptCandidates(partial)).toThrow(/every Pareto axis/);
  });

  test("never presents rule-based refinement as DSPy or GEPA", () => {
    const input = compilerInput();
    const mislabeled = {
      ...input,
      optimizerConfiguration: {
        ...input.optimizerConfiguration,
        kind: "gepa" as const,
      },
    };
    expect(() => compileForensicPromptCandidates(mislabeled)).toThrow(/integration receipt/);
  });

  test("requires independent fresh-worker holdout evaluation and blocks clean regressions", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluationInput = evaluationInputFor(candidate);
    const evaluation = validateForensicPromptEvaluation(evaluationInput);
    expect(evaluation.evaluatorIdentityRef).not.toBe(
      evaluationInput.candidate.blueprintModuleVersion.provenance.createdByRef,
    );
    expect(evaluation.evidenceReceiptDigest).toBe(
      forensicSha256Digest(evaluationInput.evidence),
    );
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evaluatorIdentityRef:
          evaluationInput.candidate.blueprintModuleVersion.provenance.createdByRef,
      }),
    ).toThrow(/distinct identities/);
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        metricFreeze: {
          ...metricFreeze,
          t5DefinitionDigest: digest("9"),
        },
      }),
    ).toThrow(/frozen metric definitions/);
  });

  test("resolves worker and source evidence as typed records rather than caller strings", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluationInput = evaluationInputFor(candidate);
    const evidence = evaluationInput.evidence;

    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: { ...evidence, workerPlacements: [evidence.workerPlacements[0]!] },
      }),
    ).toThrow(/fresh OpenAgents Cloud workers/);

    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: {
          ...evidence,
          workerPlacements: [evidence.workerPlacements[0]!, evidence.workerPlacements[0]!],
        },
      }),
    ).toThrow(/must be distinct/);

    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: {
          ...evidence,
          workerPlacements: [
            evidence.workerPlacements[0]!,
            { ...evidence.workerPlacements[1]!, ownerRef: "owner.someone-else" },
          ],
        },
      }),
    ).toThrow(/one owner scope/);

    // A worker that only reached readiness after its scorecard existed cannot
    // be the worker that produced it.
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: {
          ...evidence,
          workerPlacements: [
            evidence.workerPlacements[0]!,
            { ...evidence.workerPlacements[1]!, updatedAt: "2026-08-01T17:00:00.000Z" },
          ],
        },
      }),
    ).toThrow(/before their scorecards exist/);

    // Resolved source states must carry the candidate's exact untouched
    // holdout revisions, not arbitrary refs.
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: {
          ...evidence,
          sourceStates: [
            evidence.sourceStates[0]!,
            { ...evidence.sourceStates[1]!, datasetDigest: digest("f") },
          ],
        },
      }),
    ).toThrow(/untouched holdout revisions/);

    // Mechanical evidence must cover all three kinds and bind a real scorecard.
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: { ...evidence, mechanicalEvidence: evidence.mechanicalEvidence.slice(0, 2) },
      }),
    ).toThrow(/independent mechanical evidence/);
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: {
          ...evidence,
          mechanicalEvidence: evidence.mechanicalEvidence.map((receipt) => ({
            ...receipt,
            scorecardRef: "scorecard.some-other-run",
          })),
        },
      }),
    ).toThrow(/one of the evaluated scorecards/);

    // The evidence must name the exact evaluation and candidate it is offered for.
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: { ...evidence, candidateDigest: digest("0") },
      }),
    ).toThrow(/bind the exact evaluation and candidate/);
  });

  test("derives the Pareto verdict from measured scorecards instead of accepting a claim", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;

    const dominating = validateForensicPromptEvaluation(evaluationInputFor(candidate));
    expect(dominating.paretoStatus).toBe("dominates");
    expect(dominating.paretoComparisons).toHaveLength(FORENSIC_PROMPT_PARETO_AXES.length);
    expect(
      dominating.paretoComparisons.find((comparison) => comparison.axis === "hit_rate"),
    ).toMatchObject({ candidateValue: 0.7, baselineValue: 0.4, verdict: "better" });
    expect(
      dominating.paretoComparisons.find((comparison) => comparison.axis === "tokens")?.verdict,
    ).toBe("equal");

    // Spending more tokens for the same discovery win is a trade-off, not
    // domination, and the derived status has to say so.
    const tradedOff = validateForensicPromptEvaluation(
      evaluationInputFor(candidate, { ...DOMINATING_AXIS_VALUES, tokens: 200_000 }),
    );
    expect(tradedOff.paretoStatus).toBe("non_dominated");
    expect(
      tradedOff.paretoComparisons.find((comparison) => comparison.axis === "tokens")?.verdict,
    ).toBe("worse");

    // An axis with no measured value on either side is reported as missing
    // evidence, never silently scored as a tie.
    const { reviewer_load: _omitted, ...withoutReviewerLoad } = DOMINATING_AXIS_VALUES;
    const incomplete = validateForensicPromptEvaluation(
      evaluationInputFor(candidate, withoutReviewerLoad),
    );
    expect(incomplete.paretoStatus).toBe("insufficient_evidence");
    expect(
      incomplete.paretoComparisons.find((comparison) => comparison.axis === "reviewer_load"),
    ).toMatchObject({ candidateValue: null, verdict: "unavailable" });
  });

  test("requires an operator release gate and rolls back without rewriting activation history", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluation = validateForensicPromptEvaluation(evaluationInputFor(candidate));
    const activation = promoteForensicPrompt({
      candidate,
      currentState: genesisState(),
      decidedAt: "2026-08-01T17:00:00.000Z",
      evaluation,
      evaluationRef: "evaluation.forensic.candidate.1",
      operatorDecisionRef: "operator-decision.forensic.1",
      operatorIdentityRef: "identity.operator.release",
      releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
      transitionRef: "transition.forensic.activate.1",
    });
    const rollback = rollbackForensicPrompt({
      currentState: stateAfter(activation),
      decidedAt: "2026-08-01T18:00:00.000Z",
      operatorDecisionRef: "operator-decision.forensic.rollback.1",
      operatorIdentityRef: "identity.operator.release",
      transitionRef: "transition.forensic.rollback.1",
    });
    expect(activation.activePromptDigest).toBe(candidate.candidateDigest);
    expect(activation.priorActivePromptDigest).toBeNull();
    expect(activation.sequence).toBe(1);
    expect(rollback.activePromptDigest).toBeNull();
    expect(rollback.priorActivePromptDigest).toBe(activation.activePromptDigest);
    expect(rollback.sequence).toBe(2);
    expect(activation.transitionType).toBe("activate");
    expect(rollback.transitionType).toBe("rollback");
    expect(forensicPromptTransitionDigestMatches(activation)).toBe(true);
    expect(forensicPromptTransitionDigestMatches(rollback)).toBe(true);
    expect(
      forensicPromptTransitionDigestMatches({ ...activation, activePromptDigest: digest("0") }),
    ).toBe(false);

    expect(() =>
      promoteForensicPrompt({
        candidate,
        currentState: genesisState(),
        decidedAt: "2026-08-01T17:00:00.000Z",
        evaluation,
        evaluationRef: "evaluation.forensic.candidate.1",
        operatorDecisionRef: "operator-decision.forensic.1",
        operatorIdentityRef: "identity.optimizer.generator",
        releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
        transitionRef: "transition.forensic.self-promote",
      }),
    ).toThrow(/cannot evaluate or promote themselves/);
  });

  test("reads the rollback anchor from the durable pointer instead of the caller", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluation = validateForensicPromptEvaluation(evaluationInputFor(candidate));
    const promote = (currentState: ForensicPromptGovernanceState) =>
      promoteForensicPrompt({
        candidate,
        currentState,
        decidedAt: "2026-08-01T17:00:00.000Z",
        evaluation,
        evaluationRef: "evaluation.forensic.candidate.1",
        operatorDecisionRef: "operator-decision.forensic.1",
        operatorIdentityRef: "identity.operator.release",
        releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
        transitionRef: "transition.forensic.activate.1",
      });

    // A promotion taken against a state whose pointer already holds a governed
    // prompt anchors on that observed digest, not on anything the caller says.
    const priorActivation = promote(genesisState());
    const rolledBack = rollbackForensicPrompt({
      currentState: stateAfter(priorActivation),
      decidedAt: "2026-08-01T18:00:00.000Z",
      operatorDecisionRef: "operator-decision.forensic.rollback.1",
      operatorIdentityRef: "identity.operator.release",
      transitionRef: "transition.forensic.rollback.1",
    });
    expect(rolledBack.rollbackAnchorDigest).toBe(priorActivation.rollbackAnchorDigest);

    // The pointer and its history must agree before any decision is taken.
    expect(() =>
      promote({ ...genesisState(), revision: 3 }),
    ).toThrow(/append-only history length/);
    expect(() =>
      promote({ ...stateAfter(priorActivation), activePromptDigest: digest("0") }),
    ).toThrow(/must equal the last recorded transition/);

    // Re-promoting the prompt that is already active is not a transition.
    expect(() => promote(stateAfter(priorActivation))).toThrow(/already the active governed/);
  });

  test("holds rollback to the same authority boundary as promotion", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluation = validateForensicPromptEvaluation(evaluationInputFor(candidate));
    const activation = promoteForensicPrompt({
      candidate,
      currentState: genesisState(),
      decidedAt: "2026-08-01T17:00:00.000Z",
      evaluation,
      evaluationRef: "evaluation.forensic.candidate.1",
      operatorDecisionRef: "operator-decision.forensic.1",
      operatorIdentityRef: "identity.operator.release",
      releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
      transitionRef: "transition.forensic.activate.1",
    });

    // The candidate's own producer cannot mint a rollback, which would
    // otherwise be an unguarded path to setting the active pointer.
    expect(() =>
      rollbackForensicPrompt({
        currentState: stateAfter(activation),
        decidedAt: "2026-08-01T18:00:00.000Z",
        operatorDecisionRef: "operator-decision.forensic.rollback.1",
        operatorIdentityRef: candidate.blueprintModuleVersion.provenance.createdByRef,
        transitionRef: "transition.forensic.rollback.1",
      }),
    ).toThrow(/cannot evaluate or promote themselves/);

    // There must be a recorded activation to revert.
    expect(() =>
      rollbackForensicPrompt({
        currentState: genesisState(),
        decidedAt: "2026-08-01T18:00:00.000Z",
        operatorDecisionRef: "operator-decision.forensic.rollback.1",
        operatorIdentityRef: "identity.operator.release",
        transitionRef: "transition.forensic.rollback.1",
      }),
    ).toThrow(/recorded activation to revert/);

    // A stored activation that has been edited fails its digest check.
    const tampered = { ...activation, activePromptDigest: digest("0") };
    expect(() =>
      rollbackForensicPrompt({
        currentState: {
          ...stateAfter(tampered),
          activePromptDigest: tampered.activePromptDigest,
        },
        decidedAt: "2026-08-01T18:00:00.000Z",
        operatorDecisionRef: "operator-decision.forensic.rollback.1",
        operatorIdentityRef: "identity.operator.release",
        transitionRef: "transition.forensic.rollback.1",
      }),
    ).toThrow(/immutable digest check/);
  });

  test("refuses promotion when the frozen Pareto axes were not all measured", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const { reviewer_load: _omitted, ...withoutReviewerLoad } = DOMINATING_AXIS_VALUES;
    const evaluation = validateForensicPromptEvaluation(
      evaluationInputFor(candidate, withoutReviewerLoad),
    );
    expect(evaluation.paretoStatus).toBe("insufficient_evidence");
    expect(() =>
      promoteForensicPrompt({
        candidate,
        currentState: genesisState(),
        decidedAt: "2026-08-01T17:00:00.000Z",
        evaluation,
        evaluationRef: "evaluation.forensic.candidate.1",
        operatorDecisionRef: "operator-decision.forensic.1",
        operatorIdentityRef: "identity.operator.release",
        releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
        transitionRef: "transition.forensic.activate.1",
      }),
    ).toThrow(/derived Pareto comparison/);
  });
});
