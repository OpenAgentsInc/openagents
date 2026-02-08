import { Context, Effect, Layer, Schema } from "effect";

import { DseExecutionBudgetsV1Schema, type DseExecutionBudgetsV1 } from "../params.js";

export type PredictReceiptV1 = {
  readonly format: "openagents.dse.predict_receipt";
  readonly formatVersion: 1;

  readonly receiptId: string;
  readonly createdAt: string;

  readonly signatureId: string;
  readonly compiled_id: string;

  readonly hashes: {
    readonly inputSchemaHash: string;
    readonly outputSchemaHash: string;
    readonly promptIrHash: string;
    readonly renderedPromptHash?: string | undefined;
    readonly paramsHash: string;
    readonly outputHash?: string | undefined;
  };

  readonly model: {
    readonly modelId?: string | undefined;
    readonly temperature?: number | undefined;
    readonly topP?: number | undefined;
    readonly maxTokens?: number | undefined;
  };

  readonly usage?:
    | {
        readonly promptTokens?: number | undefined;
        readonly completionTokens?: number | undefined;
        readonly totalTokens?: number | undefined;
      }
    | undefined;

  readonly timing: {
    readonly startedAtMs: number;
    readonly endedAtMs: number;
    readonly durationMs: number;
  };

  readonly repairCount?: number | undefined;

  readonly budget?:
    | {
        readonly limits: DseExecutionBudgetsV1;
        readonly usage: {
          readonly elapsedMs: number;
          readonly lmCalls: number;
          readonly outputChars: number;
        };
      }
    | undefined;

  readonly result:
    | { readonly _tag: "Ok" }
    | { readonly _tag: "Error"; readonly errorName: string; readonly message: string };
};

export const PredictReceiptV1Schema: Schema.Schema<PredictReceiptV1> =
  Schema.Struct({
    format: Schema.Literal("openagents.dse.predict_receipt"),
    formatVersion: Schema.Literal(1),

    receiptId: Schema.String,
    createdAt: Schema.String,

    signatureId: Schema.String,
    compiled_id: Schema.String,

    hashes: Schema.Struct({
      inputSchemaHash: Schema.String,
      outputSchemaHash: Schema.String,
      promptIrHash: Schema.String,
      renderedPromptHash: Schema.optional(Schema.String),
      paramsHash: Schema.String,
      outputHash: Schema.optional(Schema.String)
    }),

    model: Schema.Struct({
      modelId: Schema.optional(Schema.String),
      temperature: Schema.optional(Schema.Number),
      topP: Schema.optional(Schema.Number),
      maxTokens: Schema.optional(Schema.Number)
    }),

    usage: Schema.optional(
      Schema.Struct({
        promptTokens: Schema.optional(Schema.Number),
        completionTokens: Schema.optional(Schema.Number),
        totalTokens: Schema.optional(Schema.Number)
      })
    ),

    timing: Schema.Struct({
      startedAtMs: Schema.Number,
      endedAtMs: Schema.Number,
      durationMs: Schema.Number
    }),

    repairCount: Schema.optional(Schema.Number),

    budget: Schema.optional(
      Schema.Struct({
        limits: DseExecutionBudgetsV1Schema,
        usage: Schema.Struct({
          elapsedMs: Schema.Number,
          lmCalls: Schema.Number,
          outputChars: Schema.Number
        })
      })
    ),

    result: Schema.Union(
      Schema.Struct({ _tag: Schema.Literal("Ok") }),
      Schema.Struct({
        _tag: Schema.Literal("Error"),
        errorName: Schema.String,
        message: Schema.String
      })
    )
  });

export type Receipt = PredictReceiptV1;

export class ReceiptRecorderError extends Schema.TaggedError<ReceiptRecorderError>()(
  "ReceiptRecorderError",
  {
    message: Schema.String,
    cause: Schema.optional(Schema.Defect)
  }
) {}

export type ReceiptRecorder = {
  readonly record: (receipt: Receipt) => Effect.Effect<void, ReceiptRecorderError>;
};

export class ReceiptRecorderService extends Context.Tag(
  "@openagentsinc/dse/ReceiptRecorder"
)<ReceiptRecorderService, ReceiptRecorder>() {}

export function layerNoop(): Layer.Layer<ReceiptRecorderService> {
  return Layer.succeed(
    ReceiptRecorderService,
    ReceiptRecorderService.of({ record: () => Effect.void })
  );
}

export function makeInMemory(): {
  readonly layer: Layer.Layer<ReceiptRecorderService>;
  readonly getReceipts: () => ReadonlyArray<Receipt>;
} {
  const receipts: Array<Receipt> = [];
  return {
    layer: Layer.succeed(
      ReceiptRecorderService,
      ReceiptRecorderService.of({
        record: (receipt) => Effect.sync(() => void receipts.push(receipt))
      })
    ),
    getReceipts: () => receipts
  };
}
