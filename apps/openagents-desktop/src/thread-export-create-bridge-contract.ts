import {
  decodeThreadDisclosureIntent,
  decodeThreadDisclosureReceipt,
  type ThreadDisclosureIntent,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";

export const DesktopThreadExportCreateChannel = "openagents:thread-export:create" as const;

type ExportIntent = Extract<ThreadDisclosureIntent, { kind: "thread.export.create" }>;

export type DesktopThreadExportCreateRequest = Readonly<{ intent: ExportIntent }>;

export type DesktopThreadExportCreateResult =
  | Readonly<{ status: "stored" | "unchanged"; receipt: ThreadDisclosureReceipt }>
  | Readonly<{
      status: "rejected";
      reason:
        | "invalid_request"
        | "invalid_intent"
        | "unsupported_export"
        | "evidence_unavailable"
        | "evidence_thread_mismatch"
        | "invalid_evidence"
        | "host_metadata_invalid"
        | "persistence_refused"
        | "existing_artifact_conflict"
        | "persistence_failed"
        | "command_unavailable";
    }>;

type RejectionReason = Extract<DesktopThreadExportCreateResult, { status: "rejected" }>["reason"];

const rejectionReasons: ReadonlyArray<RejectionReason> = [
  "invalid_request",
  "invalid_intent",
  "unsupported_export",
  "evidence_unavailable",
  "evidence_thread_mismatch",
  "invalid_evidence",
  "host_metadata_invalid",
  "persistence_refused",
  "existing_artifact_conflict",
  "persistence_failed",
  "command_unavailable",
];

const isRejectionReason = (value: unknown): value is RejectionReason =>
  typeof value === "string" && rejectionReasons.some((reason) => reason === value);

const field = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;

const ownKeysAre = (value: unknown, allowed: ReadonlyArray<string>): boolean => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const actual = Object.keys(value).sort();
  const expected = [...allowed].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
};

const exactExportIntent = (value: unknown): boolean =>
  ownKeysAre(value, [
    "schema",
    "intentRef",
    "idempotencyKey",
    "threadRef",
    "actorRef",
    "expectedVisibilityVersion",
    "createdAt",
    "kind",
    "format",
    "artifactAudience",
  ]);

export const decodeDesktopThreadExportCreateRequest = (
  input: unknown,
): DesktopThreadExportCreateRequest | null => {
  if (!ownKeysAre(input, ["intent"])) return null;
  const rawIntent = field(input, "intent");
  if (!exactExportIntent(rawIntent)) return null;
  let intent: ThreadDisclosureIntent;
  try {
    intent = decodeThreadDisclosureIntent(rawIntent);
  } catch {
    return null;
  }
  if (
    intent.kind !== "thread.export.create" ||
    intent.format !== "canonical_event_bundle" ||
    intent.artifactAudience.kind !== "owner_only"
  ) {
    return null;
  }
  return { intent };
};

const exactExportReceipt = (value: unknown): boolean => {
  if (
    !ownKeysAre(value, [
      "schema",
      "receiptRef",
      "intentRef",
      "idempotencyKey",
      "threadRef",
      "observedAt",
      "kind",
      "result",
    ])
  ) {
    return false;
  }
  const result = field(value, "result");
  return ownKeysAre(result, [
    "status",
    "artifactRef",
    "artifactSha256",
    "format",
    "artifactAudience",
  ]);
};

export const decodeDesktopThreadExportCreateResult = (
  input: unknown,
  request: DesktopThreadExportCreateRequest,
): DesktopThreadExportCreateResult | null => {
  if (typeof input !== "object" || input === null || Array.isArray(input)) return null;
  const status = Reflect.get(input, "status");
  if (status === "rejected") {
    const reason = Reflect.get(input, "reason");
    return ownKeysAre(input, ["status", "reason"]) && isRejectionReason(reason)
      ? { status, reason }
      : null;
  }
  if (
    (status !== "stored" && status !== "unchanged") ||
    !ownKeysAre(input, ["status", "receipt"])
  ) {
    return null;
  }
  const rawReceipt = Reflect.get(input, "receipt");
  if (!exactExportReceipt(rawReceipt)) return null;
  let receipt: ThreadDisclosureReceipt;
  try {
    receipt = decodeThreadDisclosureReceipt(rawReceipt);
  } catch {
    return null;
  }
  const intent = request.intent;
  if (
    receipt.kind !== "thread.export.create" ||
    receipt.result.status !== "export_created" ||
    receipt.intentRef !== intent.intentRef ||
    receipt.idempotencyKey !== intent.idempotencyKey ||
    receipt.threadRef !== intent.threadRef ||
    receipt.result.format !== intent.format ||
    receipt.result.artifactAudience.kind !== "owner_only"
  ) {
    return null;
  }
  return { status, receipt };
};

export const unavailableDesktopThreadExportCreateResult = (): DesktopThreadExportCreateResult => ({
  status: "rejected",
  reason: "command_unavailable",
});

export type DesktopThreadExportCreateInvoker = (
  channel: typeof DesktopThreadExportCreateChannel,
  request: DesktopThreadExportCreateRequest,
) => Promise<unknown>;

/** Sandboxed renderer-to-main boundary for creating one owner-only canonical export. */
export const invokeDesktopThreadExportCreate = async (
  invoke: DesktopThreadExportCreateInvoker,
  input: unknown,
): Promise<DesktopThreadExportCreateResult> => {
  const request = decodeDesktopThreadExportCreateRequest(input);
  if (request === null) return { status: "rejected", reason: "invalid_request" };
  try {
    return (
      decodeDesktopThreadExportCreateResult(
        await invoke(DesktopThreadExportCreateChannel, request),
        request,
      ) ?? unavailableDesktopThreadExportCreateResult()
    );
  } catch {
    return unavailableDesktopThreadExportCreateResult();
  }
};
