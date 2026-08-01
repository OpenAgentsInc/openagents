import { Schema as S } from "effect";

import {
  ForensicPromptArtifactSchema,
  ForensicScorecardSchema,
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

export const ForensicPromptMetricFreeze = S.Struct({
  censoringDefinitionDigest: S.String,
  eligibilityDefinitionDigest: S.String,
  frozenAt: S.String,
  metricDefinitionRevisionDigest: S.String,
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

export const ForensicPromptEvaluation = S.Struct({
  baselineHoldoutScorecard: ForensicScorecardSchema,
  candidateDigest: S.String,
  cleanHoldoutScorecard: ForensicScorecardSchema,
  evaluatorIdentityRef: S.String,
  freshSourceStateRefs: S.Array(S.String),
  freshWorkerPlacementRefs: S.Array(S.String),
  generatorIdentityRef: S.String,
  holdoutScorecard: ForensicScorecardSchema,
  mechanicalEvidenceRefs: S.Array(S.String),
  metricFreeze: ForensicPromptMetricFreeze,
  paretoStatus: S.Literals(["dominates", "non_dominated"]),
  schema: S.Literal(FORENSIC_PROMPT_EVALUATION_VERSION),
});
export type ForensicPromptEvaluation = typeof ForensicPromptEvaluation.Type;

export const ForensicPromptActiveTransition = S.Struct({
  activePromptDigest: S.String,
  candidateDigest: S.String,
  decidedAt: S.String,
  evaluationRef: S.String,
  operatorDecisionRef: S.String,
  operatorIdentityRef: S.String,
  priorActivePromptDigest: S.String,
  releaseGateRef: S.String,
  rollbackAnchorDigest: S.String,
  schema: S.Literal(FORENSIC_PROMPT_TRANSITION_VERSION),
  transitionRef: S.String,
  transitionType: S.Literals(["activate", "rollback"]),
});
export type ForensicPromptActiveTransition = typeof ForensicPromptActiveTransition.Type;
