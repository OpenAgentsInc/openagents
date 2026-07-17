import {
  decodeRuntimeControlOutcome,
  type RuntimeControlOutcome,
} from "@openagentsinc/agent-runtime-schema";

export const DesktopRuntimeControlOutcomeRecordChannel = "desktop:runtime-control-outcome:record";
export const DesktopRuntimeControlOutcomeLookupChannel = "desktop:runtime-control-outcome:lookup";
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

export type DesktopRuntimeControlOutcomeLookup = Readonly<{
  threadRef: string;
  intentRef: string;
  idempotencyKey: string;
}>;

export type DesktopRuntimeControlOutcomeLookupResult =
  | Readonly<{ status: "found"; record: DesktopRuntimeControlOutcomeRecord }>
  | Readonly<{ status: "missing" }>
  | Readonly<{
      status: "rejected";
      reason: "invalid_request" | "corrupt_ledger" | "identity_conflict";
    }>;

export const decodeDesktopRuntimeControlOutcomeLookup = (
  input: unknown,
): DesktopRuntimeControlOutcomeLookup | null => {
  if (typeof input !== "object" || input === null) return null;
  const value = input as Readonly<Record<string, unknown>>;
  if (
    Object.keys(value).some(
      (key) => key !== "threadRef" && key !== "intentRef" && key !== "idempotencyKey",
    )
  ) return null;
  if (
    typeof value.threadRef !== "string" || !safeRef.test(value.threadRef) ||
    typeof value.intentRef !== "string" || !safeRef.test(value.intentRef) ||
    typeof value.idempotencyKey !== "string" || !safeRef.test(value.idempotencyKey)
  ) return null;
  return {
    threadRef: value.threadRef,
    intentRef: value.intentRef,
    idempotencyKey: value.idempotencyKey,
  };
};

export const decodeDesktopRuntimeControlOutcomeLookupResult = (
  input: unknown,
): DesktopRuntimeControlOutcomeLookupResult | null => {
  if (typeof input !== "object" || input === null) return null;
  const value = input as Readonly<{ status?: unknown; reason?: unknown; record?: unknown }>;
  if (Object.keys(value).some((key) => key !== "status" && key !== "reason" && key !== "record")) return null;
  if (value.status === "missing") return { status: "missing" };
  if (value.status === "found") {
    const record = decodeDesktopRuntimeControlOutcomeRecord(value.record);
    return record === null ? null : { status: "found", record };
  }
  if (
    value.status === "rejected" &&
    (value.reason === "invalid_request" || value.reason === "corrupt_ledger" || value.reason === "identity_conflict")
  ) return { status: "rejected", reason: value.reason };
  return null;
};

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
