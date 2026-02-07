import { Schema } from "effect";

import { DseParamsV1Schema, type DseParamsV1 } from "./params.js";

export type EvalSummaryV1 = {
  readonly evalVersion: 1;
  readonly kind: "unscored" | "scored";
  readonly reward?: number | undefined;
  readonly notes?: string | undefined;
  readonly datasetId?: string | undefined;
  readonly datasetHash?: string | undefined;
  readonly metricId?: string | undefined;
  readonly metricVersion?: number | undefined;
  readonly n?: number | undefined;
  readonly seed?: number | undefined;
  readonly selectedExampleIdsHash?: string | undefined;
};

export const EvalSummaryV1Schema: Schema.Schema<EvalSummaryV1> = Schema.Struct({
  evalVersion: Schema.Literal(1),
  kind: Schema.Literal("unscored", "scored"),
  reward: Schema.optional(Schema.Number),
  notes: Schema.optional(Schema.String),
  datasetId: Schema.optional(Schema.String),
  datasetHash: Schema.optional(Schema.String),
  metricId: Schema.optional(Schema.String),
  metricVersion: Schema.optional(Schema.Number),
  n: Schema.optional(Schema.Number),
  seed: Schema.optional(Schema.Number),
  selectedExampleIdsHash: Schema.optional(Schema.String)
});

export type DseCompiledArtifactV1 = {
  readonly format: "openagents.dse.compiled_artifact";
  readonly formatVersion: 1;

  readonly signatureId: string;
  readonly compiled_id: string;
  readonly createdAt: string;

  readonly hashes: {
    readonly inputSchemaHash: string;
    readonly outputSchemaHash: string;
    readonly promptIrHash: string;
    readonly paramsHash: string;
  };

  readonly params: DseParamsV1;

  readonly eval: EvalSummaryV1;

  readonly optimizer: {
    readonly id: string;
    readonly config?: unknown | undefined;
    readonly iterations?: number | undefined;
  };

  readonly provenance: {
    readonly compilerVersion?: string | undefined;
    readonly gitSha?: string | undefined;
    readonly datasetId?: string | undefined;
    readonly datasetHash?: string | undefined;
    readonly metricId?: string | undefined;
    readonly searchSpaceHash?: string | undefined;
  };

  readonly compatibility?:
    | {
        readonly requiredTools?: ReadonlyArray<string> | undefined;
        readonly requiredLanes?: ReadonlyArray<string> | undefined;
        readonly privacyModesAllowed?: ReadonlyArray<string> | undefined;
      }
    | undefined;
};

export const DseCompiledArtifactV1Schema: Schema.Schema<DseCompiledArtifactV1> =
  Schema.Struct({
    format: Schema.Literal("openagents.dse.compiled_artifact"),
    formatVersion: Schema.Literal(1),

    signatureId: Schema.String,
    compiled_id: Schema.String,
    createdAt: Schema.String,

    hashes: Schema.Struct({
      inputSchemaHash: Schema.String,
      outputSchemaHash: Schema.String,
      promptIrHash: Schema.String,
      paramsHash: Schema.String
    }),

    params: DseParamsV1Schema,

    eval: EvalSummaryV1Schema,

    optimizer: Schema.Struct({
      id: Schema.String,
      config: Schema.optional(Schema.Unknown),
      iterations: Schema.optional(Schema.Number)
    }),

    provenance: Schema.Struct({
      compilerVersion: Schema.optional(Schema.String),
      gitSha: Schema.optional(Schema.String),
      datasetId: Schema.optional(Schema.String),
      datasetHash: Schema.optional(Schema.String),
      metricId: Schema.optional(Schema.String),
      searchSpaceHash: Schema.optional(Schema.String)
    }),

    compatibility: Schema.optional(
      Schema.Struct({
        requiredTools: Schema.optional(Schema.Array(Schema.String)),
        requiredLanes: Schema.optional(Schema.Array(Schema.String)),
        privacyModesAllowed: Schema.optional(Schema.Array(Schema.String))
      })
    )
  });
