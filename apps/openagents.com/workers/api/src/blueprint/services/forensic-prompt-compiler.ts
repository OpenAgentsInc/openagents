import { Schema as S } from "effect";

import { FROZEN_FORENSIC_METRIC_REGISTRY } from "@openagentsinc/forensic-contract/metrics";

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
  FORENSIC_PROMPT_PARETO_AXES,
  FORENSIC_PROMPT_TRANSITION_VERSION,
  ForensicPromptActiveTransition,
  ForensicPromptCandidate,
  ForensicPromptCompilerRun,
  ForensicPromptEvaluation,
  type ForensicPromptDatasetRevision,
  type ForensicPromptEvaluationEvidence,
  type ForensicPromptGovernanceState,
  type ForensicPromptMetricFreeze,
  type ForensicPromptOptimizerConfiguration,
  type ForensicPromptParetoComparison,
} from "../schemas/forensic-prompt-optimization";

const sha256Pattern = /^sha256:[0-9a-f]{64}$/;

const assertDigest = (label: string, value: string): void => {
  if (!sha256Pattern.test(value)) throw new Error(`${label} must be a sha256 digest`);
};

const assertDistinct = (label: string, values: ReadonlyArray<string>): void => {
  if (new Set(values).size !== values.length) throw new Error(`${label} must be distinct`);
};

const FROZEN_METRIC_REFS: ReadonlySet<string> = new Set(
  FROZEN_FORENSIC_METRIC_REGISTRY.definitions.map((definition) => definition.metricRef),
);

const assertFrozenParetoAxes = (metricFreeze: ForensicPromptMetricFreeze): void => {
  const declared = metricFreeze.paretoAxes.map((binding) => binding.axis);
  if (
    declared.length !== FORENSIC_PROMPT_PARETO_AXES.length ||
    FORENSIC_PROMPT_PARETO_AXES.some((axis) => !declared.includes(axis))
  ) {
    throw new Error(
      "the metric freeze must bind every Pareto axis exactly once before results are known",
    );
  }
  assertDistinct(
    "Pareto axis bindings",
    metricFreeze.paretoAxes.map((binding) => binding.axis),
  );
  assertDistinct(
    "Pareto axis metric refs",
    metricFreeze.paretoAxes.map((binding) => binding.metricRef),
  );
  // Without this, an axis could be bound to a metric ref that no frozen
  // definition owns. The whole comparison is then computed over values the
  // reporter invented and also supplied in its own scorecard, which is exactly
  // the assertion the derived Pareto verdict exists to remove.
  for (const binding of metricFreeze.paretoAxes) {
    if (!FROZEN_METRIC_REFS.has(binding.metricRef)) {
      throw new Error(
        `Pareto axis ${binding.axis} must bind a metric the frozen forensic metric registry defines`,
      );
    }
  }
};

/**
 * Aggregate one frozen metric ref across a scorecard's retained runs. Only
 * values the evaluator recorded as available contribute; an axis with no
 * available value on either side is reported as unavailable rather than being
 * silently treated as a tie.
 */
const aggregateMetric = (
  scorecard: ForensicScorecard,
  metricRef: string,
): { readonly mean: number | null; readonly sampleCount: number } => {
  let total = 0;
  let sampleCount = 0;
  for (const run of scorecard.runs) {
    for (const value of run.values) {
      if (value.metricRef !== metricRef || value.exactness === "unavailable") continue;
      const numeric = value.numericValue ?? (value.booleanValue === true ? 1 : 0);
      total += numeric;
      sampleCount += 1;
    }
  }
  return { mean: sampleCount === 0 ? null : total / sampleCount, sampleCount };
};

const PARETO_TOLERANCE = 1e-9;

/**
 * Derive the Pareto verdict from the measured scorecards. This runs only after
 * every hard gate has already passed, so a gate failure can never be traded off
 * against a Pareto win.
 */
