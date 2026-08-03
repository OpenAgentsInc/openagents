import { Schema as S } from "effect";

import {
  ForensicPromptArtifactSchema,
  ForensicScorecardSchema,
  ForensicWorkerPlacementSchema,
} from "@openagentsinc/forensic-contract";

import { BlueprintModuleVersion } from "./module";
import { BlueprintOptimizerRun } from "./optimizer-run";

export const FORENSIC_PROMPT_COMPILER_VERSION =
  "openagents.blueprint.forensic_prompt_compiler.v1" as const;
export const FORENSIC_PROMPT_CANDIDATE_VERSION =
  "openagents.blueprint.forensic_prompt_candidate.v1" as const;
export const FORENSIC_PROMPT_EVALUATION_VERSION =
  "openagents.blueprint.forensic_prompt_evaluation.v1" as const;
export const FORENSIC_PROMPT_TRANSITION_VERSION =
  "openagents.blueprint.forensic_prompt_transition.v1" as const;
export const FORENSIC_PROMPT_EVALUATION_EVIDENCE_VERSION =
  "openagents.blueprint.forensic_prompt_evaluation_evidence.v1" as const;
export const FORENSIC_PROMPT_GOVERNANCE_STATE_VERSION =
  "openagents.blueprint.forensic_prompt_governance_state.v1" as const;

export const ForensicPromptCompilerKind = S.Literals([
  "human_curated",
  "ablation",
  "instruction_grid",
  "few_shot_selection",
  "context_budget_grid",
  "retained_failure_replay",
  "dspy",
  "gepa",
]);
export type ForensicPromptCompilerKind = typeof ForensicPromptCompilerKind.Type;

export const ForensicDatasetSplit = S.Literals([
  "train",
  "development",
  "holdout",
  "clean_holdout",
]);
export type ForensicDatasetSplit = typeof ForensicDatasetSplit.Type;

export const ForensicOptimizerVisibility = S.Literals(["optimizer_visible", "evaluator_only"]);
export type ForensicOptimizerVisibility = typeof ForensicOptimizerVisibility.Type;

export const ForensicPromptDatasetRevision = S.Struct({
  datasetRef: S.String,
  digest: S.String,
  exampleRefs: S.Array(S.String),
  optimizerVisibility: ForensicOptimizerVisibility,
  split: ForensicDatasetSplit,
});
export type ForensicPromptDatasetRevision = typeof ForensicPromptDatasetRevision.Type;

/**
 * The exact Pareto axes named by the OFR-013 acceptance criterion. Every axis
 * must be bound to a frozen metric ref and an explicit direction before any
 * result is known, so a Pareto verdict is derived from measured scorecard
 * values rather than asserted by whoever reports the evaluation.
 */
export const FORENSIC_PROMPT_PARETO_AXES = [
  "hit_rate",
  "causal_coverage",
  "time",
  "tokens",
  "cost",
  "false_positives",
  "reviewer_load",
] as const;

export const ForensicPromptParetoAxis = S.Literals([...FORENSIC_PROMPT_PARETO_AXES]);
export type ForensicPromptParetoAxis = typeof ForensicPromptParetoAxis.Type;

export const ForensicPromptParetoAxisBinding = S.Struct({
  axis: ForensicPromptParetoAxis,
  direction: S.Literals(["maximize", "minimize"]),
  metricRef: S.String,
});
export type ForensicPromptParetoAxisBinding = typeof ForensicPromptParetoAxisBinding.Type;

export const ForensicPromptParetoComparison = S.Struct({
  axis: ForensicPromptParetoAxis,
  baselineValue: S.NullOr(S.Number),
  candidateSampleCount: S.Number,
  baselineSampleCount: S.Number,
  candidateValue: S.NullOr(S.Number),
  direction: S.Literals(["maximize", "minimize"]),
  metricRef: S.String,
  verdict: S.Literals(["better", "worse", "equal", "unavailable"]),
});
export type ForensicPromptParetoComparison = typeof ForensicPromptParetoComparison.Type;

export const ForensicPromptMetricFreeze = S.Struct({
  censoringDefinitionDigest: S.String,
  eligibilityDefinitionDigest: S.String,
  frozenAt: S.String,
  metricDefinitionRevisionDigest: S.String,
  paretoAxes: S.Array(ForensicPromptParetoAxisBinding),
  t5DefinitionDigest: S.String,
});
export type ForensicPromptMetricFreeze = typeof ForensicPromptMetricFreeze.Type;

export const ForensicPromptOptimizerConfiguration = S.Struct({
  configurationDigest: S.String,
  generatorIdentityRef: S.String,
  integrationReceiptRefs: S.Array(S.String),
  kind: ForensicPromptCompilerKind,
  maxCandidates: S.Number,
});
export type ForensicPromptOptimizerConfiguration = typeof ForensicPromptOptimizerConfiguration.Type;

export const ForensicPromptCandidate = S.Struct({
  blueprintModuleVersion: BlueprintModuleVersion,
  candidateDigest: S.String,
  candidateRef: S.String,
  cleanHoldoutDatasetDigest: S.String,
  compilerVersion: S.Literal(FORENSIC_PROMPT_COMPILER_VERSION),
  datasetRevisionDigests: S.Array(S.String),
  developmentDatasetDigest: S.String,
  holdoutDatasetDigest: S.String,
  metricFreezeDigest: S.String,
  optimizerConfigurationDigest: S.String,
  optimizerKind: ForensicPromptCompilerKind,
  promptArtifact: ForensicPromptArtifactSchema,
  retainedFailureRefs: S.Array(S.String),
  schema: S.Literal(FORENSIC_PROMPT_CANDIDATE_VERSION),
  trainDatasetDigest: S.String,
});
export type ForensicPromptCandidate = typeof ForensicPromptCandidate.Type;

