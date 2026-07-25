/**
 * Persisted community records (omega#48).
 *
 * ## Why this exists
 *
 * The community room subscribed, folded, and projected — and kept every record
 * in memory only. The client advances a per-room replay cursor, so the next
 * launch subscribes with `since: cursor - 1` and the relay correctly declines to
 * re-serve anything older. Nothing re-read them from disk, because nothing wrote
 * them. So a restart did not merely lose the transcript: it lost the **burn
 * set**.
 *
 * That is a revocation hole, not a cache miss. Concretely, on a real wire:
 *
 * 1. `9000` admits an operator, the operator's agent publishes its persona, and
 *    `9001` revokes the agent key. The fold burns the key.
 * 2. The app restarts. The cursor is past all three, so the relay serves none of
 *    them, and the in-memory ledger starts empty.
 * 3. The operator is re-admitted and the agent re-publishes the same persona.
 *    The fold sees an admission and an attestation with **no revocation in the
 *    stream**, and admits the burned key.
 *
 * The ledger fold was already order-independent and already survived
 * re-invitation and replay; it could not survive a restart that never handed it
 * the revocation. Revocation is a claim about a subject that must hold
 * regardless of arrival order, re-invitation, **or restart** — so the record
 * that carries it has to outlive the process.
 *
 * ## Two rooms, two databases
 *
 * The owner-private room persists into `openagents-omega-issue31.db`. This store
 * opens a **different file**. That is not tidiness: the exit says the two stores
 * share no history, and a shared file shares a page cache, a write lock, a
 * corruption blast radius, and — if the two ever shared a table — a row bound
 * where one room's volume could evict the other's history.
 *
 * Two further guards make the separation a property rather than a convention:
 *
 * - {@link createIssue31CommunityRecordStore} refuses any kind outside
 *   {@link ISSUE31_COMMUNITY_KINDS}, so an owner-private gift wrap, turn record,
 *   engram, or reminder cannot be written here even by a caller that confuses
 *   the two stores.
 * - A store instance is bound to one group id, and a group-scoped record that
 *   names a different group is refused. A row that is loaded is a row that named
 *   this room.
 *
 * ## The bound must not launder a revocation
 *
 * Any bounded log needs an eviction rule, and the naive one — drop the oldest
 * row — is the same hole again with a different trigger: a busy room could push
 * a `9001` out of the window and re-admit the key it burned. Eviction here is
 * revocation-preserving. `9001` rows are never discarded; when only revocations
 * remain, the store refuses the write and says so rather than quietly forgetting
 * one. Refusing to store a new chat message is a visible, recoverable failure.
 * Silently un-revoking a key is not.
 */
import type { Issue31SignedNostrEvent } from "@openagentsinc/sarah/issue31-nostr";
import { NIP_29_REMOVE_USER_KIND } from "@openagentsinc/sarah/community";
import { verifyEvent } from "nostr-effect/pure";

import {
  ISSUE31_COMMUNITY_GROUP_SCOPED_KINDS,
  ISSUE31_COMMUNITY_KINDS,
  type Issue31ConfirmedEvent,
} from "./issue31-nostr-client.ts";

declare const require: (id: string) => unknown;

/**
 * The minimal synchronous SQLite surface this store uses.
 *
 * Deliberately the same shape `expo-sqlite` exposes, so the production path is a
 * thin wrapper and the tests can drive the identical code against a real
 * `node:sqlite` database on a real file — including a genuine close-and-reopen,
 * which is the only honest way to test a restart.
 */
export interface Issue31CommunityDatabase {
  readonly execSync: (sql: string) => void;
  readonly runSync: (sql: string, ...params: ReadonlyArray<string | number>) => unknown;
  readonly getAllSync: <Row>(
    sql: string,
    ...params: ReadonlyArray<string | number>
  ) => ReadonlyArray<Row>;
  readonly closeSync: () => void;
}

export interface Issue31CommunityRecordStore {
  /** Every persisted record for this store's group, in record order. */
  readonly load: () => ReadonlyArray<Issue31SignedNostrEvent>;
  /** Persist one admitted community record. Idempotent by event id. */
  readonly put: (event: Issue31SignedNostrEvent) => void;
  readonly close: () => void;
}

/** The database file the community room persists into. Never the private one. */
export const ISSUE31_COMMUNITY_DATABASE_NAME = "openagents-omega-issue31-community.db" as const;

/** The database file the owner-private room persists into. */
export const ISSUE31_OWNER_PRIVATE_DATABASE_NAME = "openagents-omega-issue31.db" as const;

const HEX_64 = /^[0-9a-f]{64}$/;
const HEX_128 = /^[0-9a-f]{128}$/;

const COMMUNITY_KIND_SET: ReadonlySet<number> = new Set(ISSUE31_COMMUNITY_KINDS);
const GROUP_SCOPED_KIND_SET: ReadonlySet<number> = new Set(
  ISSUE31_COMMUNITY_GROUP_SCOPED_KINDS,
);

