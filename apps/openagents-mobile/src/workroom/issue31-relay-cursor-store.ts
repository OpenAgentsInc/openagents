import { Schema } from "effect";

import type {
  Issue31NostrRoom,
  Issue31RelayCursor,
  Issue31RelayCursorStore,
} from "./issue31-nostr-client.ts";
import type { Issue31SecureStore } from "./issue31-device-key-vault.ts";

/**
 * The pre-partition key, where every room's cursors shared one record.
 *
 * Retained only so an installed app can migrate its existing cursors once
 * instead of replaying every room from scratch. Nothing writes to it.
 */
export const ISSUE31_LEGACY_RELAY_CURSOR_STORE_KEY =
  "openagents.omega.issue31.relay-cursors.v1" as const;

/**
 * Cursors are stored one record per room.
 *
 * omega#48 requires that the owner-private and community stores share no
 * history, membership, cursor, thread reference, or optimistic state. Keying
 * entries by room inside a single record is not enough to satisfy that: one
 * record means one failure domain, one storage bound, and one deletion unit.
 * Measured on the combined record, community traffic could break owner-private
 * reads through an unrelated corrupt entry, and could exhaust the shared entry
 * bound so owner-private saves began to fail.
 *
 * Separate keys make the rooms genuinely independent.
 */
export const issue31RelayCursorStoreKey = (room: Issue31NostrRoom): string =>
  `openagents.omega.issue31.relay-cursors.${room}.v2`;

/** Per room, so one busy room cannot consume another's capacity. */
export const MAX_ISSUE31_CURSORS_PER_ROOM = 8 as const;
const MAX_RECORD_BYTES = 32_768;

const CursorSchema = Schema.Struct({
  since: Schema.Number.check(
    Schema.isInt(),
    Schema.isGreaterThanOrEqualTo(0),
    Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
  ),
  eventIdsAtSince: Schema.Array(Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/))).check(
    Schema.isMaxLength(4),
  ),
});

const CursorRecordSchema = Schema.Struct({
  schemaVersion: Schema.Literal(1),
  entries: Schema.Record(Schema.String, CursorSchema),
});

interface CursorRecord extends Schema.Schema.Type<typeof CursorRecordSchema> {}

const emptyRecord = (): CursorRecord => ({ schemaVersion: 1, entries: {} });

/**
 * One cursor per relay — and, in the community room, per group.
 *
 * The community cursor is a high-water mark over records addressed to one
 * group's `h` tag. The group id is build-time configuration, so an upgrade that
 * points the app at a different group inherits the previous group's cursor and
 * asks the relay only for records newer than it. The new group's history is
 * older than that mark, so the relay correctly returns nothing and the room
 * renders "No signed membership record has arrived for this group" **forever**,
 * on a device whose admission is sitting on the relay. It fails invisibly: no
 * gap, no refusal, nothing to quarantine — the same shape as omega#49's NIP-59
 * backdating defect, one layer up.
 *
 * Scoping the entry by group makes a group change replay that group from
 * scratch, which is the only correct answer for a room this device has never
 * followed.
 */
const cursorKey = (
  relayUrl: string,
  room: Issue31NostrRoom,
  communityGroupId: string | null,
): string =>
  room === "community" && communityGroupId !== null
    ? `${encodeURIComponent(relayUrl)}#${encodeURIComponent(communityGroupId)}`
    : encodeURIComponent(relayUrl);

/** The combined-record entry key this room used before the partition. */
const legacyCursorKey = (relayUrl: string, room: Issue31NostrRoom): string =>
  `${room}:${encodeURIComponent(relayUrl)}`;

const decodeRecord = (value: string): CursorRecord => {
  if (value.length > MAX_RECORD_BYTES) {
    throw new Error("The Omega relay cursor record is too large.");
  }
  try {
    return Schema.decodeUnknownSync(CursorRecordSchema)(JSON.parse(value), {
      onExcessProperty: "error",
    });
  } catch {
    throw new Error("The Omega relay cursor record is invalid.");
  }
};

