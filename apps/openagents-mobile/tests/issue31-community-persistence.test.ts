/**
 * Community history that survives a restart (omega#48).
 *
 * The prior lane proved the ledger fold is order-independent, survives replay,
 * and survives re-invitation — and closed by recording that community records
 * were still not persisted at all, so the "owner-private and community stores
 * share no history" exit held only vacuously. It held vacuously because there
 * was no community store. This file is that store, and the restart it makes
 * testable turns out to have been hiding a fourth revocation hole.
 *
 * ## The hole, on a real wire
 *
 * The client advances a per-room replay cursor and re-subscribes with
 * `since: cursor - 1`. That is correct and it is the problem: after the cursor
 * moves past a `9001`, the relay will never serve it again. Nothing wrote it to
 * disk, so the next launch folded a stream with an admission and an attestation
 * in it and no revocation anywhere, and admitted a key that had been burned.
 *
 * The three earlier fixes were each right about the boundary they saw:
 * revocation is not keyed by a grant, not derived from mutable member rows, and
 * not dependent on arrival order. This is the same claim under a fourth
 * pressure: **a revocation a restart forgets is not a revocation.**
 *
 * ## What is real here
 *
 * - The relay is `startTestRelay` from `nostr-effect`: a real in-process relay,
 *   real WebSocket framing, real storage, real `#h` matching, real `since`
 *   filtering. Not `MockRelayAdapter`, which the issue forbids.
 * - The database is real `node:sqlite` on a real file in a temp directory, and
 *   the restart is a real `close()` followed by a real re-open of that file.
 * - The relay-cursor store is deliberately shared between the two clients,
 *   because that is what the on-device secure store does across a restart. It
 *   is the reason the relay declines to re-serve the old records, which is the
 *   whole point.
 *
 * Neither of those is a phone. The omega#49 exits that name a device still need
 * a device.
 */
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { openNodeSqliteDatabase } from "@openagentsinc/sqlite-runtime";
import { LocalKeySigner } from "nostr-effect/identity";
import { finalizeEvent, generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { startTestRelay } from "nostr-effect/relay/node";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import {
  NIP_29_GROUP_CHAT_KIND,
  NIP_29_PUT_USER_KIND,
  NIP_29_REMOVE_USER_KIND,
  NIP_AP_PERSONA_KIND,
  attachOwnerAttestation,
} from "@openagentsinc/sarah/community";
import { SARAH_TURN_RECORD_KIND } from "@openagentsinc/sarah/nostr-turn";
import { ISSUE31_PRIVATE_GIFT_WRAP_KIND } from "@openagentsinc/sarah/issue31-nostr";
import type { Issue31SignedNostrEvent } from "@openagentsinc/sarah/issue31-nostr";

import {
  createIssue31NostrClient,
  type Issue31NostrClientSnapshot,
  type Issue31RelayCursor,
  type Issue31RelayCursorStore,
  type Issue31WebSocketLike,
} from "../src/workroom/issue31-nostr-client.ts";
import {
  ISSUE31_COMMUNITY_DATABASE_NAME,
  ISSUE31_OWNER_PRIVATE_DATABASE_NAME,
  Issue31CommunityRecordStoreError,
  createIssue31CommunityRecordStore,
  issue31CommunityConfirmedEventsFrom,
  issue31CommunityEvictionPlan,
  issue31MergeCommunityHistory,
  type Issue31CommunityDatabase,
} from "../src/workroom/issue31-community-record-store.ts";
import { createIssue31LocalConfirmedRecordStore } from "../src/workroom/issue31-outbound-event-store.ts";
import { projectIssue31CommunityReadModel } from "../src/workroom/issue31-community-read-model.ts";

const GROUP = "oa.community.persistence.v1";

const NodeSocket = class implements Issue31WebSocketLike {
  onopen: ((event: unknown) => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onclose: ((event: { code?: number; reason?: string }) => void) | null = null;
  readonly #socket: WebSocket;
  constructor(url: string) {
    this.#socket = new WebSocket(url);
    this.#socket.onopen = (event) => this.onopen?.(event);
    this.#socket.onmessage = (event: MessageEvent) => this.onmessage?.({ data: event.data });
    this.#socket.onerror = (event) => this.onerror?.(event);
    this.#socket.onclose = (event: CloseEvent) =>
      this.onclose?.({ code: event.code, reason: event.reason });
  }
  send(data: string): void {
    this.#socket.send(data);
  }
  close(code?: number, reason?: string): void {
    this.#socket.close(code, reason);
  }
} as unknown as new (url: string) => Issue31WebSocketLike;

/**
 * One cursor store for both launches, because the device has one.
 *
 * If each client got a fresh cursor the relay would happily replay everything
 * and the restart would prove nothing.
 */
const makeSharedCursorStore = (): Issue31RelayCursorStore => {
  const rows = new Map<string, Issue31RelayCursor>();
  const key = (relayUrl: string, room: string) => `${relayUrl}::${room}`;
  return {
    load: async (relayUrl, room) => rows.get(key(relayUrl, room)) ?? null,
    save: async (relayUrl, room, cursor) => {
      rows.set(key(relayUrl, room), cursor);
    },
  };
};

/** The expo-sqlite-shaped surface, over a real `node:sqlite` file. */
const openRealDatabase = (path: string): Issue31CommunityDatabase => {
  const database = openNodeSqliteDatabase(path);
  return {
    execSync: (sql) => database.exec(sql),
    runSync: (sql, ...params) => database.run(sql, [...params]),
    getAllSync: <Row,>(sql: string, ...params: ReadonlyArray<string | number>) =>
      database.all<Row>(sql, [...params]),
    closeSync: () => database.close(),
  };
};

const waitFor = async (
  predicate: () => boolean,
  label: string,
  timeoutMs = 15_000,
): Promise<void> => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`timed out waiting for ${label}`);
};

