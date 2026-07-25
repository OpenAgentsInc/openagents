import { Schema as S } from "effect";

import { parseEngramBody } from "../nostr-memory/engram.ts";
import { validateReadStateBlob } from "../nostr-memory/read-state.ts";
import { parseReminderContent } from "../nostr-memory/reminder.ts";
import {
  SARAH_ENGRAM_KIND,
  SARAH_READ_STATE_KIND,
  SARAH_REMINDER_KIND,
} from "../nostr-memory/types.ts";
import {
  SARAH_AUTHORITY_RECEIPT_KIND,
  SARAH_TURN_RECORD_KIND,
  SarahTurnRecordPayload,
} from "../nostr-turn/types.ts";

export const ISSUE31_OWNER_PROJECTION_SCHEMA =
  "openagents.omega.issue31.owner_projection.v1" as const;

const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));
const PublicRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(256),
  S.isPattern(/^[a-z][a-z0-9_-]*(?:\.[A-Za-z0-9][A-Za-z0-9_-]*){1,}(?::[A-Za-z0-9._-]+)?$/),
);
const UnixSeconds = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);
const Generation = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);
const BoundedEngramPlaintext = S.String.check(S.isMinLength(1), S.isMaxLength(65_535));
const BoundedStatePlaintext = S.String.check(S.isMinLength(1), S.isMaxLength(524_288));
const ReminderId = S.String.check(S.isPattern(/^[0-9a-f]{32}$/));
const ConversationTag = S.String.check(S.isPattern(/^sarah\.[0-9a-f]{24}$/));
const BoundedMessage = S.String.check(S.isMinLength(1), S.isMaxLength(12_000));

export const Issue31OwnerAuthorityReceiptProjectionSchema = S.Struct({
  kind: S.Literal("authority_receipt"),
  receiptRef: PublicRef,
  turnRef: PublicRef,
  authorityDecision: S.Struct({
    state: S.Literals(["allowed", "refused"]),
    decisionRef: PublicRef,
    reasonRef: S.optionalKey(PublicRef),
  }),
  targetOutcome: S.Struct({
    state: S.Literals(["pending", "succeeded", "failed", "stopped", "unavailable"]),
    outcomeRef: S.optionalKey(PublicRef),
    reasonRef: S.optionalKey(PublicRef),
  }),
});

export const Issue31OwnerProjectionBodySchema = S.Union([
  S.Struct({
    kind: S.Literal("message"),
    role: S.Literals(["owner", "sarah"]),
    conversation: ConversationTag,
    text: BoundedMessage,
    replyToEventId: S.optionalKey(Hex64),
  }),
  S.Struct({
    kind: S.Literal("turn"),
    payload: SarahTurnRecordPayload,
  }),
  Issue31OwnerAuthorityReceiptProjectionSchema,
  S.Struct({
    kind: S.Literal("engram"),
    dTag: Hex64,
    plaintext: BoundedEngramPlaintext,
  }),
  S.Struct({
    kind: S.Literal("read_state"),
    dTag: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
    plaintext: BoundedStatePlaintext,
  }),
  S.Struct({
    kind: S.Literal("reminder"),
    reminderId: ReminderId,
    plaintext: BoundedStatePlaintext,
    notBefore: S.optionalKey(UnixSeconds),
    expiration: S.optionalKey(UnixSeconds),
  }),
]);
export type Issue31OwnerProjectionBody = S.Schema.Type<typeof Issue31OwnerProjectionBodySchema>;

export const Issue31OwnerProjectionRecordSchema = S.Struct({
  schema: S.Literal(ISSUE31_OWNER_PROJECTION_SCHEMA),
  recordType: S.Literal("owner_projection"),
  hostRef: PublicRef,
  hostPublicKeyHex: Hex64,
  devicePublicKeyHex: Hex64,
  grantRef: PublicRef,
  expectedGeneration: Generation,
  sourceEventId: Hex64,
  sourceAuthorPublicKeyHex: Hex64,
  sourceRole: S.Literals(["owner", "sarah"]),
  sourceKind: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(65_535)),
  sourceCreatedAt: UnixSeconds,
  projectedAt: UnixSeconds,
  projection: Issue31OwnerProjectionBodySchema,
});
export interface Issue31OwnerProjectionRecord extends S.Schema.Type<
  typeof Issue31OwnerProjectionRecordSchema
> {}

const decodeRecord = S.decodeUnknownSync(Issue31OwnerProjectionRecordSchema);

const sourceKindForProjection = (projection: Issue31OwnerProjectionBody): number => {
  if (projection.kind === "message") return 14;
  if (projection.kind === "turn") return SARAH_TURN_RECORD_KIND;
  if (projection.kind === "authority_receipt") return SARAH_AUTHORITY_RECEIPT_KIND;
  if (projection.kind === "engram") return SARAH_ENGRAM_KIND;
  if (projection.kind === "read_state") return SARAH_READ_STATE_KIND;
  return SARAH_REMINDER_KIND;
};

export const decodeIssue31OwnerProjectionRecord = (
  value: unknown,
): Issue31OwnerProjectionRecord => {
  const record = decodeRecord(value, { onExcessProperty: "error" });
  if (record.sourceKind !== sourceKindForProjection(record.projection)) {
    throw new Error("Issue 31 owner projection source kind does not match its body.");
  }
  const expectedSourceRole =
    record.projection.kind === "message"
      ? record.projection.role
      : record.projection.kind === "read_state" || record.projection.kind === "reminder"
        ? "owner"
        : "sarah";
  if (record.sourceRole !== expectedSourceRole) {
    throw new Error("Issue 31 owner projection source role does not match its body.");
  }
  if (record.projectedAt < record.sourceCreatedAt) {
    throw new Error("Issue 31 owner projection predates its source event.");
  }
  if (
    record.projection.kind === "authority_receipt" &&
    record.projection.targetOutcome.state !== "pending" &&
    record.projection.targetOutcome.outcomeRef === undefined
  ) {
    throw new Error("Issue 31 terminal target outcome needs an outcome reference.");
  }
  if (
    record.projection.kind === "engram" &&
    parseEngramBody(record.projection.plaintext) === null
  ) {
    throw new Error("Issue 31 owner engram projection is invalid.");
  }
  if (record.projection.kind === "read_state") {
    let plaintext: unknown;
    try {
      plaintext = JSON.parse(record.projection.plaintext) as unknown;
    } catch {
      plaintext = null;
    }
    if (validateReadStateBlob(plaintext) === null) {
      throw new Error("Issue 31 owner read-state projection is invalid.");
    }
  }
  if (record.projection.kind === "reminder") {
    const reminder = parseReminderContent(record.projection.plaintext);
    if (reminder === null) {
      throw new Error("Issue 31 owner reminder projection is invalid.");
    }
    if (reminder.status === "pending" && record.projection.notBefore === undefined) {
      throw new Error("Issue 31 pending reminder projection needs not-before time.");
    }
    if (
      record.projection.expiration !== undefined &&
      record.projection.notBefore !== undefined &&
      record.projection.expiration <= record.projection.notBefore
    ) {
      throw new Error("Issue 31 reminder projection expiration is invalid.");
    }
  }
  return record;
};
