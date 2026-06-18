import { Schema as S } from "effect";
import { ProbeLlmUsage } from "../../llm/usage";
import { redactReceiptUrl } from "../../receipt-redaction";
import { GEMINI_BACKEND_KIND } from "./contract";

export const GeminiBackendAvailabilityReceipt = S.Struct({
  kind: S.Literal("probe_backend_availability"),
  backendKind: S.Literal(GEMINI_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  baseUrl: S.String,
  ready: S.Boolean,
  apiKeySource: S.optional(S.String),
  apiKeyRedacted: S.Literal(true),
  unavailableReason: S.optional(S.String),
  message: S.optional(S.String),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type GeminiBackendAvailabilityReceipt = typeof GeminiBackendAvailabilityReceipt.Type;

export const GeminiBackendFailureReceipt = S.Struct({
  kind: S.Literal("probe_backend_failure"),
  backendKind: S.Literal(GEMINI_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  baseUrl: S.String,
  failureClass: S.String,
  message: S.String,
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type GeminiBackendFailureReceipt = typeof GeminiBackendFailureReceipt.Type;

export const GeminiBackendTranscriptReceipt = S.Struct({
  kind: S.Literal("probe_backend_transcript"),
  backendKind: S.Literal(GEMINI_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  roundTrips: S.Number,
  usage: S.optional(ProbeLlmUsage),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type GeminiBackendTranscriptReceipt = typeof GeminiBackendTranscriptReceipt.Type;

export const GeminiBackendToolCallReceipt = S.Struct({
  kind: S.Literal("probe_backend_tool_call"),
  backendKind: S.Literal(GEMINI_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  toolCallId: S.String,
  toolName: S.String,
  status: S.Literals(["success", "error"]),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type GeminiBackendToolCallReceipt = typeof GeminiBackendToolCallReceipt.Type;

export function makeGeminiAvailabilityReceipt(input: {
  readonly profileId: string;
  readonly model: string;
  readonly baseUrl: string;
  readonly ready: boolean;
  readonly apiKeySource?: string;
  readonly unavailableReason?: string;
  readonly message?: string;
  readonly observedAt?: string;
}): GeminiBackendAvailabilityReceipt {
  return {
    kind: "probe_backend_availability",
    backendKind: GEMINI_BACKEND_KIND,
    profileId: input.profileId,
    model: input.model,
    baseUrl: redactReceiptUrl(input.baseUrl),
    ready: input.ready,
    apiKeySource: input.apiKeySource,
    apiKeyRedacted: true,
    unavailableReason: input.unavailableReason,
    message: input.message,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export function makeGeminiFailureReceipt(input: {
  readonly profileId: string;
  readonly model: string;
  readonly baseUrl: string;
  readonly failureClass: string;
  readonly message: string;
  readonly observedAt?: string;
}): GeminiBackendFailureReceipt {
  return {
    kind: "probe_backend_failure",
    backendKind: GEMINI_BACKEND_KIND,
    profileId: input.profileId,
    model: input.model,
    baseUrl: redactReceiptUrl(input.baseUrl),
    failureClass: input.failureClass,
    message: input.message,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export function makeGeminiTranscriptReceipt(input: {
  readonly profileId: string;
  readonly model: string;
  readonly roundTrips: number;
  readonly usage?: ProbeLlmUsage;
  readonly observedAt?: string;
}): GeminiBackendTranscriptReceipt {
  return {
    kind: "probe_backend_transcript",
    backendKind: GEMINI_BACKEND_KIND,
    profileId: input.profileId,
    model: input.model,
    roundTrips: input.roundTrips,
    usage: input.usage,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export function makeGeminiToolCallReceipt(input: {
  readonly profileId: string;
  readonly model: string;
  readonly toolCallId: string;
  readonly toolName: string;
  readonly status: "success" | "error";
  readonly observedAt?: string;
}): GeminiBackendToolCallReceipt {
  return {
    kind: "probe_backend_tool_call",
    backendKind: GEMINI_BACKEND_KIND,
    profileId: input.profileId,
    model: input.model,
    toolCallId: input.toolCallId,
    toolName: input.toolName,
    status: input.status,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export { redactReceiptUrl as redactUrl };