const party = () => {
  const secretKey = generateSecretKey();
  return {
    secretKey,
    secretKeyHex: [...secretKey].map((b) => b.toString(16).padStart(2, "0")).join(""),
    pubkey: getPublicKey(secretKey),
  };
};

const ADMIN = party();

let relay: Awaited<ReturnType<typeof startTestRelay>>;
let relayUrl: string;
let directory: string;

beforeAll(async () => {
  relay = await startTestRelay(41_000 + Math.floor(Math.random() * 3_000));
  relayUrl = `ws://127.0.0.1:${relay.port}`;
  directory = mkdtempSync(join(tmpdir(), "issue31-community-"));
});

afterAll(async () => {
  await Promise.resolve(relay?.stop());
  rmSync(directory, { recursive: true, force: true });
});

const sign = (
  secretKey: Uint8Array,
  input: Readonly<{
    kind: number;
    created_at: number;
    tags: ReadonlyArray<ReadonlyArray<string>>;
    content?: string;
  }>,
): Issue31SignedNostrEvent =>
  finalizeEvent(
    {
      kind: input.kind,
      created_at: input.created_at,
      tags: input.tags.map((tag) => [...tag]),
      content: input.content ?? "",
    },
    secretKey,
  ) as unknown as Issue31SignedNostrEvent;

/** Publish a signed event to the real relay and wait for its OK. */
const publish = async (event: Issue31SignedNostrEvent): Promise<string> => {
  const socket = new WebSocket(relayUrl);
  await new Promise<void>((resolve, reject) => {
    socket.onopen = () => resolve();
    socket.onerror = () => reject(new Error("publisher socket failed"));
  });
  const accepted = new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`no OK for kind ${event.kind}`)), 10_000);
    socket.onmessage = (message: MessageEvent) => {
      const frame = JSON.parse(String(message.data)) as ReadonlyArray<unknown>;
      if (frame[0] === "OK" && frame[1] === event.id) {
        clearTimeout(timer);
        if (frame[2] === true) resolve();
        else reject(new Error(`relay refused: ${String(frame[3])}`));
      }
    };
  });
  socket.send(JSON.stringify(["EVENT", event]));
  await accepted;
  socket.close(1000, "done");
  return event.id;
};