export class Issue31CommunityRecordStoreError extends Error {
  readonly code:
    | "not_a_community_kind"
    | "wrong_group"
    | "invalid_event"
    | "conflicting_event"
    | "full_of_revocations";
  constructor(
    code: Issue31CommunityRecordStoreError["code"],
    message: string,
  ) {
    super(message);
    this.code = code;
  }
}

const decodeStoredCommunityEvent = (value: unknown): Issue31SignedNostrEvent => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Issue31CommunityRecordStoreError(
      "invalid_event",
      "A persisted community record is not an object.",
    );
  }
  const candidate = value as Readonly<Record<string, unknown>>;
  if (
    typeof candidate["id"] !== "string" ||
    !HEX_64.test(candidate["id"]) ||
    typeof candidate["pubkey"] !== "string" ||
    !HEX_64.test(candidate["pubkey"]) ||
    typeof candidate["sig"] !== "string" ||
    !HEX_128.test(candidate["sig"]) ||
    !Number.isSafeInteger(candidate["created_at"]) ||
    typeof candidate["kind"] !== "number" ||
    !Number.isSafeInteger(candidate["kind"]) ||
    !Array.isArray(candidate["tags"]) ||
    candidate["tags"].length > 128 ||
    !candidate["tags"].every(
      (tag) =>
        Array.isArray(tag) &&
        tag.length <= 16 &&
        tag.every((part) => typeof part === "string" && part.length <= 4_096),
    ) ||
    typeof candidate["content"] !== "string" ||
    candidate["content"].length > 524_288
  ) {
    throw new Issue31CommunityRecordStoreError(
      "invalid_event",
      "A persisted community record has an invalid shape.",
    );
  }
  const event = candidate as unknown as Issue31SignedNostrEvent;
  // A record read back off disk is verified exactly as one off the wire. Local
  // storage is not an authority either.
  if (!verifyEvent({ ...event, tags: event.tags.map((tag) => [...tag]) })) {
    throw new Issue31CommunityRecordStoreError(
      "invalid_event",
      "A persisted community record has an invalid signature.",
    );
  }
  return event;
};

/**
 * Check one event against this store's admission rules.
 *
 * Exported so the rules can be falsified directly rather than only through
 * SQLite.
 */
export const assertIssue31CommunityRecordStorable = (input: {
  readonly event: Issue31SignedNostrEvent;
  readonly groupId: string;
}): void => {
  if (!COMMUNITY_KIND_SET.has(input.event.kind)) {
    throw new Issue31CommunityRecordStoreError(
      "not_a_community_kind",
      `The community record store refuses kind ${input.event.kind}; it is not a community kind. The owner-private room has its own store.`,
    );
  }
  if (GROUP_SCOPED_KIND_SET.has(input.event.kind)) {
    const named = input.event.tags.some(
      (tag) => (tag[0] === "h" || tag[0] === "d") && tag[1] === input.groupId,
    );
    if (!named) {
      throw new Issue31CommunityRecordStoreError(
        "wrong_group",
        "The community record store refuses a group-scoped record that does not name this group.",
      );
    }
  }
};

/**
 * Which rows to discard to make room for one more, or a refusal.
 *
 * Pure, so the revocation-preserving rule can be tested without a database and
 * falsified by swapping it for "drop the oldest".
 */
export const issue31CommunityEvictionPlan = (input: {
  readonly rows: ReadonlyArray<Readonly<{ sequence: number; kind: number }>>;
  readonly maximumRecords: number;
}): ReadonlyArray<number> => {
  const overflow = input.rows.length + 1 - input.maximumRecords;
  if (overflow <= 0) return [];
  const evictable = [...input.rows]
    .sort((left, right) => left.sequence - right.sequence)
    .filter((row) => row.kind !== NIP_29_REMOVE_USER_KIND);
  if (evictable.length < overflow) {
    throw new Issue31CommunityRecordStoreError(
      "full_of_revocations",
      "The community record store is full of revocations; none was discarded. A revocation a bound forgets is not a revocation.",
    );
  }
  return evictable.slice(0, overflow).map((row) => row.sequence);
};

