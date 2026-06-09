import { Schema as S } from "effect";
import { ProbeLlmToolResultValue } from "./messages";
import { ProbeLlmUsage, ProbeLlmProviderMetadata } from "./usage";

export const ProbeLlmFinishReason = S.Literals(["stop", "length", "tool_calls", "content_filter", "error", "unknown"]);
export type ProbeLlmFinishReason = typeof ProbeLlmFinishReason.Type;

export const ProbeLlmEvent = S.Union([
  S.Struct({
    type: S.Literal("step-start"),
    index: S.Number,
  }),
  S.Struct({
    type: S.Literal("text-delta"),
    id: S.String,
    text: S.String,
    providerMetadata: S.optional(ProbeLlmProviderMetadata),
  }),
  S.Struct({
    type: S.Literal("reasoning-delta"),
    id: S.String,
    text: S.String,
    providerMetadata: S.optional(ProbeLlmProviderMetadata),
  }),
  S.Struct({
    type: S.Literal("tool-call"),
    id: S.String,
    name: S.String,
    input: S.Unknown,
    providerExecuted: S.optional(S.Boolean),
    providerMetadata: S.optional(ProbeLlmProviderMetadata),
  }),
  S.Struct({
    type: S.Literal("tool-result"),
    id: S.String,
    name: S.String,
    result: ProbeLlmToolResultValue,
    providerExecuted: S.optional(S.Boolean),
    providerMetadata: S.optional(ProbeLlmProviderMetadata),
  }),
  S.Struct({
    type: S.Literal("tool-error"),
    id: S.String,
    name: S.String,
    message: S.String,
    providerMetadata: S.optional(ProbeLlmProviderMetadata),
  }),
  S.Struct({
    type: S.Literal("provider-error"),
    message: S.String,
    retryable: S.optional(S.Boolean),
    providerMetadata: S.optional(ProbeLlmProviderMetadata),
  }),
  S.Struct({
    type: S.Literal("step-finish"),
    index: S.Number,
    reason: ProbeLlmFinishReason,
    usage: S.optional(ProbeLlmUsage),
    providerMetadata: S.optional(ProbeLlmProviderMetadata),
  }),
  S.Struct({
    type: S.Literal("finish"),
    reason: ProbeLlmFinishReason,
    usage: S.optional(ProbeLlmUsage),
    providerMetadata: S.optional(ProbeLlmProviderMetadata),
  }),
]);
export type ProbeLlmEvent = typeof ProbeLlmEvent.Type;

export const ProbeLlmEvents = {
  stepStart: (index: number): ProbeLlmEvent => ({ type: "step-start", index }),
  textDelta: (input: Omit<Extract<ProbeLlmEvent, { readonly type: "text-delta" }>, "type">): ProbeLlmEvent => ({
    type: "text-delta",
    ...input,
  }),
  reasoningDelta: (
    input: Omit<Extract<ProbeLlmEvent, { readonly type: "reasoning-delta" }>, "type">,
  ): ProbeLlmEvent => ({
    type: "reasoning-delta",
    ...input,
  }),
  toolCall: (input: Omit<Extract<ProbeLlmEvent, { readonly type: "tool-call" }>, "type">): ProbeLlmEvent => ({
    type: "tool-call",
    ...input,
  }),
  toolResult: (input: Omit<Extract<ProbeLlmEvent, { readonly type: "tool-result" }>, "type">): ProbeLlmEvent => ({
    type: "tool-result",
    ...input,
  }),
  toolError: (input: Omit<Extract<ProbeLlmEvent, { readonly type: "tool-error" }>, "type">): ProbeLlmEvent => ({
    type: "tool-error",
    ...input,
  }),
  providerError: (input: Omit<Extract<ProbeLlmEvent, { readonly type: "provider-error" }>, "type">): ProbeLlmEvent => ({
    type: "provider-error",
    ...input,
  }),
  stepFinish: (input: Omit<Extract<ProbeLlmEvent, { readonly type: "step-finish" }>, "type">): ProbeLlmEvent => ({
    type: "step-finish",
    ...input,
  }),
  finish: (input: Omit<Extract<ProbeLlmEvent, { readonly type: "finish" }>, "type">): ProbeLlmEvent => ({
    type: "finish",
    ...input,
  }),
  isToolCall: (event: ProbeLlmEvent): event is Extract<ProbeLlmEvent, { readonly type: "tool-call" }> =>
    event.type === "tool-call",
  isToolResult: (event: ProbeLlmEvent): event is Extract<ProbeLlmEvent, { readonly type: "tool-result" }> =>
    event.type === "tool-result",
  isToolError: (event: ProbeLlmEvent): event is Extract<ProbeLlmEvent, { readonly type: "tool-error" }> =>
    event.type === "tool-error",
};
