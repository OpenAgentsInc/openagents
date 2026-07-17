import {
  decodeRuntimeControlOutcome,
  type RuntimeControlOutcome,
} from "@openagentsinc/agent-runtime-schema";

export const DesktopRuntimeControlOutcomeRecordChannel = "desktop:runtime-control-outcome:record";
export const DesktopRuntimeControlOutcomeLedgerSchema =
  "openagents.desktop_runtime_control_outcome_ledger.v1" as const;

const safeRef = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$/;

export type DesktopRuntimeControlOutcomeRecord = Readonly<{
  threadRef: string;
  outcome: RuntimeControlOutcome;
}>;

export type DesktopRuntimeControlOutcomeRecordResult = Readonly<{
  status: "stored" | "unchanged" | "advanced" | "rejected";
  reason?:
    | "invalid_request"
    | "corrupt_ledger"
    | "identity_conflict"
    | "evidence_conflict"
    | "persistence_failed";
  record?: DesktopRuntimeControlOutcomeRecord;
}>;

export const decodeDesktopRuntimeControlOutcomeRecord = (
  input: unknown,
): DesktopRuntimeControlOutcomeRecord | null => {
  if (typeof input !== "object" || input === null) return null;
  const value = input as Readonly<{ threadRef?: unknown; outcome?: unknown }>;
  if (Object.keys(value).some((key) => key !== "threadRef" && key !== "outcome")) return null;
  if (typeof value.threadRef !== "string" || !safeRef.test(value.threadRef)) return null;
  if (typeof value.outcome !== "object" || value.outcome === null) return null;
  if (
    Object.keys(value.outcome).some(
      (key) =>
        ![
          "schema",
          "outcomeRef",
          "intentRef",
          "idempotencyKey",
          "observedAt",
          "admission",
          "delivery",
          "terminal",
        ].includes(key),
    )
  )
    return null;
  try {
    return { threadRef: value.threadRef, outcome: decodeRuntimeControlOutcome(value.outcome) };
  } catch {
    return null;
  }
};

export const decodeDesktopRuntimeControlOutcomeRecordResult = (
  input: unknown,
): DesktopRuntimeControlOutcomeRecordResult | null => {
  if (typeof input !== "object" || input === null) return null;
  const value = input as Readonly<{ status?: unknown; reason?: unknown; record?: unknown }>;
  if (!["stored", "unchanged", "advanced", "rejected"].includes(String(value.status))) return null;
  const decodedRecord =
    value.record === undefined ? undefined : decodeDesktopRuntimeControlOutcomeRecord(value.record);
  if (decodedRecord === null) return null;
  const reason = value.reason === undefined ? undefined : String(value.reason);
  if (
    reason !== undefined &&
    ![
      "invalid_request",
      "corrupt_ledger",
      "identity_conflict",
      "evidence_conflict",
      "persistence_failed",
    ].includes(reason)
  )
    return null;
  return {
    status: value.status as DesktopRuntimeControlOutcomeRecordResult["status"],
    ...(reason === undefined
      ? {}
      : { reason: reason as NonNullable<DesktopRuntimeControlOutcomeRecordResult["reason"]> }),
    ...(decodedRecord === undefined ? {} : { record: decodedRecord }),
  };
};
