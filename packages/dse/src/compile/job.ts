import { Schema } from "effect";

import { sha256IdFromCanonicalJson } from "../hashes.js";

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

export type CompileSearchSpaceV1 = {
  readonly instructionVariants?: ReadonlyArray<InstructionVariantV1> | undefined;
  readonly fewShot?: FewShotSearchSpaceV1 | undefined;
};

export const CompileSearchSpaceV1Schema: Schema.Schema<CompileSearchSpaceV1> =
  Schema.Struct({
    instructionVariants: Schema.optional(Schema.Array(InstructionVariantV1Schema)),
    fewShot: Schema.optional(FewShotSearchSpaceV1Schema)
  });

export type CompileOptimizerV1 = {
  readonly id:
    | "instruction_grid.v1"
    | "fewshot_greedy_forward.v1"
    | "joint_instruction_grid_then_fewshot_greedy_forward.v1";
  readonly config?: unknown | undefined;
};

export const CompileOptimizerV1Schema: Schema.Schema<CompileOptimizerV1> =
  Schema.Struct({
    id: Schema.Literal(
      "instruction_grid.v1",
      "fewshot_greedy_forward.v1",
      "joint_instruction_grid_then_fewshot_greedy_forward.v1"
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