export const ForensicPromptCompilerRun = S.Struct({
  blueprintOptimizerRun: BlueprintOptimizerRun,
  candidates: S.Array(ForensicPromptCandidate),
  compilerRef: S.String,
  generatedAt: S.String,
  metricFreeze: ForensicPromptMetricFreeze,
  optimizerConfiguration: ForensicPromptOptimizerConfiguration,
  schema: S.Literal(FORENSIC_PROMPT_COMPILER_VERSION),
});
export type ForensicPromptCompilerRun = typeof ForensicPromptCompilerRun.Type;

/**
 * A resolved source-state record for one evaluated dataset revision. The
 * dataset digest is what binds the record to the candidate's untouched holdout
 * revisions, so a caller cannot satisfy the freshness requirement with refs
 * that name nothing the candidate actually declared.
 */
export const ForensicPromptSourceStateReceipt = S.Struct({
  datasetDigest: S.String,
  materializationReceiptRef: S.String,
  observedAt: S.String,
  sourceDigest: S.String,
  sourceStateRef: S.String,
});
export type ForensicPromptSourceStateReceipt = typeof ForensicPromptSourceStateReceipt.Type;

export const ForensicPromptMechanicalEvidenceKind = S.Literals([
  "scorecard_rebuilt",
  "source_state_resolved",
  "worker_lifecycle_resolved",
]);
export type ForensicPromptMechanicalEvidenceKind =
  typeof ForensicPromptMechanicalEvidenceKind.Type;

export const ForensicPromptMechanicalEvidenceReceipt = S.Struct({
  evidenceRef: S.String,
  evidenceType: ForensicPromptMechanicalEvidenceKind,
  observedAt: S.String,
  receiptDigest: S.String,
  scorecardRef: S.String,
});
export type ForensicPromptMechanicalEvidenceReceipt =
  typeof ForensicPromptMechanicalEvidenceReceipt.Type;

/**
 * Resolved evaluation evidence. Worker placements are the typed forensic
 * contract records, so admission and readiness receipts, the managed target
 * class, the Google Cloud provider, and the broker-only network policy are
 * checked by the contract schema itself instead of being free-form strings.
 */
export const ForensicPromptEvaluationEvidence = S.Struct({
  candidateDigest: S.String,
  evaluationRef: S.String,
  mechanicalEvidence: S.Array(ForensicPromptMechanicalEvidenceReceipt),
  recordedAt: S.String,
  schema: S.Literal(FORENSIC_PROMPT_EVALUATION_EVIDENCE_VERSION),
  sourceStates: S.Array(ForensicPromptSourceStateReceipt),
  workerPlacements: S.Array(ForensicWorkerPlacementSchema),
});
export type ForensicPromptEvaluationEvidence = typeof ForensicPromptEvaluationEvidence.Type;

export const ForensicPromptEvaluation = S.Struct({
  baselineHoldoutScorecard: ForensicScorecardSchema,
  candidateDigest: S.String,
  cleanHoldoutScorecard: ForensicScorecardSchema,
  evaluationRef: S.String,
  evaluatorIdentityRef: S.String,
  evidenceReceiptDigest: S.String,
  generatorIdentityRef: S.String,
  holdoutScorecard: ForensicScorecardSchema,
  mechanicalEvidenceRefs: S.Array(S.String),
  metricFreeze: ForensicPromptMetricFreeze,
  paretoComparisons: S.Array(ForensicPromptParetoComparison),
  paretoStatus: S.Literals(["dominates", "non_dominated", "insufficient_evidence"]),
  schema: S.Literal(FORENSIC_PROMPT_EVALUATION_VERSION),
});
export type ForensicPromptEvaluation = typeof ForensicPromptEvaluation.Type;

/**
 * `priorActivePromptDigest` and `rollbackAnchorDigest` are null exactly at
 * genesis, when no governed prompt has ever been active. They are read from the
 * durable pointer rather than supplied by the promoting caller.
 */
export const ForensicPromptActiveTransition = S.Struct({
  activePromptDigest: S.NullOr(S.String),
  /** Provenance of the candidate this transition activates or reverts. */
  candidateProducerRef: S.String,
  candidateDigest: S.String,
  decidedAt: S.String,
  evaluationRef: S.String,
  operatorDecisionRef: S.String,
  operatorIdentityRef: S.String,
  priorActivePromptDigest: S.NullOr(S.String),
  releaseGateRef: S.String,
  rollbackAnchorDigest: S.NullOr(S.String),
  schema: S.Literal(FORENSIC_PROMPT_TRANSITION_VERSION),
  sequence: S.Number,
  transitionDigest: S.String,
  transitionRef: S.String,
  transitionType: S.Literals(["activate", "rollback"]),
});
export type ForensicPromptActiveTransition = typeof ForensicPromptActiveTransition.Type;

/**
 * The durable owner-scoped governance state: the current active pointer plus
 * its append-only transition history. `revision` equals `history.length`, and
 * the active pointer always equals the last transition's active digest.
 */
export const ForensicPromptGovernanceState = S.Struct({
  activePromptDigest: S.NullOr(S.String),
  history: S.Array(ForensicPromptActiveTransition),
  ownerRef: S.String,
  revision: S.Number,
  schema: S.Literal(FORENSIC_PROMPT_GOVERNANCE_STATE_VERSION),
});
export type ForensicPromptGovernanceState = typeof ForensicPromptGovernanceState.Type;
