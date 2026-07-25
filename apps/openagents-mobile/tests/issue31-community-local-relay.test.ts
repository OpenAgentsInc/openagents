/**
 * The community room against a REAL local relay (omega#48).
 *
 * The room's whole failure was that it never subscribed: `filtersFor` returned
 * `[]` because no `communityGroupIds` was ever passed, so no community `REQ`
 * was sent and no community record could arrive. Proving that is fixed needs an
 * actual wire — a scripted socket with frames injected by direct method call
 * would have "passed" against the broken client too, because the frames would
 * have been handed to it rather than requested by it.
 *
 * `startTestRelay` from `nostr-effect` is a real relay speaking the real
 * protocol in-process: real WebSocket framing, real storage, real `#h` tag
 * matching, real EOSE. It is NOT `MockRelayAdapter` and it is NOT a physical
 * device — the omega#49 exits that name a phone still need a phone.
 */
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

import {
  createIssue31NostrClient,
  type Issue31NostrClientSnapshot,
  type Issue31RelayCursor,
  type Issue31RelayCursorStore,
  type Issue31WebSocketLike,
} from "../src/workroom/issue31-nostr-client.ts";
import { projectIssue31CommunityReadModel } from "../src/workroom/issue31-community-read-model.ts";

const GROUP = "oa.community.local.v1";

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

const makeMemoryCursorStore = (): Issue31RelayCursorStore => {
  const rows = new Map<string, Issue31RelayCursor>();
  const key = (relayUrl: string, room: string) => `${relayUrl}::${room}`;
  return {
    load: async (relayUrl, room) => rows.get(key(relayUrl, room)) ?? null,
    save: async (relayUrl, room, cursor) => {
      rows.set(key(relayUrl, room), cursor);
    },
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

let relay: Awaited<ReturnType<typeof startTestRelay>>;
let relayUrl: string;

const party = () => {
  const secretKey = generateSecretKey();
  return {
    secretKey,
    secretKeyHex: [...secretKey].map((b) => b.toString(16).padStart(2, "0")).join(""),
    pubkey: getPublicKey(secretKey),
  };
};

const ADMIN = party();

beforeAll(async () => {
  relay = await startTestRelay(36_000 + Math.floor(Math.random() * 4_000));
  relayUrl = `ws://127.0.0.1:${relay.port}`;
});

afterAll(async () => {
  await Promise.resolve(relay?.stop());
});

/** Publish a signed event straight to the relay and wait for its OK. */
const publish = async (
  secretKey: Uint8Array,
  input: Readonly<{
    kind: number;
    created_at: number;
    tags: ReadonlyArray<ReadonlyArray<string>>;
    content?: string;
  }>,
): Promise<string> => {
  const event = finalizeEvent(
    {
      kind: input.kind,
      created_at: input.created_at,
      tags: input.tags.map((tag) => [...tag]),
      content: input.content ?? "",
    },
    secretKey,
  );
  const socket = new WebSocket(relayUrl);
  await new Promise<void>((resolve, reject) => {
    socket.onopen = () => resolve();
    socket.onerror = () => reject(new Error("publisher socket failed"));
  });
  const accepted = new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`no OK for kind ${input.kind}`)), 10_000);
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
  onSnapshot: (snapshot: Issue31NostrClientSnapshot) => void,
  communityGroupIds: ReadonlyArray<string> = [GROUP],
) =>
  createIssue31NostrClient({
    relayUrls: [relayUrl],
    signer: LocalKeySigner.fromPrivateKey(generateSecretKey()),
    webSocket: NodeSocket,
    admittedHostPublicKeys: [],
    communityGroupIds,
    cursorStore: makeMemoryCursorStore(),
    onSnapshot,
  });