const openClient = (
  cursorStore: Issue31RelayCursorStore,
  onSnapshot: (snapshot: Issue31NostrClientSnapshot) => void,
  groupId: string,
) =>
  createIssue31NostrClient({
    relayUrls: [relayUrl],
    signer: LocalKeySigner.fromPrivateKey(generateSecretKey()),
    webSocket: NodeSocket,
    admittedHostPublicKeys: [],
    communityGroupIds: [groupId],
    cursorStore,
    onSnapshot,
  });

const configFor = (groupId: string, viewerPubkey: string) => ({
  groupId,
  adminPubkeys: [ADMIN.pubkey],
  scorerPubkeys: [] as ReadonlyArray<string>,
  ownerAppealPubkey: null,
  viewerPubkey,
  nowUnixSeconds: Math.floor(Date.now() / 1_000),
});

describe("a revocation survives a restart the relay will not replay", () => {
  test(
    "re-invitation after a restart cannot launder a burned agent key",
    async () => {
      const group = `${GROUP}.restart`;
      const operator = party();
      const agent = party();
      const cursorStore = makeSharedCursorStore();
      const databasePath = join(directory, "community-restart.db");
      const base = Math.floor(Date.now() / 1_000) - 3_600;

      const attestation = attachOwnerAttestation({
        agentPubkey: agent.pubkey,
        operatorSeckeyHex: operator.secretKeyHex,
      });

      const admitted = sign(ADMIN.secretKey, {
        kind: NIP_29_PUT_USER_KIND,
        created_at: base,
        tags: [["h", group], ["p", operator.pubkey]],
      });
      const persona = sign(agent.secretKey, {
        kind: NIP_AP_PERSONA_KIND,
        created_at: base + 1,
        tags: [["d", "worker"], ["h", group], [...attestation]],
      });
      const revocation = sign(ADMIN.secretKey, {
        kind: NIP_29_REMOVE_USER_KIND,
        created_at: base + 2,
        tags: [["h", group], ["p", operator.pubkey]],
      });
      // Ordinary room traffic after the revocation. This is what makes the hole
      // reachable rather than theoretical: the cursor advances past the `9001`,
      // so the next launch's `since` filter excludes it. A room that fell silent
      // the instant somebody was removed would accidentally re-serve the
      // revocation as its own newest record and survive by luck.
      const laterChatter = sign(ADMIN.secretKey, {
        kind: NIP_29_GROUP_CHAT_KIND,
        created_at: base + 600,
        tags: [["h", group]],
        content: "room carries on after the removal",
      });

      for (const event of [admitted, persona, revocation, laterChatter]) {
        await publish(event);
      }

      // ---- first launch -------------------------------------------------
      let firstSnapshot: Issue31NostrClientSnapshot | null = null;
      const firstClient = openClient(
        cursorStore,
        (next) => {
          firstSnapshot = next;
        },
        group,
      );
      const firstStore = createIssue31CommunityRecordStore({
        database: openRealDatabase(databasePath),
        groupId: group,
      });
      try {
        await firstClient.start();
        await waitFor(() => {
          const ids = new Set(
            (((): Issue31NostrClientSnapshot | null => firstSnapshot)()?.confirmedEvents ?? []).map(
              (row) => row.event.id,
            ),
          );
          return [admitted, persona, revocation, laterChatter].every((event) => ids.has(event.id));
        }, "the first launch to receive the four records");

        // Exactly what the runtime does on every snapshot.
        for (const row of (
          ((): Issue31NostrClientSnapshot | null => firstSnapshot)()?.confirmedEvents ?? []
        ).filter((row) => row.room === "community")) {
          firstStore.put(row.event);
        }

        const live = projectIssue31CommunityReadModel(
          ((): Issue31NostrClientSnapshot => firstSnapshot!)(),
          configFor(group, operator.pubkey),
        );
        expect(live.viewerRoleStatus).toBe("revoked");
        expect(live.agents.some((row) => row.agentPubkey === agent.pubkey && row.burned)).toBe(true);
      } finally {
        firstClient.close();
        firstStore.close();
      }

      // ---- restart ------------------------------------------------------
      const secondStore = createIssue31CommunityRecordStore({
        database: openRealDatabase(databasePath),
        groupId: group,
      });
      const restored = secondStore.load();
      expect(restored.map((event) => event.id).sort()).toEqual(
        [admitted, persona, revocation, laterChatter].map((event) => event.id).sort(),
      );

      // The re-invitation and the agent re-publishing its persona, after the
      // restart. Both are legitimate records from legitimate keys.
      const now = Math.floor(Date.now() / 1_000);
      const readmitted = sign(ADMIN.secretKey, {
        kind: NIP_29_PUT_USER_KIND,
        created_at: now,
        tags: [["h", group], ["p", operator.pubkey]],
      });
      const personaAgain = sign(agent.secretKey, {
        kind: NIP_AP_PERSONA_KIND,
        created_at: now + 1,
        tags: [["d", "worker"], ["h", group], [...attestation]],
      });
      await publish(readmitted);
      await publish(personaAgain);

      let secondSnapshot: Issue31NostrClientSnapshot | null = null;
      const secondClient = openClient(
        cursorStore,
        (next) => {
          secondSnapshot = next;
        },
        group,
      );
      try {
        await secondClient.start();
        await waitFor(() => {
          const ids = new Set(
            (
              ((): Issue31NostrClientSnapshot | null => secondSnapshot)()?.confirmedEvents ?? []
            ).map((row) => row.event.id),
          );
          return ids.has(readmitted.id) && ids.has(personaAgain.id);
        }, "the second launch to receive the re-invitation");

        const fresh = ((): Issue31NostrClientSnapshot => secondSnapshot!)();
        const freshIds = new Set(fresh.confirmedEvents.map((row) => row.event.id));
        // The relay was asked and declined: the cursor is past the revocation,
        // so it is not on this wire. Nothing but the store can supply it.
        expect(freshIds.has(revocation.id)).toBe(false);
        expect(freshIds.has(admitted.id)).toBe(false);
        expect(freshIds.has(persona.id)).toBe(false);

        // Falsification, in the same test rather than in a comment: this is the
        // exact projection the app produced before this change, and it admits
        // the burned key.
        const withoutHistory = projectIssue31CommunityReadModel(
          fresh,
          configFor(group, operator.pubkey),
        );
        expect(withoutHistory.viewerRole).toBe("agent_operator");
        expect(
          withoutHistory.agents.some((row) => row.agentPubkey === agent.pubkey && !row.burned),
        ).toBe(true);

        // With durable history merged in, the burn is still there.
        const merged = projectIssue31CommunityReadModel(
          issue31MergeCommunityHistory(fresh, issue31CommunityConfirmedEventsFrom(restored)),
          configFor(group, operator.pubkey),
        );
        // Re-admission gives the operator a fresh member row, so there is no
        // agent row at all rather than a burned one: the re-published
        // attestation never attached. The burn set is what refused it.
        expect(merged.agents.map((row) => row.agentPubkey)).not.toContain(agent.pubkey);
        expect(
          merged.refusals.some(
            (refusal) => refusal.code === "agent_key_burned" && refusal.eventId === personaAgain.id,
          ),
        ).toBe(true);
        // The operator's own re-admission is honoured — revocation binds the
        // agent key, not the person — but it buys back no agent.
        expect(merged.viewerRole).toBe("member");
        expect(merged.viewerRoleStatus).toBe("active");
      } finally {
        secondClient.close();
        secondStore.close();
      }
    },
    60_000,
  );

  test(
    "the restored transcript is the room's own history, not another group's",
    async () => {
      const group = `${GROUP}.scoped`;
      const otherGroup = `${GROUP}.scoped.other`;
      const operator = party();
      const databasePath = join(directory, "community-scoped.db");
      const at = Math.floor(Date.now() / 1_000) - 900;

      const mine = sign(operator.secretKey, {
        kind: NIP_29_GROUP_CHAT_KIND,
        created_at: at,
        tags: [["h", group]],
        content: "belongs to this room",
      });
      const theirs = sign(operator.secretKey, {
        kind: NIP_29_GROUP_CHAT_KIND,
        created_at: at + 1,
        tags: [["h", otherGroup]],
        content: "belongs to another room",
      });

      const store = createIssue31CommunityRecordStore({
        database: openRealDatabase(databasePath),
        groupId: group,
      });
      try {
        store.put(mine);
        expect(() => store.put(theirs)).toThrow(Issue31CommunityRecordStoreError);
        expect(store.load().map((event) => event.id)).toEqual([mine.id]);
      } finally {
        store.close();
      }
    },
    30_000,
  );
});

