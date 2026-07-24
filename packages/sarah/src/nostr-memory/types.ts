import { Schema as S } from "effect";

/** NIP-AE agent engram kind (addressable). */
export const SARAH_ENGRAM_KIND = 30174 as const;

/** NIP-RS / NIP-78 read-state kind (addressable). */
export const SARAH_READ_STATE_KIND = 30078 as const;

/** NIP-ER event reminder kind (addressable). */
export const SARAH_REMINDER_KIND = 30300 as const;

/** NIP-09 deletion request kind. */
export const SARAH_DELETION_KIND = 5 as const;

/** Domain separator for HMAC-blinded engram `d` tags. */
export const ENGRAM_D_TAG_DOMAIN = "agent-memory/v1/d-tag" as const;

/** NIP-31 alt for engrams (NIP-AE default). */
export const ENGRAM_ALT = "encrypted agent memory record" as const;

/** NIP-31 alt for read-state blobs. */
export const READ_STATE_ALT = "encrypted read state" as const;

/** NIP-31 alt for reminders. */
export const REMINDER_ALT = "Encrypted reminder" as const;

/** OpenAgents companion schema id inside engram bodies. */
export const SARAH_NIP_AE_COMPANION_SCHEMA =
  "openagents.sarah.nip_ae_companion.v1" as const;

/** Extension id for relay supported_extensions (not a new NIP number). */
export const SARAH_NIP_AE_COMPANION_EXTENSION =
  "openagents.sarah.nip_ae_companion" as const;

export const CORE_SLUG = "core" as const;

/** Memory slug grammar: mem/… path segments. */
export const MEMORY_SLUG_PATTERN =
  /^mem\/[a-z0-9][a-z0-9_-]{0,63}(\/[a-z0-9][a-z0-9_-]{0,63})*$/;

export const MAX_ENGRAM_PLAINTEXT_BYTES = 65535 as const;
export const MAX_SLUG_BYTES = 255 as const;

export const READ_STATE_VERSION = 1 as const;
export const READ_STATE_T_VALUE = "read-state" as const;
export const READ_STATE_D_PREFIX = "read-state:" as const;
export const MAX_CONTEXT_ENTRIES = 10_000 as const;
export const MAX_CONTEXT_ID_BYTES = 256 as const;
export const MAX_CONTEXT_TIMESTAMP = 4_294_967_295 as const;
export const MAX_CLIENT_ID_LENGTH = 64 as const;

const Hex64 = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));
const EntityId = S.String.check(S.isPattern(/^entity\.[0-9a-f]{24}$/));
const ContentDigest = S.String.check(
  S.isPattern(/^sha256:[0-9a-f]{64}$/),
);
const RelationType = S.String.check(S.isPattern(/^[a-z][a-z0-9_]{0,63}$/));

export const SarahMemoryAdmission = S.Literals([
  "admitted",
  "candidate",
  "rejected",
]);
export type SarahMemoryAdmission = S.Schema.Type<typeof SarahMemoryAdmission>;

export const SarahMemorySourceRole = S.Literals([
  "turn_record",
  "tool_result",
  "owner_message",
  "import",
]);
export type SarahMemorySourceRole = S.Schema.Type<typeof SarahMemorySourceRole>;

export const SarahMemoryRelationDirection = S.Literals(["out", "in", "both"]);
export type SarahMemoryRelationDirection = S.Schema.Type<
  typeof SarahMemoryRelationDirection
>;

export const SarahMemorySourceEventRef = S.Struct({
  eventId: Hex64,
  role: SarahMemorySourceRole,
});
export type SarahMemorySourceEventRef = S.Schema.Type<
  typeof SarahMemorySourceEventRef
>;

export const SarahMemoryRelation = S.Struct({
  type: RelationType,
  targetSlug: S.String.check(S.isMinLength(1), S.isMaxLength(255)),
  direction: SarahMemoryRelationDirection,
});
export type SarahMemoryRelation = S.Schema.Type<typeof SarahMemoryRelation>;

/** OpenAgents companion fields under the NIP-AE unknown-fields rule. */
export const SarahNipAeCompanion = S.Struct({
  schema: S.Literal(SARAH_NIP_AE_COMPANION_SCHEMA),
  admission: SarahMemoryAdmission,
  entityId: EntityId,
  contentDigest: ContentDigest,
  sourceEventRefs: S.Array(SarahMemorySourceEventRef),
  relations: S.Array(SarahMemoryRelation),
  derivedFromSlugs: S.Array(S.String),
});
export type SarahNipAeCompanion = S.Schema.Type<typeof SarahNipAeCompanion>;

/** Core engram body (NIP-AE). */
export interface SarahEngramCoreBody {
  readonly slug: "core";
  readonly profile: string;
}

/** Memory engram body (NIP-AE + optional OpenAgents companion). */
export interface SarahEngramMemoryBody {
  readonly slug: string;
  readonly value: string | null;
  readonly openagents?: SarahNipAeCompanion;
}

export type SarahEngramBody = SarahEngramCoreBody | SarahEngramMemoryBody;

/**
 * Cipher port for memory / read-state / reminder content.
 * Production injects NIP-44; tests inject a reversible fixture cipher.
 * Must not log plaintext.
 */
export interface SarahNostrMemoryCipher {
  readonly encryptToOwner: (plaintext: string) => string;
  readonly decryptFromOwner: (ciphertext: string) => string;
}

/** Scope for Sarah-authored engrams (agent → owner). */
export interface SarahEngramConversation {
  readonly ownerPubkey: string;
  readonly sarahPubkey: string;
  /**
   * 32-byte NIP-44 conversation key `K_c` as lowercase hex.
   * Used only for HMAC-blinded `d` tags in this package (not for encryption;
   * encryption goes through the cipher port).
   */
  readonly conversationKeyHex: string;
}

/** Flat map of context id → read-frontier unix timestamp (seconds). */
export type SarahReadContexts = Readonly<Record<string, number>>;

export interface SarahReadStateBlob {
  readonly v: typeof READ_STATE_VERSION;
  readonly client_id: string;
  readonly contexts: SarahReadContexts;
}

export const SarahReminderStatus = S.Literals([
  "pending",
  "done",
  "cancelled",
]);
export type SarahReminderStatus = S.Schema.Type<typeof SarahReminderStatus>;

export interface SarahReminderTarget {
  readonly id?: string;
  readonly a?: string;
  readonly relays?: ReadonlyArray<string>;
  readonly preview?: string;
}

export interface SarahReminderContent {
  readonly target?: SarahReminderTarget;
  readonly status: SarahReminderStatus;
  readonly note?: string;
}

/** Names forbidden on durable engram / companion bodies (ranking lives only in projection). */
export const FORBIDDEN_DURABLE_MEMORY_FIELDS: ReadonlyArray<string> = [
  "ranking",
  "feedback_weight",
  "score",
  "weight",
  "rank",
  "embedding",
  "vector",
];