describe("the community room over a real wire", () => {
  test("a group-scoped subscription actually retrieves community records", async () => {
    const operator = party();
    const agent = party();
    const at = Math.floor(Date.now() / 1_000) - 60;

    const putUserId = await publish(ADMIN.secretKey, {
      kind: NIP_29_PUT_USER_KIND,
      created_at: at,
      tags: [
        ["h", GROUP],
        ["p", operator.pubkey],
      ],
    });
    const authTag = attachOwnerAttestation({
      agentPubkey: agent.pubkey,
      operatorSeckeyHex: operator.secretKeyHex,
    });
    const personaId = await publish(agent.secretKey, {
      kind: NIP_AP_PERSONA_KIND,
      created_at: at + 1,
      tags: [["d", "worker"], ["h", GROUP], [...authTag]],
    });
    const chatId = await publish(operator.secretKey, {
      kind: NIP_29_GROUP_CHAT_KIND,
      created_at: at + 2,
      tags: [["h", GROUP]],
      content: "hello from the community room",
    });

    let snapshot: Issue31NostrClientSnapshot | null = null;
    const client = openClient((next) => {
      snapshot = next;
    });
    try {
      await client.start();
      await waitFor(() => {
        const current = ((): Issue31NostrClientSnapshot | null => snapshot)();
        const ids = new Set((current?.confirmedEvents ?? []).map((row) => row.event.id));
        return ids.has(putUserId) && ids.has(personaId) && ids.has(chatId);
      }, "the community records to arrive over the wire");

      const current = ((): Issue31NostrClientSnapshot | null => snapshot)();
      // Every one of these was requested by the client's own `#h` filter and
      // returned by the relay. Nothing was handed to it.
      const model = projectIssue31CommunityReadModel(current!, {
        groupId: GROUP,
        adminPubkeys: [ADMIN.pubkey],
        scorerPubkeys: [],
        ownerAppealPubkey: null,
        viewerPubkey: operator.pubkey,
        nowUnixSeconds: Math.floor(Date.now() / 1_000),
      });

      expect(model.status).toBe("ready");
      expect(model.roster.map((row) => row.operatorPubkey)).toContain(operator.pubkey);
      expect(model.agents.map((row) => row.agentPubkey)).toContain(agent.pubkey);
      expect(model.transcript.map((row) => row.displayText)).toContain(
        "hello from the community room",
      );
      // The role came off the wire, not from a default.
      expect(model.viewerRole).toBe("agent_operator");
      expect(model.controls.map((row) => row.kind)).toContain("post_message");
    } finally {
      client.close();
    }
  }, 30_000);

  test("without a configured group the client sends no community REQ and sees nothing", async () => {
    const operator = party();
    const at = Math.floor(Date.now() / 1_000) - 30;
    const chatId = await publish(operator.secretKey, {
      kind: NIP_29_GROUP_CHAT_KIND,
      created_at: at,
      tags: [["h", GROUP]],
      content: "this must not arrive",
    });

    let snapshot: Issue31NostrClientSnapshot | null = null;
    // This is the shipped app's actual behaviour before this change: no group
    // ids, so `filtersFor` returns `[]` for the community room.
    const client = openClient((next) => {
      snapshot = next;
    }, []);
    try {
      await client.start();
      await waitFor(
        () =>
          (((): Issue31NostrClientSnapshot | null => snapshot)()?.relays ?? []).some(
            (row) => row.state === "live",
          ),
        "the relay to reach live",
      );
      const current = ((): Issue31NostrClientSnapshot | null => snapshot)();
      expect((current?.confirmedEvents ?? []).some((row) => row.event.id === chatId)).toBe(false);
    } finally {
      client.close();
    }
  }, 30_000);

  test("revocation published to the relay removes access on the next projection", async () => {
    const operator = party();
    const agent = party();
    const group = `${GROUP}.revocation`;
    const at = Math.floor(Date.now() / 1_000) - 60;

    await publish(ADMIN.secretKey, {
      kind: NIP_29_PUT_USER_KIND,
      created_at: at,
      tags: [["h", group], ["p", operator.pubkey]],
    });
    const authTag = attachOwnerAttestation({
      agentPubkey: agent.pubkey,
      operatorSeckeyHex: operator.secretKeyHex,
    });
    const personaId = await publish(agent.secretKey, {
      kind: NIP_AP_PERSONA_KIND,
      created_at: at + 1,
      tags: [["d", "worker"], ["h", group], [...authTag]],
    });
    const revokeId = await publish(ADMIN.secretKey, {
      kind: NIP_29_REMOVE_USER_KIND,
      created_at: at + 2,
      tags: [["h", group], ["p", agent.pubkey]],
    });

    let snapshot: Issue31NostrClientSnapshot | null = null;
    const client = openClient((next) => {
      snapshot = next;
    }, [group]);
    try {
      await client.start();
      await waitFor(() => {
        const current = ((): Issue31NostrClientSnapshot | null => snapshot)();
        const ids = new Set((current?.confirmedEvents ?? []).map((row) => row.event.id));
        return ids.has(personaId) && ids.has(revokeId);
      }, "the attestation and its revocation to arrive");

      const current = ((): Issue31NostrClientSnapshot | null => snapshot)();
      const model = projectIssue31CommunityReadModel(current!, {
        groupId: group,
        adminPubkeys: [ADMIN.pubkey],
        scorerPubkeys: [],
        ownerAppealPubkey: null,
        viewerPubkey: operator.pubkey,
        nowUnixSeconds: Math.floor(Date.now() / 1_000),
      });

      // The relay stored and returned the attestation; the client refuses to
      // admit it because a revocation burned the key. The relay is transport,
      // not admission authority.
      expect(model.agents.some((row) => row.agentPubkey === agent.pubkey && row.burned)).toBe(true);
      expect(model.viewerRole).toBe("member");
    } finally {
      client.close();
    }
  }, 30_000);

  test("the two rooms keep separate replay cursors on the same relay", async () => {
    let snapshot: Issue31NostrClientSnapshot | null = null;
    const client = openClient((next) => {
      snapshot = next;
    });
    try {
      await client.start();
      await waitFor(
        () =>
          (((): Issue31NostrClientSnapshot | null => snapshot)()?.relays ?? []).some(
            (row) => row.state === "live",
          ),
        "the relay to reach live",
      );
      const current = ((): Issue31NostrClientSnapshot | null => snapshot)();
      const row = (current?.relays ?? [])[0];
      // Each room reports its own replay point. `replaySince` is the `min`
      // across rooms and must not be read as any one room's freshness.
      expect(row?.roomReplaySince).toBeDefined();
      expect(Object.keys(row?.roomReplaySince ?? {}).sort()).toEqual([
        "community",
        "discovery",
        "owner_private",
      ]);
    } finally {
      client.close();
    }
  }, 30_000);
});
