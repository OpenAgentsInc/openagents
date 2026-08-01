import { Schema as S } from "effect";

import {
  FORENSIC_FINDING_VERSION,
  FORENSIC_HYPOTHESIS_VERSION,
  FORENSIC_PROMPT_ARTIFACT_VERSION,
  ForensicPromptArtifactSchema,
  forensicPromptArtifactDigest,
  forensicSha256Digest,
  strictDecode,
  type ForensicPromptArtifact,
  type ForensicPromptIr,
  type ForensicScorecard,
} from "@openagentsinc/forensic-contract";

import { blueprintReleaseGateCanPromote, type BlueprintReleaseGate } from "../schemas/release-gate";
import type { BlueprintModuleVersion } from "../schemas/module";
import type { BlueprintOptimizerRun } from "../schemas/optimizer-run";
import {
  FORENSIC_PROMPT_CANDIDATE_VERSION,
  FORENSIC_PROMPT_COMPILER_VERSION,
  FORENSIC_PROMPT_TRANSITION_VERSION,
  ForensicPromptActiveTransition,
  ForensicPromptCandidate,
  ForensicPromptCompilerRun,
  ForensicPromptEvaluation,
  type ForensicPromptDatasetRevision,
  type ForensicPromptMetricFreeze,
  type ForensicPromptOptimizerConfiguration,
} from "../schemas/forensic-prompt-optimization";

const sha256Pattern = /^sha256:[0-9a-f]{64}$/;

const assertDigest = (label: string, value: string): void => {
  if (!sha256Pattern.test(value)) throw new Error(`${label} must be a sha256 digest`);
};

const assertDistinct = (label: string, values: ReadonlyArray<string>): void => {
  if (new Set(values).size !== values.length) throw new Error(`${label} must be distinct`);
};

const deepFreeze = <Value>(value: Value): Value => {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
};

export interface ForensicPromptCandidateInput {
  readonly exampleRefs: ReadonlyArray<string>;
  readonly parameterRefs: ReadonlyArray<string>;
  readonly promptIr: ForensicPromptIr;
  readonly summaryRef: string;
}

export interface CompileForensicPromptCandidatesInput {
  readonly candidateInputs: ReadonlyArray<ForensicPromptCandidateInput>;
  readonly compilerRef: string;
  readonly datasets: ReadonlyArray<ForensicPromptDatasetRevision>;
  readonly generatedAt: string;
  readonly metricFreeze: ForensicPromptMetricFreeze;
  readonly optimizerConfiguration: ForensicPromptOptimizerConfiguration;
  readonly optimizerRunRef: string;
  readonly retainedFailureRefs: ReadonlyArray<string>;
  readonly sourceArtifact: ForensicPromptArtifact;
}

const datasetBySplit = (
  datasets: ReadonlyArray<ForensicPromptDatasetRevision>,
  split: ForensicPromptDatasetRevision["split"],
): ForensicPromptDatasetRevision => {
  const matches = datasets.filter((dataset) => dataset.split === split);
  if (matches.length !== 1) throw new Error(`exactly one ${split} dataset revision is required`);
  return matches[0]!;
};

