import { Either, Schema } from "effect";

import {
  RlmActionV1Schema,
  RlmObservationV1Schema,
  type RlmActionV1,
  type RlmObservationV1
} from "../runtime/rlmKernel.js";

export type RlmTraceInputEventV1 = {
  readonly _tag: "Input";
  readonly input: unknown;
};

export type RlmTraceActionEventV1 = {
  readonly iteration: number;
  // Older traces may omit prompt hashes; keep optional for compatibility.
  readonly promptHash?: string | undefined;
  readonly action: RlmActionV1;
};

export type RlmTraceObservationEventV1 = {
  readonly iteration: number;
  readonly observation: RlmObservationV1;
};

export type RlmTraceFinalEventV1 = {
  readonly _tag: "Final";
  readonly output: unknown;
  readonly iteration?: number | undefined;
  readonly promptHash?: string | undefined;
};

export type RlmTraceEventV1 =
  | RlmTraceInputEventV1
  | RlmTraceActionEventV1
  | RlmTraceObservationEventV1
  | RlmTraceFinalEventV1;

export const RlmTraceEventV1Schema: Schema.Schema<RlmTraceEventV1> = Schema.Union(
  Schema.Struct({
    _tag: Schema.Literal("Input"),
    input: Schema.Unknown
  }),
  Schema.Struct({
    iteration: Schema.Number,
    promptHash: Schema.optional(Schema.String),
    action: RlmActionV1Schema
  }),
  Schema.Struct({
    iteration: Schema.Number,
    observation: RlmObservationV1Schema
  }),
  Schema.Struct({
    _tag: Schema.Literal("Final"),
    output: Schema.Unknown,
    iteration: Schema.optional(Schema.Number),
    promptHash: Schema.optional(Schema.String)
  })
);

export type RlmTraceDocV1 = {
  readonly format: "openagents.dse.rlm_trace";
  readonly formatVersion: 1;

  // Optional for backward compatibility (older traces may omit these fields).
  readonly signatureId?: string | undefined;
  readonly receiptId?: string | undefined;
  readonly strategyId?: string | undefined;

  readonly events: ReadonlyArray<RlmTraceEventV1>;
};

export const RlmTraceDocV1Schema: Schema.Schema<RlmTraceDocV1> = Schema.Struct({
  format: Schema.Literal("openagents.dse.rlm_trace"),
  formatVersion: Schema.Literal(1),
  signatureId: Schema.optional(Schema.String),
  receiptId: Schema.optional(Schema.String),
  strategyId: Schema.optional(Schema.String),
  events: Schema.Array(RlmTraceEventV1Schema)
});

type LegacyRlmTraceDocEnvelope = {
  readonly format: "openagents.dse.rlm_trace";
  readonly formatVersion: number;
  readonly signatureId?: string | undefined;
  readonly receiptId?: string | undefined;
  readonly strategyId?: string | undefined;
  readonly events: ReadonlyArray<unknown>;
};

const LegacyRlmTraceDocEnvelopeSchema: Schema.Schema<LegacyRlmTraceDocEnvelope> =
  Schema.Struct({
    format: Schema.Literal("openagents.dse.rlm_trace"),
    formatVersion: Schema.Number,
    signatureId: Schema.optional(Schema.String),
    receiptId: Schema.optional(Schema.String),
    strategyId: Schema.optional(Schema.String),
    events: Schema.Array(Schema.Unknown)
  });

const decodeTraceEventEither = Schema.decodeUnknownEither(RlmTraceEventV1Schema);
const decodeActionEither = Schema.decodeUnknownEither(RlmActionV1Schema);
const decodeObservationEither = Schema.decodeUnknownEither(RlmObservationV1Schema);

function normalizeLegacyEvent(event: unknown): RlmTraceEventV1 | null {
  const typed = decodeTraceEventEither(event);
  if (Either.isRight(typed)) {
    return typed.right;
  }

  if (!event || typeof event !== "object") {
    return null;
  }

  const raw = event as Record<string, unknown>;

  // Legacy traces sometimes persisted terminal output as a top-level event.
  if (raw._tag === "Final" && "output" in raw) {
    return {
      _tag: "Final",
      output: raw.output,
      ...(typeof raw.iteration === "number" ? { iteration: raw.iteration } : {}),
      ...(typeof raw.promptHash === "string" ? { promptHash: raw.promptHash } : {})
    };
  }

  const iteration =
    typeof raw.iteration === "number" && Number.isFinite(raw.iteration)
      ? raw.iteration
      : null;

  if (iteration == null) {
    return null;
  }

  if ("action" in raw) {
    const action = decodeActionEither(raw.action);
    if (Either.isRight(action)) {
      return {
        iteration,
        ...(typeof raw.promptHash === "string" ? { promptHash: raw.promptHash } : {}),
        action: action.right
      };
    }
  }

  if ("observation" in raw) {
    const observation = decodeObservationEither(raw.observation);
    if (Either.isRight(observation)) {
      return {
        iteration,
        observation: observation.right
      };
    }
  }

  return null;
}

/**
 * Decodes the canonical v1 trace format and applies a compatibility path for older
 * v1 traces that used looser event shapes.
 */
export function decodeRlmTraceDocV1CompatibleSync(value: unknown): RlmTraceDocV1 {
  const strict = Schema.decodeUnknownEither(RlmTraceDocV1Schema)(value);
  if (Either.isRight(strict)) {
    return strict.right;
  }

  const envelope = Schema.decodeUnknownSync(LegacyRlmTraceDocEnvelopeSchema)(value);

  if (envelope.formatVersion !== 1) {
    throw new Error(
      `Unsupported rlm trace formatVersion=${String(envelope.formatVersion)} (expected 1)`
    );
  }

  const normalizedEvents = envelope.events.flatMap((event) => {
    const normalized = normalizeLegacyEvent(event);
    return normalized ? [normalized] : [];
  });

  return {
    format: "openagents.dse.rlm_trace",
    formatVersion: 1,
    ...(typeof envelope.signatureId === "string"
      ? { signatureId: envelope.signatureId }
      : {}),
    ...(typeof envelope.receiptId === "string"
      ? { receiptId: envelope.receiptId }
      : {}),
    ...(typeof envelope.strategyId === "string"
      ? { strategyId: envelope.strategyId }
      : {}),
    events: normalizedEvents
  };
}