export const createIssue31CommunityRecordStore = (input: {
  readonly database: Issue31CommunityDatabase;
  readonly groupId: string;
  readonly maximumRecords?: number;
}): Issue31CommunityRecordStore => {
  const maximumRecords = input.maximumRecords ?? 2_048;
  if (
    !Number.isSafeInteger(maximumRecords) ||
    maximumRecords < 8 ||
    maximumRecords > 16_384
  ) {
    throw new Issue31CommunityRecordStoreError(
      "invalid_event",
      "The community record bound is invalid.",
    );
  }
  const groupId = input.groupId.trim();
  if (groupId === "" || groupId.length > 128 || /\s/.test(groupId)) {
    throw new Issue31CommunityRecordStoreError(
      "wrong_group",
      "The community record store needs a single well-formed group id.",
    );
  }
  const database = input.database;
  database.execSync(`
    PRAGMA journal_mode = WAL;
    CREATE TABLE IF NOT EXISTS issue31_community_records (
      sequence INTEGER PRIMARY KEY AUTOINCREMENT,
      group_id TEXT NOT NULL,
      event_id TEXT NOT NULL UNIQUE,
      kind INTEGER NOT NULL,
      created_at INTEGER NOT NULL,
      event_json TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS issue31_community_records_group
      ON issue31_community_records (group_id, created_at);
  `);
  return {
    load: () =>
      database
        .getAllSync<Readonly<{ event_json: string }>>(
          "SELECT event_json FROM issue31_community_records WHERE group_id = ? ORDER BY created_at ASC, event_id ASC LIMIT ?",
          groupId,
          maximumRecords,
        )
        .map((row) => {
          const event = decodeStoredCommunityEvent(JSON.parse(row.event_json) as unknown);
          // A row that was written under a different rule set — an older build,
          // a tampered file — is still checked on the way out.
          assertIssue31CommunityRecordStorable({ event, groupId });
          return event;
        }),
    put: (event) => {
      assertIssue31CommunityRecordStorable({ event, groupId });
      if (!verifyEvent({ ...event, tags: event.tags.map((tag) => [...tag]) })) {
        throw new Issue31CommunityRecordStoreError(
          "invalid_event",
          "The community record store refuses an event with an invalid signature.",
        );
      }
      const eventJson = JSON.stringify(event);
      if (eventJson.length > 524_288) {
        throw new Issue31CommunityRecordStoreError(
          "invalid_event",
          "The community record exceeds its storage bound.",
        );
      }
      const existing = database.getAllSync<Readonly<{ event_json: string }>>(
        "SELECT event_json FROM issue31_community_records WHERE event_id = ? LIMIT 1",
        event.id,
      )[0];
      if (existing !== undefined) {
        if (existing.event_json !== eventJson) {
          throw new Issue31CommunityRecordStoreError(
            "conflicting_event",
            "A persisted community record identifier conflicts.",
          );
        }
        return;
      }
      const rows = database.getAllSync<Readonly<{ sequence: number; kind: number }>>(
        "SELECT sequence, kind FROM issue31_community_records WHERE group_id = ?",
        groupId,
      );
      // Throws rather than evicting when only revocations are left. The caller
      // sees a refusal; nothing is silently forgotten.
      for (const sequence of issue31CommunityEvictionPlan({ rows, maximumRecords })) {
        database.runSync("DELETE FROM issue31_community_records WHERE sequence = ?", sequence);
      }
      database.runSync(
        "INSERT INTO issue31_community_records (group_id, event_id, kind, created_at, event_json) VALUES (?, ?, ?, ?, ?)",
        groupId,
        event.id,
        event.kind,
        event.created_at,
        eventJson,
      );
    },
    close: () => database.closeSync(),
  };
};

export const openExpoIssue31CommunityRecordStore = (
  groupId: string,
  maximumRecords = 2_048,
): Issue31CommunityRecordStore => {
  const sqlite = require("expo-sqlite") as Readonly<{
    openDatabaseSync: (name: string) => Issue31CommunityDatabase;
  }>;
  return createIssue31CommunityRecordStore({
    // A different file from the owner-private room, on purpose.
    database: sqlite.openDatabaseSync(ISSUE31_COMMUNITY_DATABASE_NAME),
    groupId,
    maximumRecords,
  });
};

/** The relay url persisted community history is attributed to. */
export const ISSUE31_COMMUNITY_STORE_RELAY_URL = "local://issue31-community-store" as const;

/**
 * Persisted records as confirmed community events, ready to merge into a
 * snapshot the projection already understands.
 */
export const issue31CommunityConfirmedEventsFrom = (
  events: ReadonlyArray<Issue31SignedNostrEvent>,
): ReadonlyArray<Issue31ConfirmedEvent> =>
  events.map((event) => ({
    relayUrl: ISSUE31_COMMUNITY_STORE_RELAY_URL,
    room: "community" as const,
    event,
    canonicalRecordId: event.id,
    privateRumorId: null,
    privateRecord: null,
    hostAnnouncement: null,
  }));

/**
 * Merge persisted community history into a live snapshot.
 *
 * The live rows win on a collision — the relay's copy of a record this device
 * already stored is the same record — and persisted rows fill in everything the
 * replay cursor has moved past. The fold this feeds is order-independent, so a
 * revocation restored here burns its subject no matter where in the merged
 * sequence it lands.
 */
export const issue31MergeCommunityHistory = <
  Snapshot extends Readonly<{ confirmedEvents: ReadonlyArray<Issue31ConfirmedEvent> }>,
>(
  snapshot: Snapshot,
  persisted: ReadonlyArray<Issue31ConfirmedEvent>,
): Snapshot => {
  const liveIds = new Set(snapshot.confirmedEvents.map((row) => row.event.id));
  const restored = persisted.filter((row) => !liveIds.has(row.event.id));
  if (restored.length === 0) return snapshot;
  return {
    ...snapshot,
    confirmedEvents: [...restored, ...snapshot.confirmedEvents].sort(
      (left, right) =>
        left.event.created_at - right.event.created_at ||
        left.event.id.localeCompare(right.event.id),
    ),
  };
};
