import { Schema as S } from "effect";
import { redactReceiptUrl } from "../../receipt-redaction.js";
import {
  APPLE_FM_BACKEND_KIND,
  type AppleFmUnavailableReason,
  type AppleFmUsageMeasurement,
} from "./contract.js";

export const AppleFmBackendAvailabilityReceipt = S.Struct({
  kind: S.Literal("probe_backend_availability"),
  backendKind: S.Literal(APPLE_FM_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  baseUrl: S.String,
  ready: S.Boolean,
  unavailableReason: S.optional(S.String),
  message: S.optional(S.String),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type AppleFmBackendAvailabilityReceipt = typeof AppleFmBackendAvailabilityReceipt.Type;

export const AppleFmBackendFailureReceipt = S.Struct({
  kind: S.Literal("probe_backend_failure"),
  backendKind: S.Literal(APPLE_FM_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  baseUrl: S.String,
  failureClass: S.String,
  message: S.String,
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type AppleFmBackendFailureReceipt = typeof AppleFmBackendFailureReceipt.Type;

export const AppleFmBackendTranscriptReceipt = S.Struct({
  kind: S.Literal("probe_backend_transcript"),
  backendKind: S.Literal(APPLE_FM_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  usage: S.optional(S.Struct({
    truth: S.Literals(["exact", "estimated", "unknown"]),
    promptTokens: S.optional(S.Number),
    completionTokens: S.optional(S.Number),
    totalTokens: S.optional(S.Number),
  })),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type AppleFmBackendTranscriptReceipt = typeof AppleFmBackendTranscriptReceipt.Type;

export interface MakeAppleFmAvailabilityReceiptInput {
  readonly profileId: string;
  readonly model: string;
  readonly baseUrl: string;
  readonly ready: boolean;
  readonly unavailableReason?: AppleFmUnavailableReason | string;
  readonly message?: string;
  readonly observedAt?: string;
}

export function makeAppleFmAvailabilityReceipt(
  input: MakeAppleFmAvailabilityReceiptInput,
): AppleFmBackendAvailabilityReceipt {
  return {
    kind: "probe_backend_availability",
    backendKind: APPLE_FM_BACKEND_KIND,
    profileId: input.profileId,
    model: input.model,
    baseUrl: redactReceiptUrl(input.baseUrl),
    ready: input.ready,
    unavailableReason: input.unavailableReason,
    message: input.message,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export function makeAppleFmFailureReceipt(input: {
  readonly profileId: string;
  readonly model: string;
  readonly baseUrl: string;
  readonly failureClass: string;
  readonly message: string;
  readonly observedAt?: string;
}): AppleFmBackendFailureReceipt {
  return {
    kind: "probe_backend_failure",
    backendKind: APPLE_FM_BACKEND_KIND,
    profileId: input.profileId,
    model: input.model,
    baseUrl: redactReceiptUrl(input.baseUrl),
    failureClass: input.failureClass,
    message: input.message,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export function makeAppleFmTranscriptReceipt(input: {
  readonly profileId: string;
  readonly model: string;
  readonly usage?: AppleFmUsageMeasurement;
  readonly observedAt?: string;
}): AppleFmBackendTranscriptReceipt {
  return {
    kind: "probe_backend_transcript",
    backendKind: APPLE_FM_BACKEND_KIND,
    profileId: input.profileId,
    model: input.model,
    usage: input.usage,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export { redactReceiptUrl as redactUrl };