const deriveParetoComparison = (
  baselineHoldoutScorecard: ForensicScorecard,
  holdoutScorecard: ForensicScorecard,
  metricFreeze: ForensicPromptMetricFreeze,
): {
  readonly comparisons: ReadonlyArray<ForensicPromptParetoComparison>;
  readonly status: "dominates" | "non_dominated" | "insufficient_evidence";
} => {
  const comparisons = metricFreeze.paretoAxes.map((binding) => {
    const baseline = aggregateMetric(baselineHoldoutScorecard, binding.metricRef);
    const candidate = aggregateMetric(holdoutScorecard, binding.metricRef);
    let verdict: "better" | "worse" | "equal" | "unavailable" = "unavailable";
    if (baseline.mean !== null && candidate.mean !== null) {
      const delta =
        binding.direction === "maximize"
          ? candidate.mean - baseline.mean
          : baseline.mean - candidate.mean;
      verdict = delta > PARETO_TOLERANCE ? "better" : delta < -PARETO_TOLERANCE ? "worse" : "equal";
    }
    return {
      axis: binding.axis,
      baselineSampleCount: baseline.sampleCount,
      baselineValue: baseline.mean,
      candidateSampleCount: candidate.sampleCount,
      candidateValue: candidate.mean,
      direction: binding.direction,
      metricRef: binding.metricRef,
      verdict,
    };
  });
  if (comparisons.some((comparison) => comparison.verdict === "unavailable")) {
    return { comparisons, status: "insufficient_evidence" };
  }
  const dominates =
    comparisons.every((comparison) => comparison.verdict !== "worse") &&
    comparisons.some((comparison) => comparison.verdict === "better");
  return { comparisons, status: dominates ? "dominates" : "non_dominated" };
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
  assertFrozenParetoAxes(input.metricFreeze);
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
  readonly evaluationRef: string;
  readonly evaluatorIdentityRef: string;
  readonly evidence: ForensicPromptEvaluationEvidence;
  readonly holdoutScorecard: ForensicScorecard;
  readonly metricFreeze: ForensicPromptMetricFreeze;
}

/**
 * Check the resolved worker and source-state evidence for one evaluation.
 *
 * The placements are typed `ForensicWorkerPlacement` records, so the forensic
 * contract already requires admission and readiness receipts for a ready or
 * running worker, the managed target class, the Google Cloud provider, and the
 * broker-only network policy. What this adds is the binding that the contract
 * cannot see on its own: the workers must be genuinely separate sandboxes owned
 * by one owner, they must have reached readiness before the scorecards they
 * produced were generated, and the resolved source states must carry the exact
 * untouched holdout dataset digests the candidate declared.
 *
 * This is structural resolution of typed records, not a lookup against a live
 * placement authority. It cannot be satisfied by arbitrary caller strings, and
 * it is still evidence the evaluator submits.
 */
const assertResolvedEvaluationEvidence = (
  input: ValidateForensicPromptEvaluationInput,
): void => {
  const { evidence } = input;
  if (
    evidence.evaluationRef !== input.evaluationRef ||
    evidence.candidateDigest !== input.candidate.candidateDigest
  ) {
    throw new Error("resolved evaluation evidence must bind the exact evaluation and candidate");
  }
  if (evidence.workerPlacements.length < 2 || evidence.sourceStates.length < 2) {
    throw new Error("holdout evaluation requires fresh OpenAgents Cloud workers and source state");
  }
  assertDistinct(
    "worker placement refs",
    evidence.workerPlacements.map((placement) => placement.placementRef),
  );
  assertDistinct(
    "worker sandboxes",
    evidence.workerPlacements.map((placement) => placement.sandboxRef),
  );
  assertDistinct(
    "worker work units",
    evidence.workerPlacements.map((placement) => placement.workUnitRef),
  );
  const ownerRefs = new Set(evidence.workerPlacements.map((placement) => placement.ownerRef));
  if (ownerRefs.size !== 1) {
    throw new Error("evaluation workers must belong to one owner scope");
  }
  for (const placement of evidence.workerPlacements) {
    if (
      !["worker_ready", "running"].includes(placement.state) ||
      placement.admissionReceiptRef === undefined ||
      placement.readinessReceiptRef === undefined
    ) {
      throw new Error("evaluation workers must be admitted ready OpenAgents Cloud placements");
    }
    const readyAt = Date.parse(placement.updatedAt);
    if (Number.isNaN(readyAt)) throw new Error("worker placements require an observable timestamp");
    for (const scorecard of [input.holdoutScorecard, input.cleanHoldoutScorecard]) {
      if (readyAt > Date.parse(scorecard.generatedAt)) {
        throw new Error("evaluation workers must reach readiness before their scorecards exist");
      }
    }
  }
  assertDistinct(
    "source state refs",
    evidence.sourceStates.map((source) => source.sourceStateRef),
  );
  for (const source of evidence.sourceStates) {
    assertDigest("resolved source state", source.sourceDigest);
    assertDigest("resolved source dataset", source.datasetDigest);
  }
  const resolvedDatasets = new Set(evidence.sourceStates.map((source) => source.datasetDigest));
  if (
    !resolvedDatasets.has(input.candidate.holdoutDatasetDigest) ||
    !resolvedDatasets.has(input.candidate.cleanHoldoutDatasetDigest)
  ) {
    throw new Error("resolved source states must bind both untouched holdout revisions");
  }
  const scorecardRefs = new Set([
    input.baselineHoldoutScorecard.scorecardRef,
    input.holdoutScorecard.scorecardRef,
    input.cleanHoldoutScorecard.scorecardRef,
  ]);
  const outstanding = new Set([
    "scorecard_rebuilt",
    "source_state_resolved",
    "worker_lifecycle_resolved",
  ]);
  assertDistinct(
    "mechanical evidence refs",
    evidence.mechanicalEvidence.map((receipt) => receipt.evidenceRef),
  );
  for (const receipt of evidence.mechanicalEvidence) {
    outstanding.delete(receipt.evidenceType);
    assertDigest("mechanical evidence", receipt.receiptDigest);
    if (!scorecardRefs.has(receipt.scorecardRef)) {
      throw new Error("mechanical evidence must bind one of the evaluated scorecards");
    }
  }
  if (outstanding.size > 0) {
    throw new Error("independent mechanical evidence is required");
  }
};