const validateCompilerInputs = (input: CompileForensicPromptCandidatesInput) => {
  strictDecode(S.Array(S.Unknown), input.candidateInputs);
  const sourceArtifact = strictDecode(ForensicPromptArtifactSchema, input.sourceArtifact);
  if (sourceArtifact.schema !== FORENSIC_PROMPT_ARTIFACT_VERSION) {
    throw new Error("source artifact must use the canonical forensic prompt schema");
  }
  if (input.candidateInputs.length === 0) throw new Error("at least one candidate is required");
  if (
    !Number.isSafeInteger(input.optimizerConfiguration.maxCandidates) ||
    input.optimizerConfiguration.maxCandidates < 1 ||
    input.candidateInputs.length > input.optimizerConfiguration.maxCandidates
  ) {
    throw new Error("candidate count exceeds the bounded optimizer configuration");
  }
  for (const digest of [
    input.optimizerConfiguration.configurationDigest,
    input.metricFreeze.metricDefinitionRevisionDigest,
    input.metricFreeze.t5DefinitionDigest,
    input.metricFreeze.censoringDefinitionDigest,
    input.metricFreeze.eligibilityDefinitionDigest,
  ]) {
    assertDigest("compiler input", digest);
  }
  const train = datasetBySplit(input.datasets, "train");
  const development = datasetBySplit(input.datasets, "development");
  const holdout = datasetBySplit(input.datasets, "holdout");
  const cleanHoldout = datasetBySplit(input.datasets, "clean_holdout");
  for (const dataset of input.datasets) assertDigest("dataset revision", dataset.digest);
  assertDistinct(
    "dataset revision digests",
    input.datasets.map((dataset) => dataset.digest),
  );
  if (
    train.optimizerVisibility !== "optimizer_visible" ||
    development.optimizerVisibility !== "optimizer_visible" ||
    holdout.optimizerVisibility !== "evaluator_only" ||
    cleanHoldout.optimizerVisibility !== "evaluator_only"
  ) {
    throw new Error(
      "holdout revisions are evaluator-only and train/development are optimizer-visible",
    );
  }
  const optimizerExamples = new Set([...train.exampleRefs, ...development.exampleRefs]);
  const holdoutExamples = new Set([...holdout.exampleRefs, ...cleanHoldout.exampleRefs]);
  for (const exampleRef of optimizerExamples) {
    if (holdoutExamples.has(exampleRef))
      throw new Error("optimizer-visible and holdout examples overlap");
  }
  for (const candidate of input.candidateInputs) {
    if (!candidate.exampleRefs.every((exampleRef) => optimizerExamples.has(exampleRef))) {
      throw new Error("candidate generation cannot read evaluator-only holdout examples");
    }
  }
  if (
    ["dspy", "gepa"].includes(input.optimizerConfiguration.kind) &&
    input.optimizerConfiguration.integrationReceiptRefs.length === 0
  ) {
    throw new Error("DSPy or GEPA naming requires an independently tested integration receipt");
  }
  return { cleanHoldout, development, holdout, sourceArtifact, train };
};

const createArtifact = (
  source: ForensicPromptArtifact,
  input: ForensicPromptCandidateInput,
  candidateRef: string,
  generatedAt: string,
): ForensicPromptArtifact => {
  if (
    input.promptIr.findingSchemaRef !== FORENSIC_FINDING_VERSION ||
    input.promptIr.hypothesisSchemaRef !== FORENSIC_HYPOTHESIS_VERSION
  ) {
    throw new Error("candidate cannot replace the typed finding or hypothesis schemas");
  }
  const digestInput = {
    parentPromptArtifactRef: source.promptArtifactRef,
    promptIr: input.promptIr,
    exampleRefs: input.exampleRefs,
    parameterRefs: input.parameterRefs,
    datasetRevisionRef: source.datasetRevisionRef,
    compatibilityRefs: source.compatibilityRefs,
  };
  return deepFreeze({
    schema: FORENSIC_PROMPT_ARTIFACT_VERSION,
    promptArtifactRef: candidateRef,
    ...digestInput,
    canonicalDigest: forensicPromptArtifactDigest(digestInput),
    createdAt: generatedAt,
  });
};

