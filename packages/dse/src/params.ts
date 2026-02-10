import { Schema } from "effect";

export type DseModelConfigV1 = {
  readonly modelId?: string | undefined;
  readonly temperature?: number | undefined;
  readonly topP?: number | undefined;
  readonly maxTokens?: number | undefined;
};

export const DseModelConfigV1Schema: Schema.Schema<DseModelConfigV1> = Schema.Struct({
  modelId: Schema.optional(Schema.String),
  temperature: Schema.optional(Schema.Number),
  topP: Schema.optional(Schema.Number),
  maxTokens: Schema.optional(Schema.Number)
});

export type DseModelRolesV1 = {
  readonly main?: DseModelConfigV1 | undefined;
  readonly sub?: DseModelConfigV1 | undefined;
  readonly repair?: DseModelConfigV1 | undefined;
  readonly judge?: DseModelConfigV1 | undefined;
};

export const DseModelRolesV1Schema: Schema.Schema<DseModelRolesV1> = Schema.Struct({
  main: Schema.optional(DseModelConfigV1Schema),
  sub: Schema.optional(DseModelConfigV1Schema),
  repair: Schema.optional(DseModelConfigV1Schema),
  judge: Schema.optional(DseModelConfigV1Schema)
});

export type RlmLiteChunkDefaultsV1 = {
  readonly chunkChars: number;
  readonly overlapChars?: number | undefined;
  readonly maxChunks?: number | undefined;
};

export const RlmLiteChunkDefaultsV1Schema: Schema.Schema<RlmLiteChunkDefaultsV1> =
  Schema.Struct({
    chunkChars: Schema.Number,
    overlapChars: Schema.optional(Schema.Number),
    maxChunks: Schema.optional(Schema.Number)
  });

export type RlmLiteConfigV1 = {
  readonly controllerInstructions?: string | undefined;
  readonly extractionSystem?: string | undefined;
  readonly chunkDefaults?: RlmLiteChunkDefaultsV1 | undefined;
  // Controls whether kernel-driven sub-LM calls use modelRoles.sub vs modelRoles.main.
  readonly subRole?: "sub" | "main" | undefined;
};

export const RlmLiteConfigV1Schema: Schema.Schema<RlmLiteConfigV1> =
  Schema.Struct({
    controllerInstructions: Schema.optional(Schema.String),
    extractionSystem: Schema.optional(Schema.String),
    chunkDefaults: Schema.optional(RlmLiteChunkDefaultsV1Schema),
    subRole: Schema.optional(Schema.Literal("sub", "main"))
  });

export type DseExecutionBudgetsV1 = {
  readonly maxTimeMs?: number | undefined;
  readonly maxLmCalls?: number | undefined;
  readonly maxToolCalls?: number | undefined;
  readonly maxRlmIterations?: number | undefined;
  readonly maxSubLmCalls?: number | undefined;
  readonly maxOutputChars?: number | undefined;
};

export const DseExecutionBudgetsV1Schema: Schema.Schema<DseExecutionBudgetsV1> =
  Schema.Struct({
    maxTimeMs: Schema.optional(Schema.Number),
    maxLmCalls: Schema.optional(Schema.Number),
    maxToolCalls: Schema.optional(Schema.Number),
    maxRlmIterations: Schema.optional(Schema.Number),
    maxSubLmCalls: Schema.optional(Schema.Number),
    maxOutputChars: Schema.optional(Schema.Number)
  });

export type DseParamsV1 = {
  readonly paramsVersion: 1;

  // Inference strategy selection (pinned by compiled artifact).
  // - "direct.v1": single LLM call (+ optional repair)
  // - "rlm_lite.v1": RLM-style loop (Phase C)
  readonly strategy?:
    | {
        readonly id: string;
        readonly config?: unknown | undefined;
      }
    | undefined;

  readonly instruction?:
    | {
        readonly text?: string | undefined;
      }
    | undefined;

  readonly fewShot?:
    | {
        readonly exampleIds: ReadonlyArray<string>;
        readonly k?: number | undefined;
      }
    | undefined;

  readonly model?:
    | DseModelConfigV1
    | undefined;

  // Optional role-based model config. When provided, it overrides the base `model` for that role.
  readonly modelRoles?: DseModelRolesV1 | undefined;

  readonly decode?:
    | {
        readonly mode: "strict_json" | "jsonish";
        readonly maxRepairs?: number | undefined;
      }
    | undefined;

  readonly tools?:
    | {
        readonly allowedToolNames?: ReadonlyArray<string> | undefined;
        readonly maxToolCalls?: number | undefined;
        readonly timeoutMsByToolName?: Readonly<Record<string, number>> | undefined;
      }
    | undefined;

  // Optional config for RLM-lite strategy behavior (controller instructions, chunking hints, etc).
  readonly rlmLite?: RlmLiteConfigV1 | undefined;

  readonly budgets?: DseExecutionBudgetsV1 | undefined;
};

export type DseParams = DseParamsV1;

export const emptyParamsV1: DseParamsV1 = { paramsVersion: 1 };

export const DseParamsV1Schema: Schema.Schema<DseParamsV1> = Schema.Struct({
  paramsVersion: Schema.Literal(1),
  strategy: Schema.optional(
    Schema.Struct({
      id: Schema.String,
      config: Schema.optional(Schema.Unknown)
    })
  ),
  instruction: Schema.optional(
    Schema.Struct({
      text: Schema.optional(Schema.String)
    })
  ),
  fewShot: Schema.optional(
    Schema.Struct({
      exampleIds: Schema.Array(Schema.String),
      k: Schema.optional(Schema.Number)
    })
  ),
  model: Schema.optional(
    DseModelConfigV1Schema
  ),
  modelRoles: Schema.optional(DseModelRolesV1Schema),
  decode: Schema.optional(
    Schema.Struct({
      mode: Schema.Literal("strict_json", "jsonish"),
      maxRepairs: Schema.optional(Schema.Number)
    })
  ),
  tools: Schema.optional(
    Schema.Struct({
      allowedToolNames: Schema.optional(Schema.Array(Schema.String)),
      maxToolCalls: Schema.optional(Schema.Number),
      timeoutMsByToolName: Schema.optional(
        Schema.Record({ key: Schema.String, value: Schema.Number })
      )
    })
  ),
  rlmLite: Schema.optional(RlmLiteConfigV1Schema),
  budgets: Schema.optional(DseExecutionBudgetsV1Schema)
});