export const validateForensicPromptEvaluation = (
  input: ValidateForensicPromptEvaluationInput,
): ForensicPromptEvaluation => {
  if (
    input.evaluatorIdentityRef === input.candidate.blueprintModuleVersion.provenance.createdByRef
  ) {
    throw new Error("candidate generation and release evaluation require distinct identities");
  }
  assertFrozenParetoAxes(input.metricFreeze);
  assertResolvedEvaluationEvidence(input);
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
  // Every hard gate has now passed, so the Pareto comparison below can never be
  // traded off against a failing gate.
  const pareto = deriveParetoComparison(
    input.baselineHoldoutScorecard,
    input.holdoutScorecard,
    input.metricFreeze,
  );
  return deepFreeze(
    strictDecode(ForensicPromptEvaluation, {
      baselineHoldoutScorecard: input.baselineHoldoutScorecard,
      candidateDigest: input.candidate.candidateDigest,
      cleanHoldoutScorecard: input.cleanHoldoutScorecard,
      evaluationRef: input.evaluationRef,
      evaluatorIdentityRef: input.evaluatorIdentityRef,
      evidenceReceiptDigest: forensicSha256Digest(input.evidence),
      generatorIdentityRef: input.candidate.blueprintModuleVersion.provenance.createdByRef,
      holdoutScorecard: input.holdoutScorecard,
      mechanicalEvidenceRefs: input.evidence.mechanicalEvidence.map(
        (receipt) => receipt.evidenceRef,
      ),
      metricFreeze: input.metricFreeze,
      paretoComparisons: pareto.comparisons,
      paretoStatus: pareto.status,
      schema: "openagents.blueprint.forensic_prompt_evaluation.v1",
    }),
  );
};

type ForensicPromptTransitionDraft = Omit<ForensicPromptActiveTransition, "transitionDigest">;

const sealTransition = (draft: ForensicPromptTransitionDraft): ForensicPromptActiveTransition =>
  deepFreeze(
    strictDecode(ForensicPromptActiveTransition, {
      ...draft,
      transitionDigest: forensicSha256Digest(draft),
    }),
  );

/**
 * Recompute a stored transition's digest over every field except the digest
 * itself. A transition that has been edited in storage no longer verifies.
 */
export const forensicPromptTransitionDigestMatches = (
  transition: ForensicPromptActiveTransition,
): boolean => {
  const { transitionDigest, ...draft } = transition;
  return forensicSha256Digest(draft) === transitionDigest;
};

const assertGovernanceState = (state: ForensicPromptGovernanceState): void => {
  if (!Number.isSafeInteger(state.revision) || state.revision < 0) {
    throw new Error("governance state requires a whole revision");
  }
  if (state.revision !== state.history.length) {
    throw new Error("governance revision must equal its append-only history length");
  }
  const last = state.history.at(-1);
  if ((last?.activePromptDigest ?? null) !== state.activePromptDigest) {
    throw new Error("the active pointer must equal the last recorded transition");
  }
};

export interface PromoteForensicPromptInput {
  readonly candidate: ForensicPromptCandidate;
  /** The durable pointer and history read immediately before this decision. */
  readonly currentState: ForensicPromptGovernanceState;
  readonly decidedAt: string;
  readonly evaluation: ForensicPromptEvaluation;
  readonly evaluationRef: string;
  readonly operatorDecisionRef: string;
  readonly operatorIdentityRef: string;
  readonly releaseGate: BlueprintReleaseGate;
  readonly transitionRef: string;
}

