import { Schema } from "effect";

export type DseParamsV1 = {
  readonly paramsVersion: 1;

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
    | {
        readonly modelId?: string | undefined;
        readonly temperature?: number | undefined;
        readonly topP?: number | undefined;
        readonly maxTokens?: number | undefined;
      }
    | undefined;

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
};

export type DseParams = DseParamsV1;

export const emptyParamsV1: DseParamsV1 = { paramsVersion: 1 };

export const DseParamsV1Schema: Schema.Schema<DseParamsV1> = Schema.Struct({
  paramsVersion: Schema.Literal(1),
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
    Schema.Struct({
      modelId: Schema.optional(Schema.String),
      temperature: Schema.optional(Schema.Number),
      topP: Schema.optional(Schema.Number),
      maxTokens: Schema.optional(Schema.Number)
    })
  ),
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
  )
});