describe("the two rooms' stores share nothing", () => {
  test("the community store lives in a different database file", () => {
    expect(ISSUE31_COMMUNITY_DATABASE_NAME).not.toBe(ISSUE31_OWNER_PRIVATE_DATABASE_NAME);
  });

  test("an owner-private kind cannot be written to the community store", () => {
    const operator = party();
    const store = createIssue31CommunityRecordStore({
      database: openRealDatabase(join(directory, "community-kinds.db")),
      groupId: GROUP,
    });
    try {
      for (const kind of [ISSUE31_PRIVATE_GIFT_WRAP_KIND, SARAH_TURN_RECORD_KIND]) {
        const event = sign(operator.secretKey, {
          kind,
          created_at: Math.floor(Date.now() / 1_000),
          tags: [["h", GROUP], ["p", operator.pubkey]],
          content: "",
        });
        expect(() => store.put(event)).toThrow(/not a community kind/);
      }
      expect(store.load()).toEqual([]);
    } finally {
      store.close();
    }
  });

  test("a community room that fills its outbound queue does not refuse an owner-private publish", () => {
    // The other half of "no shared optimistic state". No relay claim is made
    // here: this is the client's own queue, exercised offline on purpose, which
    // is exactly the situation where a shared bound would bite.
    const operator = party();
    const client = createIssue31NostrClient({
      relayUrls: [relayUrl],
      signer: LocalKeySigner.fromPrivateKey(generateSecretKey()),
      webSocket: NodeSocket,
      admittedHostPublicKeys: [],
      communityGroupIds: [GROUP],
      cursorStore: makeSharedCursorStore(),
      maxQueuedEvents: 1,
    });
    try {
      const at = Math.floor(Date.now() / 1_000);
      const communityFirst = sign(operator.secretKey, {
        kind: NIP_29_GROUP_CHAT_KIND,
        created_at: at,
        tags: [["h", GROUP]],
        content: "fills the community budget",
      });
      const communitySecond = sign(operator.secretKey, {
        kind: NIP_29_GROUP_CHAT_KIND,
        created_at: at + 1,
        tags: [["h", GROUP]],
        content: "overflows it",
      });
      const privateEvent = sign(operator.secretKey, {
        kind: ISSUE31_PRIVATE_GIFT_WRAP_KIND,
        created_at: at + 2,
        tags: [["p", operator.pubkey]],
        content: "the owner's own message",
      });

      client.publish(communityFirst, "community");
      expect(() => client.publish(communitySecond, "community")).toThrow(/queue is full/);
      // The owner's room still has its whole budget.
      expect(client.publish(privateEvent, "owner_private").transportState).toBe("queued");
    } finally {
      client.close();
    }
  });

  test("filling the community store to its bound does not refuse an owner-private write", () => {
    const operator = party();
    const communityPath = join(directory, "isolation-community.db");
    const privatePath = join(directory, "isolation-private.db");
    const community = createIssue31CommunityRecordStore({
      database: openRealDatabase(communityPath),
      groupId: GROUP,
      maximumRecords: 8,
    });
    const ownerPrivate = createIssue31LocalConfirmedRecordStore(
      openRealDatabase(privatePath) as never,
      64,
    );
    try {
      const at = Math.floor(Date.now() / 1_000);
      for (let index = 0; index < 24; index += 1) {
        community.put(
          sign(operator.secretKey, {
            kind: NIP_29_GROUP_CHAT_KIND,
            created_at: at + index,
            tags: [["h", GROUP]],
            content: `message ${index}`,
          }),
        );
      }
      // The community room churned three times its whole budget. The private
      // room's history is untouched and its own budget is unspent, because they
      // are different tables in different files.
      expect(community.load()).toHaveLength(8);
      expect(ownerPrivate.load()).toEqual([]);
    } finally {
      community.close();
      ownerPrivate.close();
    }
  });
});

