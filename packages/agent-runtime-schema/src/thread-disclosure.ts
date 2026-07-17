import { Schema as S } from "effect";

export const ThreadDisclosureIntentSchemaLiteral =
  "openagents.thread_disclosure_intent.v1" as const;
export const ThreadDisclosureReceiptSchemaLiteral =
  "openagents.thread_disclosure_receipt.v1" as const;

const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const Timestamp = S.String.check(S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/));
const Version = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));

export const ThreadDisclosureAudience = S.Union([
  S.Struct({ kind: S.Literal("owner_only") }),
  S.Struct({ kind: S.Literal("workspace_members"), workspaceRef: Ref }),
  S.Struct({ kind: S.Literal("named_group"), workspaceRef: Ref, groupRef: Ref }),
  S.Struct({ kind: S.Literal("internet_readable") }),
]);
export type ThreadDisclosureAudience = typeof ThreadDisclosureAudience.Type;

export const ThreadAdministratorAccess = S.Union([
  S.Struct({ kind: S.Literal("none") }),
  S.Struct({ kind: S.Literal("workspace_admins"), workspaceRef: Ref }),
]);
export type ThreadAdministratorAccess = typeof ThreadAdministratorAccess.Type;

export const ThreadDisclosureTarget = S.Struct({
  audience: ThreadDisclosureAudience,
  administratorAccess: ThreadAdministratorAccess,
});
export type ThreadDisclosureTarget = typeof ThreadDisclosureTarget.Type;

const ExpectedVisibilityVersion = S.Union([
  S.Struct({ state: S.Literal("known"), value: Version }),
  S.Struct({ state: S.Literal("unknown"), reason: S.Literal("not_observed") }),
]);

export const ThreadExportFormat = S.Literals(["markdown", "json", "canonical_event_bundle"]);
export type ThreadExportFormat = typeof ThreadExportFormat.Type;

const intentBase = {
  schema: S.Literal(ThreadDisclosureIntentSchemaLiteral),
  intentRef: Ref,
  idempotencyKey: Ref,
  threadRef: Ref,
  actorRef: Ref,
  expectedVisibilityVersion: ExpectedVisibilityVersion,
  createdAt: Timestamp,
};

/**
 * Provider-neutral intent only. Decoding this envelope grants no disclosure,
 * export, persistence, transport, or acceptance authority.
 */
export const ThreadDisclosureIntent = S.Union([
  S.Struct({
    ...intentBase,
    kind: S.Literal("thread.visibility.set"),
    target: ThreadDisclosureTarget,
  }),
  S.Struct({
    ...intentBase,
    kind: S.Literal("thread.export.create"),
    format: ThreadExportFormat,
    artifactAudience: ThreadDisclosureAudience,
  }),
]);
export type ThreadDisclosureIntent = typeof ThreadDisclosureIntent.Type;

const receiptBase = {
  schema: S.Literal(ThreadDisclosureReceiptSchemaLiteral),
  receiptRef: Ref,
  intentRef: Ref,
  idempotencyKey: Ref,
  threadRef: Ref,
  observedAt: Timestamp,
  kind: S.Literals(["thread.visibility.set", "thread.export.create"]),
};

const ThreadDisclosureResult = S.Union([
  S.Struct({ status: S.Literal("accepted_pending") }),
  S.Struct({ status: S.Literal("rejected"), reasonRef: Ref }),
  S.Struct({ status: S.Literal("failed"), reasonRef: Ref }),
  S.Struct({
    status: S.Literal("visibility_applied"),
    visibilityVersion: Version,
    target: ThreadDisclosureTarget,
  }),
  S.Struct({
    status: S.Literal("export_created"),
    artifactRef: Ref,
    artifactSha256: S.String.check(S.isPattern(/^[a-f0-9]{64}$/)),
    format: ThreadExportFormat,
    artifactAudience: ThreadDisclosureAudience,
  }),
]);

/** Receipt evidence remains ref-only; the exported thread bytes live elsewhere. */
export const ThreadDisclosureReceipt = S.Struct({
  ...receiptBase,
  result: ThreadDisclosureResult,
});
export type ThreadDisclosureReceipt = typeof ThreadDisclosureReceipt.Type;

const decodeIntent = S.decodeUnknownSync(ThreadDisclosureIntent);
const decodeReceipt = S.decodeUnknownSync(ThreadDisclosureReceipt);

const forbiddenRawFields = new Set([
  "body",
  "bytes",
  "content",
  "message",
  "prompt",
  "summary",
  "transcript",
]);

const assertRefOnlyInput = (input: unknown): void => {
  const seen = new WeakSet<object>();
  const visit = (value: unknown): void => {
    if (typeof value !== "object" || value === null || seen.has(value)) return;
    seen.add(value);
    if (Array.isArray(value)) {
      for (const item of value) visit(item);
      return;
    }
    for (const [key, child] of Object.entries(value)) {
      if (forbiddenRawFields.has(key)) {
        throw new Error(`Thread disclosure evidence contains forbidden raw field: ${key}`);
      }
      visit(child);
    }
  };
  visit(input);
};

const assertTimestamp = (value: string): void => {
  if (!Number.isFinite(Date.parse(value))) {
    throw new Error("Thread disclosure timestamp is invalid");
  }
};

const assertWorkspaceConsistency = (target: ThreadDisclosureTarget): void => {
  if (
    target.administratorAccess.kind === "workspace_admins" &&
    (target.audience.kind === "workspace_members" || target.audience.kind === "named_group") &&
    target.administratorAccess.workspaceRef !== target.audience.workspaceRef
  ) {
    throw new Error("Thread audience and administrator access must name the same workspace");
  }
};

export const decodeThreadDisclosureIntent = (input: unknown): ThreadDisclosureIntent => {
  assertRefOnlyInput(input);
  const decoded = decodeIntent(input);
  assertTimestamp(decoded.createdAt);
  if (decoded.kind === "thread.visibility.set") assertWorkspaceConsistency(decoded.target);
  return decoded;
};

export const decodeThreadDisclosureReceipt = (input: unknown): ThreadDisclosureReceipt => {
  assertRefOnlyInput(input);
  const decoded = decodeReceipt(input);
  assertTimestamp(decoded.observedAt);
  if (decoded.result.status === "visibility_applied") {
    if (decoded.kind !== "thread.visibility.set") {
      throw new Error("An export intent cannot produce a visibility-applied receipt");
    }
    assertWorkspaceConsistency(decoded.result.target);
  }
  if (decoded.result.status === "export_created" && decoded.kind !== "thread.export.create") {
    throw new Error("A visibility intent cannot produce an export-created receipt");
  }
  return decoded;
};

export type ThreadDisclosureReplayDisposition = "new" | "exact_retry" | "conflicting_reuse";

/** Classify retries without granting execution or treating reuse as new work. */
export const classifyThreadDisclosureReplay = (
  existing: ThreadDisclosureIntent,
  incoming: ThreadDisclosureIntent,
): ThreadDisclosureReplayDisposition => {
  const sharesIdentity =
    existing.intentRef === incoming.intentRef ||
    existing.idempotencyKey === incoming.idempotencyKey;
  if (!sharesIdentity) return "new";
  return JSON.stringify(existing) === JSON.stringify(incoming)
    ? "exact_retry"
    : "conflicting_reuse";
};