/**
 * Promotion never accepts a caller-chosen prior digest. The prior active digest
 * and the rollback anchor are read from the durable governance state, and the
 * sequence continues that state's history, so the store's compare-and-set
 * rejects a decision taken against a stale read.
 */
export const promoteForensicPrompt = (
  input: PromoteForensicPromptInput,
): ForensicPromptActiveTransition => {
  const producer = input.candidate.blueprintModuleVersion.provenance.createdByRef;
  if (input.operatorIdentityRef === producer || input.releaseGate.decidedByRef === producer) {
    throw new Error("prompt candidates cannot evaluate or promote themselves");
  }
  if (
    input.evaluation.candidateDigest !== input.candidate.candidateDigest ||
    input.evaluation.evaluationRef !== input.evaluationRef ||
    input.operatorIdentityRef === input.evaluation.evaluatorIdentityRef ||
    input.releaseGate.scorecardRef !== input.evaluation.holdoutScorecard.scorecardRef
  ) {
    throw new Error("promotion requires the independent candidate evaluation and operator");
  }
  if (input.evaluation.paretoStatus === "insufficient_evidence") {
    throw new Error("promotion requires a derived Pareto comparison on every frozen axis");
  }
  if (
    !blueprintReleaseGateCanPromote(input.releaseGate) ||
    input.releaseGate.targetRef !== input.candidate.blueprintModuleVersion.id
  ) {
    throw new Error("an explicit passing Blueprint release gate is required");
  }
  assertGovernanceState(input.currentState);
  if (input.currentState.activePromptDigest === input.candidate.candidateDigest) {
    throw new Error("the candidate is already the active governed prompt");
  }
  return sealTransition({
    activePromptDigest: input.candidate.candidateDigest,
    candidateDigest: input.candidate.candidateDigest,
    candidateProducerRef: producer,
    decidedAt: input.decidedAt,
    evaluationRef: input.evaluationRef,
    operatorDecisionRef: input.operatorDecisionRef,
    operatorIdentityRef: input.operatorIdentityRef,
    priorActivePromptDigest: input.currentState.activePromptDigest,
    releaseGateRef: input.releaseGate.id,
    rollbackAnchorDigest: input.currentState.activePromptDigest,
    schema: FORENSIC_PROMPT_TRANSITION_VERSION,
    sequence: input.currentState.revision + 1,
    transitionRef: input.transitionRef,
    transitionType: "activate",
  });
};

export interface RollbackForensicPromptInput {
  /** The durable pointer and history read immediately before this decision. */
  readonly currentState: ForensicPromptGovernanceState;
  readonly decidedAt: string;
  readonly operatorDecisionRef: string;
  readonly operatorIdentityRef: string;
  readonly transitionRef: string;
}

/**
 * Rollback is the reverse of promotion and is held to the same authority
 * boundary. It reverts exactly the activation that is currently in force, it
 * takes its anchor from that stored activation rather than from the caller, and
 * it refuses an operator who produced the candidate being reverted. Without
 * these checks rollback would be an unguarded path to the outcome promotion is
 * guarded against.
 *
 * It deliberately does not require a release gate: reverting to an already
 * governed prior prompt is a safety action, and gating it would block recovery.
 */
export const rollbackForensicPrompt = (
  input: RollbackForensicPromptInput,
): ForensicPromptActiveTransition => {
  assertGovernanceState(input.currentState);
  const activation = input.currentState.history.at(-1);
  if (activation === undefined) {
    throw new Error("rollback requires a recorded activation to revert");
  }
  if (activation.transitionType !== "activate") {
    throw new Error("rollback can only revert the activation currently in force");
  }
  if (!forensicPromptTransitionDigestMatches(activation)) {
    throw new Error("the stored activation failed its immutable digest check");
  }
  if (input.operatorIdentityRef === activation.candidateProducerRef) {
    throw new Error("prompt candidates cannot evaluate or promote themselves");
  }
  return sealTransition({
    activePromptDigest: activation.rollbackAnchorDigest,
    candidateDigest: activation.candidateDigest,
    candidateProducerRef: activation.candidateProducerRef,
    decidedAt: input.decidedAt,
    evaluationRef: activation.evaluationRef,
    operatorDecisionRef: input.operatorDecisionRef,
    operatorIdentityRef: input.operatorIdentityRef,
    priorActivePromptDigest: activation.activePromptDigest,
    releaseGateRef: activation.releaseGateRef,
    rollbackAnchorDigest: activation.rollbackAnchorDigest,
    schema: FORENSIC_PROMPT_TRANSITION_VERSION,
    sequence: input.currentState.revision + 1,
    transitionRef: input.transitionRef,
    transitionType: "rollback",
  });
};
