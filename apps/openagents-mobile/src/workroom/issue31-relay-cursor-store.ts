import { Schema } from "effect";

import type {
  Issue31NostrRoom,
  Issue31RelayCursor,
  Issue31RelayCursorStore,
} from "./issue31-nostr-client.ts";
import type { Issue31SecureStore } from "./issue31-device-key-vault.ts";

export const ISSUE31_RELAY_CURSOR_STORE_KEY = "openagents.omega.issue31.relay-cursors.v1" as const;

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

const cursorKey = (relayUrl: string, room: Issue31NostrRoom): string =>
  `${room}:${encodeURIComponent(relayUrl)}`;

export const createIssue31SecureRelayCursorStore = (
  store: Issue31SecureStore,
): Issue31RelayCursorStore => {
  let writes: Promise<void> = Promise.resolve();

  const read = async (): Promise<CursorRecord> => {
    const value = await store.getItemAsync(ISSUE31_RELAY_CURSOR_STORE_KEY);
    if (value === null) return emptyRecord();
    if (value.length > 32_768) throw new Error("The Omega relay cursor record is too large.");
    try {
      return Schema.decodeUnknownSync(CursorRecordSchema)(JSON.parse(value), {
        onExcessProperty: "error",
      });
    } catch {
      throw new Error("The Omega relay cursor record is invalid.");
    }
  };

  return {
    load: async (relayUrl, room) => (await read()).entries[cursorKey(relayUrl, room)] ?? null,
    save: async (relayUrl, room, cursor) => {
      const write = writes.then(async () => {
        const current = await read();
        const next = Schema.decodeUnknownSync(CursorRecordSchema)(
          {
            schemaVersion: 1,
            entries: {
              ...current.entries,
              [cursorKey(relayUrl, room)]: {
                since: cursor.since,
                eventIdsAtSince: [...new Set(cursor.eventIdsAtSince)].sort().slice(-4),
              },
            },
          },
          { onExcessProperty: "error" },
        );
        if (Object.keys(next.entries).length > 24) {
          throw new Error("The Omega relay cursor record exceeds eight relays and three rooms.");
        }
        const serialized = JSON.stringify(next);
        if (serialized.length > 32_768) {
          throw new Error("The Omega relay cursor record exceeds its storage bound.");
        }
        await store.setItemAsync(ISSUE31_RELAY_CURSOR_STORE_KEY, serialized);
      });
      writes = write;
      await write;
    },
  };
};
