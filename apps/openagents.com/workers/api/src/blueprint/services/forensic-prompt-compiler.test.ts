import { describe, expect, test } from "vitest";

import {
  FORENSIC_FINDING_VERSION,
  FORENSIC_HYPOTHESIS_VERSION,
  FORENSIC_PROMPT_ARTIFACT_VERSION,
  FORENSIC_SCORECARD_VERSION,
  forensicPromptArtifactDigest,
  strictDecode,
  ForensicScorecardSchema,
  type ForensicPromptArtifact,
  type ForensicPromptIr,
} from "@openagentsinc/forensic-contract";

import type { BlueprintReleaseGate } from "../schemas/release-gate";
import type {
  ForensicPromptDatasetRevision,
  ForensicPromptMetricFreeze,
} from "../schemas/forensic-prompt-optimization";
import {
  compileForensicPromptCandidates,
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

const metricFreeze: ForensicPromptMetricFreeze = {
  censoringDefinitionDigest: digest("5"),
  eligibilityDefinitionDigest: digest("6"),
  frozenAt: "2026-08-01T16:00:00.000Z",
  metricDefinitionRevisionDigest: digest("7"),
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

const scorecard = (
  datasetRevisionDigest: string,
  candidateDigest: string,
  split: "holdout" | "clean_holdout",
  holdoutHit = true,
) =>
  strictDecode(ForensicScorecardSchema, {
    schema: FORENSIC_SCORECARD_VERSION,
    scorecardRef: `scorecard.${split}.v1`,
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
        values: [],
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
    const evaluationInput = {
      baselineHoldoutScorecard: scorecard(
        candidate.holdoutDatasetDigest,
        digest("0"),
        "holdout" as const,
        false,
      ),
      candidate,
      cleanHoldoutScorecard: scorecard(
        candidate.cleanHoldoutDatasetDigest,
        candidate.candidateDigest,
        "clean_holdout" as const,
      ),
      evaluatorIdentityRef: "identity.release.evaluator",
      freshSourceStateRefs: ["source-state.holdout.1", "source-state.clean.1"],
      freshWorkerPlacementRefs: ["placement.openagents-cloud.1", "placement.openagents-cloud.2"],
      holdoutScorecard: scorecard(
        candidate.holdoutDatasetDigest,
        candidate.candidateDigest,
        "holdout" as const,
      ),
      mechanicalEvidenceRefs: ["evidence.source-resolver", "evidence.fixed-control"],
      metricFreeze,
      paretoStatus: "non_dominated" as const,
    };
    const evaluation = validateForensicPromptEvaluation(evaluationInput);
    expect(evaluation.evaluatorIdentityRef).not.toBe(
      evaluationInput.candidate.blueprintModuleVersion.provenance.createdByRef,
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
        freshWorkerPlacementRefs: ["placement.openagents-cloud.shared"],
      }),
    ).toThrow(/fresh OpenAgents Cloud workers/);
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

  test("requires an operator release gate and rolls back without rewriting activation history", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluation = validateForensicPromptEvaluation({
      baselineHoldoutScorecard: scorecard(
        candidate.holdoutDatasetDigest,
        digest("0"),
        "holdout",
        false,
      ),
      candidate,
      cleanHoldoutScorecard: scorecard(
        candidate.cleanHoldoutDatasetDigest,
        candidate.candidateDigest,
        "clean_holdout",
      ),
      evaluatorIdentityRef: "identity.release.evaluator",
      freshSourceStateRefs: ["source-state.holdout.1", "source-state.clean.1"],
      freshWorkerPlacementRefs: ["placement.openagents-cloud.1", "placement.openagents-cloud.2"],
      holdoutScorecard: scorecard(
        candidate.holdoutDatasetDigest,
        candidate.candidateDigest,
        "holdout",
      ),
      mechanicalEvidenceRefs: ["evidence.source-resolver"],
      metricFreeze,
      paretoStatus: "non_dominated",
    });
    const activation = promoteForensicPrompt({
      candidate,
      decidedAt: "2026-08-01T17:00:00.000Z",
      evaluation,
      evaluationRef: "evaluation.forensic.candidate.1",
      operatorDecisionRef: "operator-decision.forensic.1",
      operatorIdentityRef: "identity.operator.release",
      priorActivePromptDigest: sourceArtifact().canonicalDigest,
      releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
      transitionRef: "transition.forensic.activate.1",
    });
    const rollback = rollbackForensicPrompt(
      activation,
      "transition.forensic.rollback.1",
      "operator-decision.forensic.rollback.1",
      "identity.operator.release",
      "2026-08-01T18:00:00.000Z",
    );
    expect(activation.activePromptDigest).toBe(candidate.candidateDigest);
    expect(rollback.activePromptDigest).toBe(sourceArtifact().canonicalDigest);
    expect(rollback.priorActivePromptDigest).toBe(activation.activePromptDigest);
    expect(activation.transitionType).toBe("activate");
    expect(rollback.transitionType).toBe("rollback");

    expect(() =>
      promoteForensicPrompt({
        candidate,
        decidedAt: "2026-08-01T17:00:00.000Z",
        evaluation,
        evaluationRef: "evaluation.forensic.candidate.1",
        operatorDecisionRef: "operator-decision.forensic.1",
        operatorIdentityRef: "identity.optimizer.generator",
        priorActivePromptDigest: sourceArtifact().canonicalDigest,
        releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
        transitionRef: "transition.forensic.self-promote",
      }),
    ).toThrow(/cannot evaluate or promote themselves/);
  });
});