describe("a bound must not launder a revocation", () => {
  test("eviction discards ordinary records and never a removal", () => {
    const plan = issue31CommunityEvictionPlan({
      rows: [
        { sequence: 1, kind: NIP_29_REMOVE_USER_KIND },
        { sequence: 2, kind: NIP_29_GROUP_CHAT_KIND },
        { sequence: 3, kind: NIP_29_GROUP_CHAT_KIND },
      ],
      maximumRecords: 3,
    });
    expect(plan).toEqual([2]);
  });

  test("a store with nothing but revocations refuses the write instead of forgetting one", () => {
    expect(() =>
      issue31CommunityEvictionPlan({
        rows: [
          { sequence: 1, kind: NIP_29_REMOVE_USER_KIND },
          { sequence: 2, kind: NIP_29_REMOVE_USER_KIND },
        ],
        maximumRecords: 2,
      }),
    ).toThrow(/full of revocations/);
  });

  test("the bound evicts chat and keeps the removal, on a real database", () => {
    const operator = party();
    const agent = party();
    const store = createIssue31CommunityRecordStore({
      database: openRealDatabase(join(directory, "community-bound.db")),
      groupId: GROUP,
      maximumRecords: 8,
    });
    try {
      const at = Math.floor(Date.now() / 1_000);
      const revocation = sign(ADMIN.secretKey, {
        kind: NIP_29_REMOVE_USER_KIND,
        created_at: at,
        tags: [["h", GROUP], ["p", agent.pubkey]],
      });
      store.put(revocation);
      for (let index = 0; index < 32; index += 1) {
        store.put(
          sign(operator.secretKey, {
            kind: NIP_29_GROUP_CHAT_KIND,
            created_at: at + 1 + index,
            tags: [["h", GROUP]],
            content: `filler ${index}`,
          }),
        );
      }
      const kept = store.load();
      expect(kept).toHaveLength(8);
      // Thirty-two messages could not push the revocation out. A naive
      // drop-the-oldest rule would have discarded it on the first write.
      expect(kept.some((event) => event.id === revocation.id)).toBe(true);
    } finally {
      store.close();
    }
  });
});