export const createIssue31SecureRelayCursorStore = (
  store: Issue31SecureStore,
  /**
   * The community group this device is configured for, when it has one.
   * Absent keeps the pre-scoping key, so a build with no community room reads
   * and writes exactly what it did before.
   */
  communityGroupId: string | null = null,
): Issue31RelayCursorStore => {
  let writes: Promise<void> = Promise.resolve();
  const migrated = new Set<Issue31NostrRoom>();

  const read = async (room: Issue31NostrRoom): Promise<CursorRecord> => {
    const value = await store.getItemAsync(issue31RelayCursorStoreKey(room));
    if (value !== null) return decodeRecord(value);
    return migrateLegacy(room);
  };

  /**
   * Lift this room's entries out of the pre-partition record, once.
   *
   * A damaged legacy record is not an error here: cursors are a replay
   * optimization, and the client already handles an absent cursor by replaying.
   * Failing the migration would turn one room's old corruption into a
   * permanent failure for a room that has no other reason to care about it —
   * exactly the coupling the partition removes.
   */
  const migrateLegacy = async (room: Issue31NostrRoom): Promise<CursorRecord> => {
    if (migrated.has(room)) return emptyRecord();
    migrated.add(room);
    const legacy = await store.getItemAsync(ISSUE31_LEGACY_RELAY_CURSOR_STORE_KEY);
    if (legacy === null) return emptyRecord();

    let record: CursorRecord;
    try {
      record = decodeRecord(legacy);
    } catch {
      return emptyRecord();
    }

    const prefix = `${room}:`;
    const entries: Record<string, Schema.Schema.Type<typeof CursorSchema>> = {};
    for (const [key, cursor] of Object.entries(record.entries)) {
      if (!key.startsWith(prefix)) continue;
      entries[key.slice(prefix.length)] = cursor;
      if (Object.keys(entries).length >= MAX_ISSUE31_CURSORS_PER_ROOM) break;
    }
    if (Object.keys(entries).length === 0) return emptyRecord();

    const next: CursorRecord = { schemaVersion: 1, entries };
    await store.setItemAsync(issue31RelayCursorStoreKey(room), JSON.stringify(next));
    return next;
  };

  return {
    load: async (relayUrl, room) => (await read(room)).entries[cursorKey(relayUrl, room, communityGroupId)] ?? null,
    save: async (relayUrl, room, cursor: Issue31RelayCursor) => {
      const write = writes.then(async () => {
        const current = await read(room);
        const next = Schema.decodeUnknownSync(CursorRecordSchema)(
          {
            schemaVersion: 1,
            entries: {
              ...current.entries,
              [cursorKey(relayUrl, room, communityGroupId)]: {
                since: cursor.since,
                eventIdsAtSince: [...new Set(cursor.eventIdsAtSince)].sort().slice(-4),
              },
            },
          },
          { onExcessProperty: "error" },
        );
        if (Object.keys(next.entries).length > MAX_ISSUE31_CURSORS_PER_ROOM) {
          throw new Error("The Omega relay cursor record exceeds eight relays for one room.");
        }
        const serialized = JSON.stringify(next);
        if (serialized.length > MAX_RECORD_BYTES) {
          throw new Error("The Omega relay cursor record exceeds its storage bound.");
        }
        await store.setItemAsync(issue31RelayCursorStoreKey(room), serialized);
      });
      writes = write;
      await write;
    },
  };
};

/**
 * Drop one room's cursors without touching any other room.
 *
 * omega#48 requires that member and agent revocation removes community access
 * immediately. Clearing community state must never disturb the owner-private
 * room, which the combined record made impossible.
 */
export const clearIssue31RelayCursorsForRoom = async (
  store: Issue31SecureStore,
  room: Issue31NostrRoom,
): Promise<void> => {
  await store.deleteItemAsync(issue31RelayCursorStoreKey(room));
};
