import { Schema } from "effect";

import { sha256IdFromCanonicalJson } from "../hashes.js";
import { DseExecutionBudgetsV1Schema, type DseExecutionBudgetsV1 } from "../params.js";

export type InstructionVariantV1 = {
  readonly id: string;
  readonly text: string;
};

export const InstructionVariantV1Schema: Schema.Schema<InstructionVariantV1> =
  Schema.Struct({
    id: Schema.String,
    text: Schema.String
  });

export type FewShotSearchSpaceV1 = {
  readonly candidateExampleIds: ReadonlyArray<string>;
  readonly kMax: number;
};

export const FewShotSearchSpaceV1Schema: Schema.Schema<FewShotSearchSpaceV1> =
  Schema.Struct({
    candidateExampleIds: Schema.Array(Schema.String),
    kMax: Schema.Number
  });

export type StrategyVariantV1 = {
  readonly id: string;
  readonly strategyId: string;
};

export const StrategyVariantV1Schema: Schema.Schema<StrategyVariantV1> =
  Schema.Struct({
    id: Schema.String,
    strategyId: Schema.String
  });

export type BudgetProfileV1 = {
  readonly id: string;
  readonly budgets: DseExecutionBudgetsV1;
};

export const BudgetProfileV1Schema: Schema.Schema<BudgetProfileV1> =
  Schema.Struct({
    id: Schema.String,
    budgets: DseExecutionBudgetsV1Schema
  });

export type ChunkingPolicyVariantV1 = {
  readonly id: string;
  readonly chunkChars: number;
  readonly overlapChars?: number | undefined;
  readonly maxChunks?: number | undefined;
};

export const ChunkingPolicyVariantV1Schema: Schema.Schema<ChunkingPolicyVariantV1> =
  Schema.Struct({
    id: Schema.String,
    chunkChars: Schema.Number,
    overlapChars: Schema.optional(Schema.Number),
    maxChunks: Schema.optional(Schema.Number)
  });

export type SubRoleVariantV1 = {
  readonly id: string;
  readonly subRole: "sub" | "main";
};

export const SubRoleVariantV1Schema: Schema.Schema<SubRoleVariantV1> =
  Schema.Struct({
    id: Schema.String,
    subRole: Schema.Literal("sub", "main")
  });

export type CompileSearchSpaceV1 = {
  // Strategy selection (direct vs RLM vs distilled).
  readonly strategyVariants?: ReadonlyArray<StrategyVariantV1> | undefined;

  readonly instructionVariants?: ReadonlyArray<InstructionVariantV1> | undefined;
  readonly fewShot?: FewShotSearchSpaceV1 | undefined;

  // RLM-lite controller knobs (artifact-pinnable).
  readonly rlmControllerInstructionVariants?: ReadonlyArray<InstructionVariantV1> | undefined;
  readonly rlmChunkingPolicyVariants?: ReadonlyArray<ChunkingPolicyVariantV1> | undefined;
  readonly rlmSubRoleVariants?: ReadonlyArray<SubRoleVariantV1> | undefined;

  // Budget profiles to evaluate as part of compilation (artifact-pinnable).
  readonly budgetProfiles?: ReadonlyArray<BudgetProfileV1> | undefined;
};

export const CompileSearchSpaceV1Schema: Schema.Schema<CompileSearchSpaceV1> =
  Schema.Struct({
    strategyVariants: Schema.optional(Schema.Array(StrategyVariantV1Schema)),
    instructionVariants: Schema.optional(Schema.Array(InstructionVariantV1Schema)),
    fewShot: Schema.optional(FewShotSearchSpaceV1Schema),
    rlmControllerInstructionVariants: Schema.optional(Schema.Array(InstructionVariantV1Schema)),
    rlmChunkingPolicyVariants: Schema.optional(Schema.Array(ChunkingPolicyVariantV1Schema)),
    rlmSubRoleVariants: Schema.optional(Schema.Array(SubRoleVariantV1Schema)),
    budgetProfiles: Schema.optional(Schema.Array(BudgetProfileV1Schema))
  });

export type CompileOptimizerV1 = {
  readonly id:
    | "instruction_grid.v1"
    | "fewshot_greedy_forward.v1"
    | "joint_instruction_grid_then_fewshot_greedy_forward.v1"
    | "knobs_grid.v1"
    | "knobs_grid_refine.v1";
  readonly config?: unknown | undefined;
};

export const CompileOptimizerV1Schema: Schema.Schema<CompileOptimizerV1> =
  Schema.Struct({
    id: Schema.Literal(
      "instruction_grid.v1",
      "fewshot_greedy_forward.v1",
      "joint_instruction_grid_then_fewshot_greedy_forward.v1",
      "knobs_grid.v1",
      "knobs_grid_refine.v1"
    ),
    config: Schema.optional(Schema.Unknown)
  });

export type CompileJobSpecV1 = {
  readonly format: "openagents.dse.compile_job";
  readonly formatVersion: 1;

  readonly signatureId: string;
  readonly datasetId: string;
  readonly metricId: string;

  readonly searchSpace: CompileSearchSpaceV1;
  readonly optimizer: CompileOptimizerV1;
};

export const CompileJobSpecV1Schema: Schema.Schema<CompileJobSpecV1> =
  Schema.Struct({
    format: Schema.Literal("openagents.dse.compile_job"),
    formatVersion: Schema.Literal(1),

    signatureId: Schema.String,
    datasetId: Schema.String,
    metricId: Schema.String,

    searchSpace: CompileSearchSpaceV1Schema,
    optimizer: CompileOptimizerV1Schema
  });

export function compileJobHash(job: CompileJobSpecV1) {
  return sha256IdFromCanonicalJson(job);
}
