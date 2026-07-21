import { Schema as S } from "effect";

/**
 * `@openagentsinc/apple-fm-runtime` portable wire contract (AFS-02).
 *
 * These Effect schemas mirror the EXACT JSON the Swift `foundation-bridge`
 * helper accepts and emits on the loopback HTTP contract. They are portable:
 * no Node, no provider SDK, no Pylon or Desktop concern. Both the Swift bridge
 * and every TypeScript consumer are proven against this single source, so a
 * wire-shape drift is caught by the conformance tests instead of silently
 * breaking a caller.
 *
 * This module was extracted from the nested Pylon runtime
 * (`apps/pylon/packages/runtime/src/backends/apple-fm/contract.ts` and
 * `.../wire.ts`), reduced to the portable subset the shared turn system needs.
 * The Pylon Blueprint tools, receipts, fleet, and CLI concerns stay in Pylon.
 */

export const APPLE_FM_WIRE_SCHEMA_ID = "openagents.apple_fm.wire.v1" as const;

export const AppleFmUsageTruth = S.Literals(["exact", "estimated", "unknown"]);
export type AppleFmUsageTruth = typeof AppleFmUsageTruth.Type;

export const AppleFmUsageMeasurement = S.Struct({
  truth: AppleFmUsageTruth,
  promptTokens: S.optional(S.Number),
  completionTokens: S.optional(S.Number),
  totalTokens: S.optional(S.Number),
});
export type AppleFmUsageMeasurement = typeof AppleFmUsageMeasurement.Type;

/**
 * The frozen Apple FM unavailable-reason vocabulary. It matches the current
 * consumer contract so a decoder on any surface reaches the same fact.
 */
export const AppleFmUnavailableReason = S.Literals([
  "bridge_unreachable",
  "apple_intelligence_disabled",
  "unsupported_hardware",
  "model_unavailable",
  "permission_denied",
  "malformed_response",
  "not_ready",
  "unknown",
]);
export type AppleFmUnavailableReason = typeof AppleFmUnavailableReason.Type;

export const AppleFmHealthResponse = S.Struct({
  ready: S.Boolean,
  model: S.optional(S.String),
  modelId: S.optional(S.String),
  unavailableReason: S.optional(AppleFmUnavailableReason),
  message: S.optional(S.String),
  platform: S.optional(S.String),
  version: S.optional(S.String),
});
export type AppleFmHealthResponse = typeof AppleFmHealthResponse.Type;

export const AppleFmChatRole = S.Literals(["system", "user", "assistant", "tool"]);
export type AppleFmChatRole = typeof AppleFmChatRole.Type;

export const AppleFmChatMessage = S.Struct({
  role: AppleFmChatRole,
  content: S.String,
  name: S.optional(S.String),
  toolCallId: S.optional(S.String),
});
export type AppleFmChatMessage = typeof AppleFmChatMessage.Type;

/**
 * Guided-route request: the owner-bound connected candidate vocabulary the model
 * must choose exactly one of via constrained sampling. When present and
 * non-empty, the bridge returns a well-formed route-recommendation instead of
 * free text (owner directive 2026-07-20: on-device router).
 */
export const AppleFmRouteRequest = S.Struct({
  candidates: S.Array(S.String),
});
export type AppleFmRouteRequest = typeof AppleFmRouteRequest.Type;

export const AppleFmChatCompletionRequest = S.Struct({
  model: S.optional(S.String),
  messages: S.Array(AppleFmChatMessage),
  temperature: S.optional(S.Number),
  maxTokens: S.optional(S.Number),
  route: S.optional(AppleFmRouteRequest),
});
export type AppleFmChatCompletionRequest = typeof AppleFmChatCompletionRequest.Type;

export const AppleFmFinishReason = S.Literals([
  "stop",
  "length",
  "tool_calls",
  "content_filter",
  "error",
  "unknown",
]);
export type AppleFmFinishReason = typeof AppleFmFinishReason.Type;

export const AppleFmChatCompletionChoice = S.Struct({
  index: S.optional(S.Number),
  message: AppleFmChatMessage,
  finishReason: S.optional(AppleFmFinishReason),
});
export type AppleFmChatCompletionChoice = typeof AppleFmChatCompletionChoice.Type;

export const AppleFmChatCompletionResponse = S.Struct({
  id: S.optional(S.String),
  model: S.optional(S.String),
  choices: S.Array(AppleFmChatCompletionChoice),
  usage: S.optional(AppleFmUsageMeasurement),
});
export type AppleFmChatCompletionResponse = typeof AppleFmChatCompletionResponse.Type;

// --- Endpoint manifest ------------------------------------------------------

/**
 * Machine-readable index of the wire endpoints the neutral client speaks. The
 * conformance tests cover each; a new endpoint must be added here so the sweep
 * covers it. Session/tool-callback endpoints stay a Pylon concern and are not
 * part of the neutral turn-system client.
 */
export const APPLE_FM_WIRE_ENDPOINTS = [
  { method: "GET", path: "/health", response: "AppleFmHealthResponse" },
  {
    method: "POST",
    path: "/v1/chat/completions",
    request: "AppleFmChatCompletionRequest",
    response: "AppleFmChatCompletionResponse",
  },
  { method: "*", path: "(error envelope)", response: "AppleFmWireErrorResponse" },
] as const;

export const AppleFmWireErrorResponse = S.Struct({
  error: S.String,
  message: S.String,
  unavailableReason: S.optional(AppleFmUnavailableReason),
});
export type AppleFmWireErrorResponse = typeof AppleFmWireErrorResponse.Type;