export const compileForensicPromptCandidates = (
  input: CompileForensicPromptCandidatesInput,
): ForensicPromptCompilerRun => {
  const { cleanHoldout, development, holdout, sourceArtifact, train } =
    validateCompilerInputs(input);
  const candidates = input.candidateInputs.map((candidateInput, index) => {
    const identityInput = {
      parentPromptDigest: sourceArtifact.canonicalDigest,
      promptIr: candidateInput.promptIr,
      findingSchemaRef: candidateInput.promptIr.findingSchemaRef,
      hypothesisSchemaRef: candidateInput.promptIr.hypothesisSchemaRef,
      toolPolicyRefs: candidateInput.promptIr.toolPolicyRefs,
      exampleRefs: candidateInput.exampleRefs,
      parameterRefs: candidateInput.parameterRefs,
      datasetRevisionDigests: input.datasets.map((dataset) => dataset.digest),
      metricFreeze: input.metricFreeze,
      optimizerConfigurationDigest: input.optimizerConfiguration.configurationDigest,
      optimizerKind: input.optimizerConfiguration.kind,
    };
    const candidateDigest = forensicSha256Digest(identityInput);
    const candidateRef = `forensic.prompt.candidate.${candidateDigest.slice("sha256:".length, 29)}`;
    const promptArtifact = createArtifact(
      sourceArtifact,
      candidateInput,
      candidateRef,
      input.generatedAt,
    );
    const blueprintModuleVersion: BlueprintModuleVersion = {
      artifactRefs: [promptArtifact.promptArtifactRef, promptArtifact.canonicalDigest],
      deprecatedAt: null,
      id: `${input.optimizerRunRef}.candidate.${index + 1}`,
      implementationRef: promptArtifact.promptArtifactRef,
      moduleKind: "optimizer_candidate",
      moduleRef: "blueprint.module.forensic_prompt",
      programSignatureId: "blueprint.signature.forensic_discovery.v1",
      programTypeId: "blueprint.program.forensic_discovery.v1",
      provenance: {
        createdByRef: input.optimizerConfiguration.generatorIdentityRef,
        optimizerRunId: input.optimizerRunRef,
        retainedFailureRefs: [...input.retainedFailureRefs],
        sourceModuleVersionId: sourceArtifact.promptArtifactRef,
        trainingDataRefs: [train.datasetRef, development.datasetRef],
      },
      releaseDecision: null,
      releaseState: "unpromoted",
      rollbackOfModuleVersionId: null,
      scorecards: [],
      status: "candidate",
      versionRef: candidateDigest,
    };
    return strictDecode(ForensicPromptCandidate, {
      blueprintModuleVersion,
      candidateDigest,
      candidateRef,
      cleanHoldoutDatasetDigest: cleanHoldout.digest,
      compilerVersion: FORENSIC_PROMPT_COMPILER_VERSION,
      datasetRevisionDigests: input.datasets.map((dataset) => dataset.digest),
      developmentDatasetDigest: development.digest,
      holdoutDatasetDigest: holdout.digest,
      metricFreezeDigest: forensicSha256Digest(input.metricFreeze),
      optimizerConfigurationDigest: input.optimizerConfiguration.configurationDigest,
      optimizerKind: input.optimizerConfiguration.kind,
      promptArtifact,
      retainedFailureRefs: input.retainedFailureRefs,
      schema: FORENSIC_PROMPT_CANDIDATE_VERSION,
      trainDatasetDigest: train.digest,
    });
  });
  const blueprintOptimizerRun: BlueprintOptimizerRun = {
    candidateModules: candidates.map((candidate, index) => ({
      candidateState: "candidate",
      candidateSummaryRef:
        input.candidateInputs[index]?.summaryRef ?? "summary.forensic_prompt_candidate",
      moduleVersionId: candidate.blueprintModuleVersion.id,
      releaseGateRef: "blueprint.release_gate.forensic_prompt.v1",
      scorecardRefs: [],
    })),
    createdAt: input.generatedAt,
    evidenceRefs: [input.metricFreeze.metricDefinitionRevisionDigest, ...input.retainedFailureRefs],
    id: input.optimizerRunRef,
    optimizerKind:
      input.optimizerConfiguration.kind === "human_curated"
        ? "human_curated"
        : input.optimizerConfiguration.kind === "ablation"
          ? "ablation"
          : input.optimizerConfiguration.kind === "retained_failure_replay"
            ? "retained_failure_replay"
            : "scorecard_search",
    retainedFailureRefs: [...input.retainedFailureRefs],
    scorecardRefs: [],
    status: "completed",
    updatedAt: input.generatedAt,
  };
  return deepFreeze(
    strictDecode(ForensicPromptCompilerRun, {
      blueprintOptimizerRun,
      candidates,
      compilerRef: input.compilerRef,
      generatedAt: input.generatedAt,
      metricFreeze: input.metricFreeze,
      optimizerConfiguration: input.optimizerConfiguration,
      schema: FORENSIC_PROMPT_COMPILER_VERSION,
    }),
  );
};