describe("stored records are re-checked, not trusted", () => {
  test("a tampered row is refused on the way out", () => {
    const operator = party();
    const path = join(directory, "community-tamper.db");
    const database = openRealDatabase(path);
    const store = createIssue31CommunityRecordStore({ database, groupId: GROUP });
    try {
      const event = sign(operator.secretKey, {
        kind: NIP_29_GROUP_CHAT_KIND,
        created_at: Math.floor(Date.now() / 1_000),
        tags: [["h", GROUP]],
        content: "the original",
      });
      store.put(event);
      // Somebody edits the database file directly. The signature no longer
      // covers the content, and local storage is not an authority either.
      database.runSync(
        "UPDATE issue31_community_records SET event_json = ? WHERE event_id = ?",
        JSON.stringify({ ...event, content: "forged" }),
        event.id,
      );
      expect(() => store.load()).toThrow(/invalid signature/);
    } finally {
      store.close();
    }
  });

  test("merging keeps the live copy of a record that is in both places", () => {
    const operator = party();
    const event = sign(operator.secretKey, {
      kind: NIP_29_GROUP_CHAT_KIND,
      created_at: Math.floor(Date.now() / 1_000),
      tags: [["h", GROUP]],
      content: "seen twice",
    });
    const live = {
      confirmedEvents: issue31CommunityConfirmedEventsFrom([event]).map((row) => ({
        ...row,
        relayUrl: "ws://relay.example",
      })),
    };
    const merged = issue31MergeCommunityHistory(
      live,
      issue31CommunityConfirmedEventsFrom([event]),
    );
    expect(merged.confirmedEvents).toHaveLength(1);
    expect(merged.confirmedEvents[0]?.relayUrl).toBe("ws://relay.example");
  });
});
