import { Schema as S } from "effect";
import { ProbeLlmProviderMetadata } from "./usage.js";

export const ProbeLlmCacheHint = S.Struct({
  type: S.Literals(["ephemeral", "persistent"]),
  ttlSeconds: S.optional(S.Number),
});
export type ProbeLlmCacheHint = typeof ProbeLlmCacheHint.Type;

export const ProbeLlmTextPart = S.Struct({
  type: S.Literal("text"),
  text: S.String,
  cache: S.optional(ProbeLlmCacheHint),
  metadata: S.optional(S.Record(S.String, S.Unknown)),
  providerMetadata: S.optional(ProbeLlmProviderMetadata),
});
export type ProbeLlmTextPart = typeof ProbeLlmTextPart.Type;

export const ProbeLlmMediaPart = S.Struct({
  type: S.Literal("media"),
  mediaType: S.String,
  data: S.Union([S.String, S.Uint8Array]),
  filename: S.optional(S.String),
  metadata: S.optional(S.Record(S.String, S.Unknown)),
});
export type ProbeLlmMediaPart = typeof ProbeLlmMediaPart.Type;

export const ProbeLlmReasoningPart = S.Struct({
  type: S.Literal("reasoning"),
  text: S.String,
  metadata: S.optional(S.Record(S.String, S.Unknown)),
  providerMetadata: S.optional(ProbeLlmProviderMetadata),
});
export type ProbeLlmReasoningPart = typeof ProbeLlmReasoningPart.Type;

export const ProbeLlmToolResultValue = S.Union([
  S.Struct({ type: S.Literal("json"), value: S.Unknown }),
  S.Struct({ type: S.Literal("text"), value: S.String }),
  S.Struct({ type: S.Literal("error"), value: S.String }),
]);
export type ProbeLlmToolResultValue = typeof ProbeLlmToolResultValue.Type;

export function makeProbeLlmToolResultValue(value: unknown, type: ProbeLlmToolResultValue["type"] = "json"): ProbeLlmToolResultValue {
  if (isProbeLlmToolResultValue(value)) {
    return value;
  }

  if (type === "text") {
    return { type, value: stringifyToolResult(value) };
  }

  if (type === "error") {
    return { type, value: stringifyToolResult(value) };
  }

  return { type, value };
}

export function stringifyToolResult(value: unknown): string {
  if (typeof value === "string") {
    return value;
  }

  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
}

export const ProbeLlmToolCallPart = S.Struct({
  type: S.Literal("tool-call"),
  id: S.String,
  name: S.String,
  input: S.Unknown,
  providerExecuted: S.optional(S.Boolean),
  metadata: S.optional(S.Record(S.String, S.Unknown)),
  providerMetadata: S.optional(ProbeLlmProviderMetadata),
});
export type ProbeLlmToolCallPart = typeof ProbeLlmToolCallPart.Type;

export const ProbeLlmToolResultPart = S.Struct({
  type: S.Literal("tool-result"),
  id: S.String,
  name: S.String,
  result: ProbeLlmToolResultValue,
  providerExecuted: S.optional(S.Boolean),
  cache: S.optional(ProbeLlmCacheHint),
  metadata: S.optional(S.Record(S.String, S.Unknown)),
  providerMetadata: S.optional(ProbeLlmProviderMetadata),
});
export type ProbeLlmToolResultPart = typeof ProbeLlmToolResultPart.Type;

export const ProbeLlmContentPart = S.Union([
  ProbeLlmTextPart,
  ProbeLlmMediaPart,
  ProbeLlmReasoningPart,
  ProbeLlmToolCallPart,
  ProbeLlmToolResultPart,
]);
export type ProbeLlmContentPart = typeof ProbeLlmContentPart.Type;

export const ProbeLlmMessageRole = S.Literals(["system", "user", "assistant", "tool"]);
export type ProbeLlmMessageRole = typeof ProbeLlmMessageRole.Type;

export const ProbeLlmMessage = S.Struct({
  id: S.optional(S.String),
  role: ProbeLlmMessageRole,
  content: S.Array(ProbeLlmContentPart),
  metadata: S.optional(S.Record(S.String, S.Unknown)),
});
export type ProbeLlmMessage = typeof ProbeLlmMessage.Type;

export type ProbeLlmMessageInput = string | ProbeLlmContentPart | ReadonlyArray<ProbeLlmContentPart>;

export function makeProbeLlmTextPart(text: string): ProbeLlmTextPart {
  return { type: "text", text };
}

export function makeProbeLlmMessage(role: ProbeLlmMessageRole, content: ProbeLlmMessageInput): ProbeLlmMessage {
  return {
    role,
    content: normalizeContent(content),
  };
}

export function makeProbeLlmToolResult(input: {
  readonly id: string;
  readonly name: string;
  readonly result: unknown;
  readonly resultType?: ProbeLlmToolResultValue["type"];
}): ProbeLlmToolResultPart {
  return {
    type: "tool-result",
    id: input.id,
    name: input.name,
    result: makeProbeLlmToolResultValue(input.result, input.resultType),
  };
}

function normalizeContent(content: ProbeLlmMessageInput): ReadonlyArray<ProbeLlmContentPart> {
  if (typeof content === "string") {
    return [makeProbeLlmTextPart(content)];
  }

  // `Array.isArray` types its argument as `any[]`, which does not narrow a
  // `ReadonlyArray<...>` out of the union, so guard on the readonly array shape
  // explicitly to keep both branches typed as ProbeLlmContentPart[].
  if (Array.isArray(content)) {
    return content as ReadonlyArray<ProbeLlmContentPart>;
  }

  return [content as ProbeLlmContentPart];
}

function isProbeLlmToolResultValue(value: unknown): value is ProbeLlmToolResultValue {
  return (
    typeof value === "object" &&
    value !== null &&
    "type" in value &&
    (value.type === "json" || value.type === "text" || value.type === "error") &&
    "value" in value
  );
}