export interface ValidateForensicPromptEvaluationInput {
  readonly baselineHoldoutScorecard: ForensicScorecard;
  readonly candidate: ForensicPromptCandidate;
  readonly cleanHoldoutScorecard: ForensicScorecard;
  readonly evaluatorIdentityRef: string;
  readonly freshSourceStateRefs: ReadonlyArray<string>;
  readonly freshWorkerPlacementRefs: ReadonlyArray<string>;
  readonly holdoutScorecard: ForensicScorecard;
  readonly mechanicalEvidenceRefs: ReadonlyArray<string>;
  readonly metricFreeze: ForensicPromptMetricFreeze;
  readonly paretoStatus: "dominates" | "non_dominated";
}

export const validateForensicPromptEvaluation = (
  input: ValidateForensicPromptEvaluationInput,
): ForensicPromptEvaluation => {
  if (
    input.evaluatorIdentityRef === input.candidate.blueprintModuleVersion.provenance.createdByRef
  ) {
    throw new Error("candidate generation and release evaluation require distinct identities");
  }
  if (input.freshWorkerPlacementRefs.length < 2 || input.freshSourceStateRefs.length < 2) {
    throw new Error("holdout evaluation requires fresh OpenAgents Cloud workers and source state");
  }
  if (
    input.baselineHoldoutScorecard.datasetRevisionDigest !== input.candidate.holdoutDatasetDigest ||
    input.baselineHoldoutScorecard.metricDefinitionRevisionDigest !==
      input.metricFreeze.metricDefinitionRevisionDigest
  ) {
    throw new Error("baseline and candidate must use the same untouched holdout and metrics");
  }
  if (forensicSha256Digest(input.metricFreeze) !== input.candidate.metricFreezeDigest) {
    throw new Error("evaluation cannot change the candidate's frozen metric definitions");
  }
  assertDistinct("worker placement refs", input.freshWorkerPlacementRefs);
  assertDistinct("source state refs", input.freshSourceStateRefs);
  if (
    input.holdoutScorecard.candidateDigest !== input.candidate.candidateDigest ||
    input.cleanHoldoutScorecard.candidateDigest !== input.candidate.candidateDigest ||
    input.holdoutScorecard.datasetRevisionDigest !== input.candidate.holdoutDatasetDigest ||
    input.cleanHoldoutScorecard.datasetRevisionDigest !== input.candidate.cleanHoldoutDatasetDigest
  ) {
    throw new Error("evaluation scorecards must bind the candidate and untouched holdouts");
  }
  for (const scorecard of [input.holdoutScorecard, input.cleanHoldoutScorecard]) {
    if (
      scorecard.metricDefinitionRevisionDigest !==
        input.metricFreeze.metricDefinitionRevisionDigest ||
      !scorecard.hardGates.every((gate) => gate.passed)
    ) {
      throw new Error("hard gates and the frozen metric revision apply before comparison");
    }
  }
  if (input.cleanHoldoutScorecard.runs.some((run) => run.outcome === "hit")) {
    throw new Error("clean holdout regression blocks the candidate");
  }
  const candidateHits = input.holdoutScorecard.runs.filter((run) => run.outcome === "hit").length;
  const baselineHits = input.baselineHoldoutScorecard.runs.filter(
    (run) => run.outcome === "hit",
  ).length;
  if (input.holdoutScorecard.runs.length !== input.baselineHoldoutScorecard.runs.length) {
    throw new Error("baseline and candidate must use matched holdout populations");
  }
  if (candidateHits <= baselineHits) {
    throw new Error("a candidate must improve the untouched holdout");
  }
  if (input.mechanicalEvidenceRefs.length === 0) {
    throw new Error("independent mechanical evidence is required");
  }
  return deepFreeze(
    strictDecode(ForensicPromptEvaluation, {
      baselineHoldoutScorecard: input.baselineHoldoutScorecard,
      candidateDigest: input.candidate.candidateDigest,
      cleanHoldoutScorecard: input.cleanHoldoutScorecard,
      evaluatorIdentityRef: input.evaluatorIdentityRef,
      freshSourceStateRefs: input.freshSourceStateRefs,
      freshWorkerPlacementRefs: input.freshWorkerPlacementRefs,
      generatorIdentityRef: input.candidate.blueprintModuleVersion.provenance.createdByRef,
      holdoutScorecard: input.holdoutScorecard,
      mechanicalEvidenceRefs: input.mechanicalEvidenceRefs,
      metricFreeze: input.metricFreeze,
      paretoStatus: input.paretoStatus,
      schema: "openagents.blueprint.forensic_prompt_evaluation.v1",
    }),
  );
};

