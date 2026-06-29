import { Schema as S } from "effect";

export const APPLE_FM_BACKEND_KIND = "apple_fm_bridge" as const;
export const APPLE_FM_LOCAL_PROFILE_ID = "apple-fm-local" as const;
export const APPLE_FM_DEFAULT_MODEL_ID = "apple-foundation-model" as const;
export const APPLE_FM_DEFAULT_BASE_URL = "http://127.0.0.1:11435" as const;
export const PROBE_APPLE_FM_BACKEND_CAPABILITY = "probe.backend.apple_fm_bridge" as const;

export const AppleFmBackendKind = S.Literal(APPLE_FM_BACKEND_KIND);
export type AppleFmBackendKind = typeof AppleFmBackendKind.Type;

export const AppleFmProfileId = S.Literal(APPLE_FM_LOCAL_PROFILE_ID);
export type AppleFmProfileId = typeof AppleFmProfileId.Type;

export const AppleFmUsageTruth = S.Literals(["exact", "estimated", "unknown"]);
export type AppleFmUsageTruth = typeof AppleFmUsageTruth.Type;

export const AppleFmUsageMeasurement = S.Struct({
  truth: AppleFmUsageTruth,
  promptTokens: S.optional(S.Number),
  completionTokens: S.optional(S.Number),
  totalTokens: S.optional(S.Number),
});
export type AppleFmUsageMeasurement = typeof AppleFmUsageMeasurement.Type;

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

export const AppleFmChatCompletionRequest = S.Struct({
  model: S.optional(S.String),
  messages: S.Array(AppleFmChatMessage),
  temperature: S.optional(S.Number),
  maxTokens: S.optional(S.Number),
});
export type AppleFmChatCompletionRequest = typeof AppleFmChatCompletionRequest.Type;

export const AppleFmFinishReason = S.Literals(["stop", "length", "tool_calls", "content_filter", "error", "unknown"]);
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

export const AppleFmStreamSnapshotEvent = S.Struct({
  kind: S.Literal("apple_fm_assistant_snapshot"),
  sequence: S.Number,
  content: S.String,
  observedAt: S.String,
  finishReason: S.optional(AppleFmFinishReason),
});
export type AppleFmStreamSnapshotEvent = typeof AppleFmStreamSnapshotEvent.Type;
