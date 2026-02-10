import { Schema } from "effect";

export type RlmTraceDocV1 = {
  readonly format: "openagents.dse.rlm_trace";
  readonly formatVersion: 1;

  // Optional for backward compatibility (older traces may omit these fields).
  readonly signatureId?: string | undefined;
  readonly receiptId?: string | undefined;
  readonly strategyId?: string | undefined;

  readonly events: ReadonlyArray<unknown>;
};

export const RlmTraceDocV1Schema: Schema.Schema<RlmTraceDocV1> = Schema.Struct({
  format: Schema.Literal("openagents.dse.rlm_trace"),
  formatVersion: Schema.Literal(1),
  signatureId: Schema.optional(Schema.String),
  receiptId: Schema.optional(Schema.String),
  strategyId: Schema.optional(Schema.String),
  events: Schema.Array(Schema.Unknown)
});