export interface PromoteForensicPromptInput {
  readonly candidate: ForensicPromptCandidate;
  readonly decidedAt: string;
  readonly evaluation: ForensicPromptEvaluation;
  readonly evaluationRef: string;
  readonly operatorDecisionRef: string;
  readonly operatorIdentityRef: string;
  readonly priorActivePromptDigest: string;
  readonly releaseGate: BlueprintReleaseGate;
  readonly transitionRef: string;
}

export const promoteForensicPrompt = (
  input: PromoteForensicPromptInput,
): ForensicPromptActiveTransition => {
  const producer = input.candidate.blueprintModuleVersion.provenance.createdByRef;
  if (input.operatorIdentityRef === producer || input.releaseGate.decidedByRef === producer) {
    throw new Error("prompt candidates cannot evaluate or promote themselves");
  }
  if (
    input.evaluation.candidateDigest !== input.candidate.candidateDigest ||
    input.operatorIdentityRef === input.evaluation.evaluatorIdentityRef ||
    input.releaseGate.scorecardRef !== input.evaluation.holdoutScorecard.scorecardRef
  ) {
    throw new Error("promotion requires the independent candidate evaluation and operator");
  }
  if (
    !blueprintReleaseGateCanPromote(input.releaseGate) ||
    input.releaseGate.targetRef !== input.candidate.blueprintModuleVersion.id
  ) {
    throw new Error("an explicit passing Blueprint release gate is required");
  }
  return deepFreeze(
    strictDecode(ForensicPromptActiveTransition, {
      activePromptDigest: input.candidate.candidateDigest,
      candidateDigest: input.candidate.candidateDigest,
      decidedAt: input.decidedAt,
      evaluationRef: input.evaluationRef,
      operatorDecisionRef: input.operatorDecisionRef,
      operatorIdentityRef: input.operatorIdentityRef,
      priorActivePromptDigest: input.priorActivePromptDigest,
      releaseGateRef: input.releaseGate.id,
      rollbackAnchorDigest: input.priorActivePromptDigest,
      schema: FORENSIC_PROMPT_TRANSITION_VERSION,
      transitionRef: input.transitionRef,
      transitionType: "activate",
    }),
  );
};

export const rollbackForensicPrompt = (
  activation: ForensicPromptActiveTransition,
  transitionRef: string,
  operatorDecisionRef: string,
  operatorIdentityRef: string,
  decidedAt: string,
): ForensicPromptActiveTransition =>
  deepFreeze(
    strictDecode(ForensicPromptActiveTransition, {
      activePromptDigest: activation.rollbackAnchorDigest,
      candidateDigest: activation.candidateDigest,
      decidedAt,
      evaluationRef: activation.evaluationRef,
      operatorDecisionRef,
      operatorIdentityRef,
      priorActivePromptDigest: activation.activePromptDigest,
      releaseGateRef: activation.releaseGateRef,
      rollbackAnchorDigest: activation.rollbackAnchorDigest,
      schema: FORENSIC_PROMPT_TRANSITION_VERSION,
      transitionRef,
      transitionType: "rollback",
    }),
  );
