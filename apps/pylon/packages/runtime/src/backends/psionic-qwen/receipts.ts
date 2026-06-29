import { Schema as S } from "effect";
import { ProbeLlmUsage } from "../../llm/usage.js";
import { redactReceiptUrl } from "../../receipt-redaction.js";
import { PSIONIC_QWEN_BACKEND_KIND } from "./contract.js";

export const PsionicQwenAvailabilityReceipt = S.Struct({
  kind: S.Literal("probe_backend_availability"),
  backendKind: S.Literal(PSIONIC_QWEN_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  baseUrl: S.String,
  ready: S.Boolean,
  status: S.String,
  modelRefs: S.Array(S.String),
  supportedEndpointRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  message: S.optional(S.String),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type PsionicQwenAvailabilityReceipt = typeof PsionicQwenAvailabilityReceipt.Type;

export const PsionicQwenFailureReceipt = S.Struct({
  kind: S.Literal("probe_backend_failure"),
  backendKind: S.Literal(PSIONIC_QWEN_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  baseUrl: S.String,
  failureClass: S.String,
  message: S.String,
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type PsionicQwenFailureReceipt = typeof PsionicQwenFailureReceipt.Type;

export const PsionicQwenTranscriptReceipt = S.Struct({
  kind: S.Literal("probe_backend_transcript"),
  backendKind: S.Literal(PSIONIC_QWEN_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  roundTrips: S.Number,
  usage: S.optional(ProbeLlmUsage),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type PsionicQwenTranscriptReceipt = typeof PsionicQwenTranscriptReceipt.Type;

export const PsionicQwenToolCallReceipt = S.Struct({
  kind: S.Literal("probe_backend_tool_call"),
  backendKind: S.Literal(PSIONIC_QWEN_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  toolCallId: S.String,
  toolName: S.String,
  status: S.Literals(["success", "error"]),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type PsionicQwenToolCallReceipt = typeof PsionicQwenToolCallReceipt.Type;

export function makePsionicQwenAvailabilityReceipt(input: {
  readonly profileId: string;
  readonly model: string;
  readonly baseUrl: string;
  readonly ready: boolean;
  readonly status: string;
  readonly modelRefs: ReadonlyArray<string>;
  readonly supportedEndpointRefs: ReadonlyArray<string>;
  readonly blockerRefs: ReadonlyArray<string>;
  readonly message?: string;
  readonly observedAt?: string;
}): PsionicQwenAvailabilityReceipt {
  return {
    kind: "probe_backend_availability",
    backendKind: PSIONIC_QWEN_BACKEND_KIND,
    profileId: input.profileId,
    model: input.model,
    baseUrl: redactReceiptUrl(input.baseUrl),
    ready: input.ready,
    status: input.status,
    modelRefs: [...input.modelRefs],
    supportedEndpointRefs: [...input.supportedEndpointRefs],
    blockerRefs: [...input.blockerRefs],
    message: input.message,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export function makePsionicQwenFailureReceipt(input: {
  readonly profileId: string;
  readonly model: string;
  readonly baseUrl: string;
  readonly failureClass: string;
  readonly message: string;
  readonly observedAt?: string;
}): PsionicQwenFailureReceipt {
  return {
    kind: "probe_backend_failure",
    backendKind: PSIONIC_QWEN_BACKEND_KIND,
    profileId: input.profileId,
    model: input.model,
    baseUrl: redactReceiptUrl(input.baseUrl),
    failureClass: input.failureClass,
    message: input.message,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export function makePsionicQwenTranscriptReceipt(input: {
  readonly profileId: string;
  readonly model: string;
  readonly roundTrips: number;
  readonly usage?: typeof ProbeLlmUsage.Type;
  readonly observedAt?: string;
}): PsionicQwenTranscriptReceipt {
  return {
    kind: "probe_backend_transcript",
    backendKind: PSIONIC_QWEN_BACKEND_KIND,
    profileId: input.profileId,
    model: input.model,
    roundTrips: input.roundTrips,
    usage: input.usage,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export function makePsionicQwenToolCallReceipt(input: {
  readonly profileId: string;
  readonly model: string;
  readonly toolCallId: string;
  readonly toolName: string;
  readonly status: "success" | "error";
  readonly observedAt?: string;
}): PsionicQwenToolCallReceipt {
  return {
    kind: "probe_backend_tool_call",
    backendKind: PSIONIC_QWEN_BACKEND_KIND,
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
