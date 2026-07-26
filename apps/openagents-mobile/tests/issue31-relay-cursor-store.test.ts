/**
 * Store isolation for the issue #31 rooms (omega#48).
 *
 * The exit is that the owner-private and community stores share no history,
 * membership, cursor, thread reference, or optimistic state. Keying entries by
 * room inside one record satisfied the *naming* half of that and none of the
 * rest: one record is one failure domain, one storage bound, and one deletion
 * unit. These tests pin the couplings that were measured on the combined
 * record so a future consolidation cannot quietly restore them.
 */
import { describe, expect, test } from "vite-plus/test";

import {
  ISSUE31_LEGACY_RELAY_CURSOR_STORE_KEY,
  MAX_ISSUE31_CURSORS_PER_ROOM,
  clearIssue31RelayCursorsForRoom,
  createIssue31SecureRelayCursorStore,
  issue31RelayCursorStoreKey,
} from "../src/workroom/issue31-relay-cursor-store.ts";

const memoryStore = () => {
  const map = new Map<string, string>();
  return {
    map,
    getItemAsync: async (key: string) => map.get(key) ?? null,
    setItemAsync: async (key: string, value: string) => {
      map.set(key, value);
    },
    deleteItemAsync: async (key: string) => {
      map.delete(key);
    },
  };
};

const cursor = (since: number) => ({ since, eventIdsAtSince: [] as ReadonlyArray<string> });

describe("issue 31 relay cursor store isolation", () => {
  test("keeps each room in its own record", async () => {
    const backing = memoryStore();
    const store = createIssue31SecureRelayCursorStore(backing);

    await store.save("wss://relay", "owner_private", cursor(100));
    await store.save("wss://relay", "community", cursor(200));

    expect(backing.map.has(issue31RelayCursorStoreKey("owner_private"))).toBe(true);
    expect(backing.map.has(issue31RelayCursorStoreKey("community"))).toBe(true);
    // The rooms never touch a shared record.
    expect(backing.map.has(ISSUE31_LEGACY_RELAY_CURSOR_STORE_KEY)).toBe(false);

    expect((await store.load("wss://relay", "owner_private"))?.since).toBe(100);
    expect((await store.load("wss://relay", "community"))?.since).toBe(200);
  });

  test("a corrupt community record cannot break owner-private reads", async () => {
    const backing = memoryStore();
    const store = createIssue31SecureRelayCursorStore(backing);
    await store.save("wss://relay", "owner_private", cursor(100));
    await store.save("wss://relay", "community", cursor(200));

    backing.map.set(issue31RelayCursorStoreKey("community"), "{not json");

    // The owner-private room is unaffected by damage it did not cause.
    expect((await store.load("wss://relay", "owner_private"))?.since).toBe(100);
    await expect(store.load("wss://relay", "community")).rejects.toThrow(/invalid/);
  });

  test("a busy community room cannot starve owner-private saves", async () => {
    const store = createIssue31SecureRelayCursorStore(memoryStore());
    for (let index = 0; index < MAX_ISSUE31_CURSORS_PER_ROOM; index += 1) {
      await store.save(`wss://community-${index}`, "community", cursor(index));
    }
    // The community room is now at its own bound. Owner-private has its own.
    await store.save("wss://owner", "owner_private", cursor(1));
    expect((await store.load("wss://owner", "owner_private"))?.since).toBe(1);

    await expect(
      store.save("wss://community-overflow", "community", cursor(99)),
    ).rejects.toThrow(/eight relays/);
  });

  test("clearing the community room leaves owner-private state intact", async () => {
    const backing = memoryStore();
    const store = createIssue31SecureRelayCursorStore(backing);
    await store.save("wss://relay", "owner_private", cursor(100));
    await store.save("wss://relay", "community", cursor(200));

    // Revocation must remove community access immediately, and only that.
    await clearIssue31RelayCursorsForRoom(backing, "community");

    expect(await store.load("wss://relay", "community")).toBeNull();
    expect((await store.load("wss://relay", "owner_private"))?.since).toBe(100);
  });

  test("migrates an installed combined record once, per room", async () => {
    const backing = memoryStore();
    backing.map.set(
      ISSUE31_LEGACY_RELAY_CURSOR_STORE_KEY,
      JSON.stringify({
        schemaVersion: 1,
        entries: {
          "owner_private:wss%3A%2F%2Frelay": { since: 100, eventIdsAtSince: [] },
          "community:wss%3A%2F%2Frelay": { since: 200, eventIdsAtSince: [] },
        },
      }),
    );
    const store = createIssue31SecureRelayCursorStore(backing);

    expect((await store.load("wss://relay", "owner_private"))?.since).toBe(100);
    expect((await store.load("wss://relay", "community"))?.since).toBe(200);
    expect(backing.map.has(issue31RelayCursorStoreKey("owner_private"))).toBe(true);
    expect(backing.map.has(issue31RelayCursorStoreKey("community"))).toBe(true);
  });

  test("a damaged legacy record costs a replay, not a permanent failure", async () => {
    const backing = memoryStore();
    backing.map.set(ISSUE31_LEGACY_RELAY_CURSOR_STORE_KEY, "{not json");
    const store = createIssue31SecureRelayCursorStore(backing);

    // Turning one room's old corruption into a permanent failure for a room
    // that has no reason to care about it is the coupling being removed here.
    expect(await store.load("wss://relay", "owner_private")).toBeNull();
    await store.save("wss://relay", "owner_private", cursor(5));
    expect((await store.load("wss://relay", "owner_private"))?.since).toBe(5);
  });

  test("does not carry another room's entries across the migration", async () => {
    const backing = memoryStore();
    backing.map.set(
      ISSUE31_LEGACY_RELAY_CURSOR_STORE_KEY,
      JSON.stringify({
        schemaVersion: 1,
        entries: { "community:wss%3A%2F%2Frelay": { since: 200, eventIdsAtSince: [] } },
      }),
    );
    const store = createIssue31SecureRelayCursorStore(backing);

    expect(await store.load("wss://relay", "owner_private")).toBeNull();
    expect((await store.load("wss://relay", "community"))?.since).toBe(200);
  });
});

/**
 * omega#49. The community cursor is a high-water mark over one group's records.
 * A build that changes the configured group inherited the previous group's mark
 * and asked the relay only for records above it, so a device whose admission
 * was already on the relay read an empty room with no gap and no refusal.
 */
describe("Issue31 community cursors are scoped to their group", () => {
  test("does not serve one group's cursor to another, and leaves the other rooms alone", async () => {
    const backing = memoryStore();
    const groupA = createIssue31SecureRelayCursorStore(backing, "group.a");
    const groupB = createIssue31SecureRelayCursorStore(backing, "group.b");
    const cursor = { since: 1_700_000_000, eventIdsAtSince: ["a".repeat(64)] };
    await groupA.save("wss://relay.example.com", "community", cursor);

    expect(await groupA.load("wss://relay.example.com", "community")).toEqual(cursor);
    // The whole point: group B replays from scratch rather than starting above
    // group A's history.
    expect(await groupB.load("wss://relay.example.com", "community")).toBeNull();

    // The owner-private room never carries a group, so its key is unchanged and
    // two differently-configured stores still share it.
    await groupA.save("wss://relay.example.com", "owner_private", cursor);
    expect(await groupB.load("wss://relay.example.com", "owner_private")).toEqual(cursor);
  });
});
