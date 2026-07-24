import { Schema as S } from "effect";

/** Wire schema for a public-safe migration export/rollback manifest. */
export const SARAH_NOSTR_MIGRATION_MANIFEST_SCHEMA =
  "openagents.sarah.nostr_migration_manifest.v1" as const;

/**
 * Ordered migration stages (SARAH-NR-08 §24.9).
 *
 * 1. shadow — dual-publish; Khala Sync remains the record
 * 2. cutover — relay is the record; Khala is a derived projection
 * 3. retirement — Khala write path for the Sarah lane is stopped
 */
export const SarahNostrMigrationStage = S.Literals([
  "shadow",
  "cutover",
  "retirement",
]);
export type SarahNostrMigrationStage = S.Schema.Type<
  typeof SarahNostrMigrationStage
>;

/**
 * Process env feature flag values for SARAH_NOSTR_RECORD_MODE.
 *
 * - khala  — no Nostr publish (default production)
 * - shadow — dual-publish; Khala remains authority
 * - nostr  — relay is the record (cutover or retirement stage)
 */
export const SarahNostrRecordMode = S.Literals(["khala", "shadow", "nostr"]);
export type SarahNostrRecordMode = S.Schema.Type<typeof SarahNostrRecordMode>;

export const SARAH_NOSTR_RECORD_MODE_ENV = "SARAH_NOSTR_RECORD_MODE" as const;

/**
 * Legacy opt-in from SARAH-NR-05. Treated as mode=shadow when RECORD_MODE is
 * unset. Prefer SARAH_NOSTR_RECORD_MODE for new deploys.
 */
export const SARAH_NOSTR_SHADOW_PUBLISH_ENV = "SARAH_NOSTR_SHADOW_PUBLISH" as const;

const Digest24 = S.String.check(S.isPattern(/^[0-9a-f]{24}$/));
const ConversationTag = S.String.check(S.isPattern(/^sarah\.[0-9a-f]{24}$/));
const ThreadRef = S.String.check(S.isPattern(/^thread\.sarah\.[0-9a-f]{24}$/));
const EventIdHex = S.String.check(S.isPattern(/^[0-9a-f]{64}$/));

export const SarahNostrMigrationManifest = S.Struct({
  schema: S.Literal(SARAH_NOSTR_MIGRATION_MANIFEST_SCHEMA),
  stage: SarahNostrMigrationStage,
  conversation: ConversationTag,
  threadRef: ThreadRef,
  digest: Digest24,
  exportedAt: S.String.check(S.isMinLength(1)),
  eventCount: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  /** Ordered public-safe Nostr event ids (64 hex). Never includes content. */
  eventIds: S.Array(EventIdHex),
  /** SHA-256 hex of joined eventIds for integrity of the export set. */
  digestChain: S.String.check(S.isPattern(/^[0-9a-f]{64}$/)),
  /** ISO-8601 UTC when the rollback window closes (cutover/retirement). */
  rollbackWindowClosesAt: S.optional(S.String.check(S.isMinLength(1))),
  /** Public-safe note; no secrets, prompts, or private paths. */
  note: S.optional(S.String.check(S.isMaxLength(500))),
});
export type SarahNostrMigrationManifest = S.Schema.Type<
  typeof SarahNostrMigrationManifest
>;

/** Public-safe Khala-shaped durable ladder entry for drift comparison. */
export interface KhalaShapedEvent {
  readonly kind:
    | "turn.started"
    | "tool.call"
    | "tool.result"
    | "tool.error"
    | "turn.finished"
    | "turn.interrupted";
  readonly seq: number;
  readonly turnRef: string;
}

/** Public-safe Nostr durable turn-record projection for drift comparison. */
export interface NostrDurableEventProjection {
  readonly entry:
    | "turn.started"
    | "tool.call"
    | "tool.result"
    | "tool.error"
    | "turn.finished"
    | "turn.interrupted";
  readonly seq: number;
  readonly turnRef: string;
  readonly eventId?: string;
}

export type DriftMismatchKind =
  | "missing_on_nostr"
  | "missing_on_khala"
  | "entry_mismatch"
  | "seq_mismatch";

export interface DriftItem {
  readonly kind: DriftMismatchKind;
  readonly turnRef: string;
  readonly seq: number;
  readonly khalaEntry?: string;
  readonly nostrEntry?: string;
  readonly eventId?: string;
}

export interface DriftReport {
  readonly ok: boolean;
  readonly matched: number;
  readonly khalaCount: number;
  readonly nostrCount: number;
  readonly items: ReadonlyArray<DriftItem>;
}
