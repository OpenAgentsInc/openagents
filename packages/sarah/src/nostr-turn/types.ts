import { Schema as S } from "effect";

/** Durable turn-record kind (SARAH-NR-00). */
export const SARAH_TURN_RECORD_KIND = 44300 as const;

/** Durable authority-receipt kind (SARAH-NR-00). */
export const SARAH_AUTHORITY_RECEIPT_KIND = 44301 as const;

/** Live telemetry kind (NIP-AO ephemeral). */
export const SARAH_NIP_AO_KIND = 24200 as const;

/** NIP-AM metric kind. */
export const SARAH_NIP_AM_KIND = 44200 as const;

export const SARAH_TURN_RECORD_SCHEMA = "openagents.sarah.turn_record.v1" as const;

export const SarahTurnEntry = S.Literals([
  "turn.started",
  "tool.call",
  "tool.result",
  "tool.error",
  "turn.finished",
  "turn.interrupted",
]);
export type SarahTurnEntry = S.Schema.Type<typeof SarahTurnEntry>;

const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));
const ConversationTag = S.String.check(S.isPattern(/^sarah\.[0-9a-f]{24}$/));

export const SarahTurnParent = S.Struct({
  eventId: Hex64,
  marker: S.Literals(["prompt", "reply", "root", "mention", "tool", "prior"]),
});
export type SarahTurnParent = S.Schema.Type<typeof SarahTurnParent>;

export const SarahTurnRecordPayload = S.Struct({
  schema: S.Literal(SARAH_TURN_RECORD_SCHEMA),
  entry: SarahTurnEntry,
  conversation: ConversationTag,
  turnRef: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  seq: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  timestamp: S.String.check(S.isMinLength(1)),
  parents: S.Array(SarahTurnParent),
  payload: S.Record(S.String, S.Unknown),
});
export type SarahTurnRecordPayload = S.Schema.Type<typeof SarahTurnRecordPayload>;

export interface SarahTurnConversation {
  readonly ownerPubkey: string;
  readonly sarahPubkey: string;
  readonly conversation: string;
}

/** Claimed turn slot — exactly one writer may hold a turnRef. */
export interface SarahTurnClaim {
  readonly turnRef: string;
  readonly conversation: string;
  readonly claimedAtMs: number;
  readonly claimEventId?: string;
}

export interface SarahNostrCipher {
  /** Encrypt plaintext to the owner. Must not log plaintext. */
  readonly encryptToOwner: (plaintext: string) => string;
}

export const TURN_RECORD_ALT = "OpenAgents Sarah turn record (encrypted)" as const;
