import { Schema as S } from "effect";
import { APPLE_FM_BACKEND_KIND, APPLE_FM_DEFAULT_MODEL_ID, type AppleFmUsageMeasurement } from "./contract";
import { type AppleFmHealthStatus } from "./client";
import { type AppleFmToolCallbackStatus, type AppleFmToolName } from "./tools";

export const AppleFmAcceptanceCaseName = S.Literals([
  "read_file_answer",
  "list_then_read",
  "search_then_read",
  "shell_then_summarize",
  "patch_then_verify",
  "approval_pause_or_refusal",
]);
export type AppleFmAcceptanceCaseName = typeof AppleFmAcceptanceCaseName.Type;

export const AppleFmAcceptanceStatus = S.Literals(["passed", "failed", "unsupported", "unavailable"]);
export type AppleFmAcceptanceStatus = typeof AppleFmAcceptanceStatus.Type;

export const AppleFmAcceptanceToolFact = S.Struct({
  toolName: S.String,
  status: S.String,
  message: S.optional(S.String),
});
export type AppleFmAcceptanceToolFact = {
  readonly toolName: AppleFmToolName | string;
  readonly status: AppleFmToolCallbackStatus | string;
  readonly message?: string;
};

export const AppleFmAcceptanceReceipt = S.Struct({
  kind: S.Literal("probe_apple_fm_acceptance_case"),
  caseName: AppleFmAcceptanceCaseName,
  backendKind: S.Literal(APPLE_FM_BACKEND_KIND),
  model: S.String,
  status: AppleFmAcceptanceStatus,
  availability: S.Struct({
    ready: S.Boolean,
    status: S.Literals(["ready", "unavailable", "unsupported", "malformed", "unreachable"]),
    unavailableReason: S.optional(S.String),
  }),
  usage: S.Struct({
    truth: S.Literals(["exact", "estimated", "unknown"]),
    promptTokens: S.optional(S.Number),
    completionTokens: S.optional(S.Number),
    totalTokens: S.optional(S.Number),
  }),
  toolFacts: S.Array(S.Struct({
    toolName: S.String,
    status: S.String,
    message: S.optional(S.String),
  })),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type AppleFmAcceptanceReceipt = typeof AppleFmAcceptanceReceipt.Type;

export const RETAINED_APPLE_FM_ACCEPTANCE_CASES: ReadonlyArray<AppleFmAcceptanceCaseName> = [
  "read_file_answer",
  "list_then_read",
  "search_then_read",
  "shell_then_summarize",
  "patch_then_verify",
  "approval_pause_or_refusal",
];

export function makeAppleFmAcceptanceReceipt(input: {
  readonly caseName: AppleFmAcceptanceCaseName;
  readonly status: AppleFmAcceptanceStatus;
  readonly availability: {
    readonly ready: boolean;
    readonly status: AppleFmHealthStatus;
    readonly unavailableReason?: string;
  };
  readonly usage?: AppleFmUsageMeasurement;
  readonly toolFacts?: ReadonlyArray<AppleFmAcceptanceToolFact>;
  readonly model?: string;
  readonly observedAt?: string;
}): AppleFmAcceptanceReceipt {
  return {
    kind: "probe_apple_fm_acceptance_case",
    caseName: input.caseName,
    backendKind: APPLE_FM_BACKEND_KIND,
    model: input.model ?? APPLE_FM_DEFAULT_MODEL_ID,
    status: input.status,
    availability: {
      ready: input.availability.ready,
      status: input.availability.status,
      unavailableReason: input.availability.unavailableReason,
    },
    usage: input.usage ?? { truth: "unknown" },
    toolFacts: (input.toolFacts ?? []).map((fact) => ({
      toolName: fact.toolName,
      status: fact.status,
      message: fact.message,
    })),
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
  };
}

export function classifyAppleFmAcceptanceStatus(input: {
  readonly ready: boolean;
  readonly status: AppleFmHealthStatus;
  readonly passed?: boolean;
}): AppleFmAcceptanceStatus {
  if (!input.ready) {
    return input.status === "unsupported" ? "unsupported" : "unavailable";
  }

  return input.passed === false ? "failed" : "passed";
}
