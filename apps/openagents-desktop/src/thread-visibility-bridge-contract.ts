import {
  decodeThreadDisclosureIntent,
  decodeThreadDisclosureReceipt,
  type ThreadDisclosureIntent,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";
import { Schema as S } from "effect";

export const DesktopThreadVisibilityApplyChannel = "openagents:thread-visibility:apply" as const;

type VisibilityIntent = Extract<ThreadDisclosureIntent, { kind: "thread.visibility.set" }>;

export type DesktopThreadVisibilityApplyRequest = Readonly<{ intent: VisibilityIntent }>;

export const DesktopThreadVisibilityRejectionReason = S.Literals([
  "invalid_request",
  "corrupt_store",
  "identity_conflict",
  "stale_version",
  "capacity_exceeded",
  "persistence_failed",
  "command_unavailable",
]);
type DesktopThreadVisibilityRejectionReason = typeof DesktopThreadVisibilityRejectionReason.Type;
export type DesktopThreadVisibilityApplyResult =
  | Readonly<{ status: "stored" | "unchanged"; receipt: ThreadDisclosureReceipt }>
  | Readonly<{
      status: "rejected";
      reason: DesktopThreadVisibilityRejectionReason;
    }>;

const ownKeysAre = (value: unknown, allowed: ReadonlyArray<string>): boolean => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const actual = Object.keys(value).sort();
  const expected = [...allowed].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
};

const field = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;

const exactExpectedVersion = (value: unknown): boolean => {
  const state = field(value, "state");
  return state === "known"
    ? ownKeysAre(value, ["state", "value"])
    : state === "unknown"
      ? ownKeysAre(value, ["state", "reason"])
      : false;
};

const exactAudience = (value: unknown): boolean => {
  const kind = field(value, "kind");
  if (kind === "owner_only" || kind === "internet_readable") {
    return ownKeysAre(value, ["kind"]);
  }
  if (kind === "workspace_members") return ownKeysAre(value, ["kind", "workspaceRef"]);
  if (kind === "named_group") {
    return ownKeysAre(value, ["kind", "workspaceRef", "groupRef"]);
  }
  return false;
};

const exactAdministratorAccess = (value: unknown): boolean => {
  const kind = field(value, "kind");
  return kind === "none"
    ? ownKeysAre(value, ["kind"])
    : kind === "workspace_admins"
      ? ownKeysAre(value, ["kind", "workspaceRef"])
      : false;
};

const exactTarget = (value: unknown): boolean =>
  ownKeysAre(value, ["audience", "administratorAccess"]) &&
  exactAudience(field(value, "audience")) &&
  exactAdministratorAccess(field(value, "administratorAccess"));

const exactVisibilityIntent = (value: unknown): boolean =>
  ownKeysAre(value, [
    "schema",
    "intentRef",
    "idempotencyKey",
    "threadRef",
    "actorRef",
    "expectedVisibilityVersion",
    "createdAt",
    "kind",
    "target",
  ]) &&
  exactExpectedVersion(field(value, "expectedVisibilityVersion")) &&
  exactTarget(field(value, "target"));

export const decodeDesktopThreadVisibilityApplyRequest = (
  input: unknown,
): DesktopThreadVisibilityApplyRequest | null => {
  if (!ownKeysAre(input, ["intent"])) return null;
  const rawIntent = field(input, "intent");
  if (!exactVisibilityIntent(rawIntent)) return null;
  let intent: ThreadDisclosureIntent;
  try {
    intent = decodeThreadDisclosureIntent(rawIntent);
  } catch {
    return null;
  }
  return intent.kind === "thread.visibility.set" ? { intent } : null;
};

const decodeRejectionReason = S.decodeUnknownSync(DesktopThreadVisibilityRejectionReason);

export const decodeDesktopThreadVisibilityApplyResult = (
  input: unknown,
  request: DesktopThreadVisibilityApplyRequest,
): DesktopThreadVisibilityApplyResult | null => {
  const status = field(input, "status");
  if (status === "rejected") {
    const reason = field(input, "reason");
    if (!ownKeysAre(input, ["status", "reason"])) return null;
    try {
      return { status, reason: decodeRejectionReason(reason) };
    } catch {
      return null;
    }
  }
  if (
    (status !== "stored" && status !== "unchanged") ||
    !ownKeysAre(input, ["status", "receipt"])
  ) {
    return null;
  }
  const rawReceipt = field(input, "receipt");
  if (
    !ownKeysAre(rawReceipt, [
      "schema",
      "receiptRef",
      "intentRef",
      "idempotencyKey",
      "threadRef",
      "observedAt",
      "kind",
      "result",
    ])
  )
    return null;
  const rawResult = field(rawReceipt, "result");
  if (
    !ownKeysAre(rawResult, ["status", "visibilityVersion", "target"]) ||
    !exactTarget(field(rawResult, "target"))
  ) {
    return null;
  }
  let receipt: ThreadDisclosureReceipt;
  try {
    receipt = decodeThreadDisclosureReceipt(rawReceipt);
  } catch {
    return null;
  }
  const intent = request.intent;
  if (
    receipt.kind !== "thread.visibility.set" ||
    receipt.result.status !== "visibility_applied" ||
    receipt.intentRef !== intent.intentRef ||
    receipt.idempotencyKey !== intent.idempotencyKey ||
    receipt.threadRef !== intent.threadRef ||
    JSON.stringify(receipt.result.target) !== JSON.stringify(intent.target)
  ) {
    return null;
  }
  return { status, receipt };
};

export const unavailableDesktopThreadVisibilityApplyResult =
  (): DesktopThreadVisibilityApplyResult => ({
    status: "rejected",
    reason: "command_unavailable",
  });

export type DesktopThreadVisibilityApplyInvoker = (
  channel: typeof DesktopThreadVisibilityApplyChannel,
  request: DesktopThreadVisibilityApplyRequest,
) => Promise<unknown>;

/** Sandboxed fixed-channel boundary for explicit visibility policy evidence. */
export const invokeDesktopThreadVisibilityApply = async (
  invoke: DesktopThreadVisibilityApplyInvoker,
  input: unknown,
): Promise<DesktopThreadVisibilityApplyResult> => {
  const request = decodeDesktopThreadVisibilityApplyRequest(input);
  if (request === null) return { status: "rejected", reason: "invalid_request" };
  try {
    return (
      decodeDesktopThreadVisibilityApplyResult(
        await invoke(DesktopThreadVisibilityApplyChannel, request),
        request,
      ) ?? unavailableDesktopThreadVisibilityApplyResult()
    );
  } catch {
    return unavailableDesktopThreadVisibilityApplyResult();
  }
};
