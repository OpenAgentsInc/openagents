/**
 * The mobile Issue 31 client against a REAL local relay (omega#46, omega#48).
 *
 * Epic omega#31 is explicit that the deployed relay is not on the critical
 * path for client work — the local relay is. `nostr-effect` ships
 * `startTestRelay`, and until now nothing in this repository used it: every
 * mobile client test drives `ScriptedWebSocket`, which injects frames by
 * direct method call, and the one live-relay test is skipped by default AND
 * bypasses the client entirely by hand-writing frames onto a raw socket.
 *
 * So the client's behaviour on an actual wire — real WebSocket framing, real
 * relay storage, real filter matching, real EOSE, real replay from a cursor —
 * was unproven at every fidelity. This closes that.
 *
 * This is a real relay speaking the real protocol in-process. It is NOT a
 * mock, and it is NOT a physical-device proof: the omega#46/#49 exits that
 * name a phone still need a phone.
 */
import { LocalKeySigner } from "nostr-effect/identity";
import { generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { startTestRelay } from "nostr-effect/relay/node";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import { ISSUE31_HOST_ANNOUNCEMENT_KIND } from "@openagentsinc/sarah/issue31-nostr";

import {
  createIssue31NostrClient,
  type Issue31NostrClientSnapshot,
  type Issue31RelayCursor,
  type Issue31RelayCursorStore,
  type Issue31WebSocketLike,
} from "../src/workroom/issue31-nostr-client.ts";

/**
 * Node 24's global `WebSocket` is the same runtime shape the client's factory
 * produces on device. It is not structurally assignable to
 * `Issue31WebSocketLike` — that interface types handler arguments as `unknown`
 * on purpose, to stop the client reaching into DOM-specific event fields — so
 * this adapts rather than widening the client's contract to make it compile.
 */
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

/** A cursor store that survives a client restart, like the device keystore. */
const makeMemoryCursorStore = (): Issue31RelayCursorStore & {
  readonly dump: () => ReadonlyArray<string>;
} => {
  const rows = new Map<string, Issue31RelayCursor>();
  const key = (relayUrl: string, room: string) => `${relayUrl}::${room}`;
  return {
    load: async (relayUrl, room) => rows.get(key(relayUrl, room)) ?? null,
    save: async (relayUrl, room, cursor) => {
      rows.set(key(relayUrl, room), cursor);
    },
    dump: () => [...rows.keys()],
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

/** The Sarah key the host announcement binds; identity is out of scope here. */
const sarahPublicKey = getPublicKey(generateSecretKey());

beforeAll(async () => {
  relay = await startTestRelay(32_000 + Math.floor(Math.random() * 4_000));
  relayUrl = `ws://127.0.0.1:${relay.port}`;
});

afterAll(async () => {
  await Promise.resolve(relay?.stop());
});

/**
 * Publish one real, schema-valid v2 host announcement straight to the relay.
 *
 * The content has to be a genuine `openagents.omega.issue31.host_discovery.v2`
 * record bound to the signing key: the client decodes and validates every
 * discovery event, so a placeholder body is dropped exactly as an attacker's
 * would be. That is the behaviour under test, not an obstacle to it.
 */
const publishHostAnnouncement = async (
  hostSigner: LocalKeySigner,
  hostPublicKey: string,
  sarahPublicKey: string,
  createdAt: number,
  label: string,
): Promise<string> => {
  const announcement = {
    schema: "openagents.omega.issue31.host_discovery.v2",
    hostRef: "omega.host.local",
    hostPublicKeyHex: hostPublicKey,
    sarahPublicKeyHex: sarahPublicKey,
    displayName: label,
    conversation: "sarah.0123456789abcdef01234567",
    protocols: [
      "openagents.omega.issue31.pairing.v1",
      "openagents.omega.issue31.command.v1",
      "openagents.omega.issue31.command.v2",
    ],
    relayUrls: ["wss://relay.example.com"],
    generation: 5,
    issuedAt: createdAt,
    expiresAt: createdAt + 86_400,
  };
  const event = await hostSigner.signEvent({
    kind: ISSUE31_HOST_ANNOUNCEMENT_KIND,
    created_at: createdAt,
    tags: [
      ["t", "omega-issue31-host"],
      // The client requires `d` to equal `hostRef` and a `k` tag naming the
      // gift-wrap kind, so the announcement is self-describing on the wire.
      ["d", announcement.hostRef],
      ["k", "1059"],
    ],
    content: JSON.stringify(announcement),
  });
  const socket = new WebSocket(relayUrl);
  await new Promise<void>((resolve, reject) => {
    socket.onopen = () => resolve();
    socket.onerror = () => reject(new Error("publisher socket failed"));
  });
  const accepted = new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("no OK for announcement")), 10_000);
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

describe("mobile Issue 31 client on a real local relay", () => {
  test("connects, subscribes, and confirms a stored event over the wire", async () => {
    const hostSecret = generateSecretKey();
    const hostPublicKey = getPublicKey(hostSecret);
    const hostSigner = LocalKeySigner.fromPrivateKey(hostSecret);
    const createdAt = Math.floor(Date.now() / 1000);
    const eventId = await publishHostAnnouncement(hostSigner, hostPublicKey, sarahPublicKey, createdAt, "first");

    let snapshot: Issue31NostrClientSnapshot | null = null;
    const client = createIssue31NostrClient({
      relayUrls: [relayUrl],
      signer: LocalKeySigner.fromPrivateKey(generateSecretKey()),
      webSocket: NodeSocket,
      admittedHostPublicKeys: [hostPublicKey],
      cursorStore: makeMemoryCursorStore(),
      onSnapshot: (next) => {
        snapshot = next;
      },
    });

    try {
      await client.start();
      // The relay really stored the event and really matched the filter.
      await waitFor(
        () => (snapshot?.confirmedEvents ?? []).some((row) => row.event.id === eventId),
        "the announcement to arrive over the wire",
      );

      const confirmed = snapshot!.confirmedEvents.find((row) => row.event.id === eventId)!;
      expect(confirmed.event.pubkey).toBe(hostPublicKey);
      expect(confirmed.room).toBe("discovery");
      expect(confirmed.event.kind).toBe(ISSUE31_HOST_ANNOUNCEMENT_KIND);

      // A real EOSE, from a real relay, ends the replay and opens the live lane.
      await waitFor(
        () => (snapshot?.relays ?? []).some((row) => row.state === "live"),
        "the relay to reach live after a real EOSE",
      );
      const relayRow = snapshot!.relays.find((row) => row.relayUrl === relayUrl)!;
      expect(relayRow.state).toBe("live");
      expect(relayRow.gapReason).toBeNull();
    } finally {
      client.close();
    }
  });

  test("a restarted client replays from its persisted cursor and sees later events", async () => {
    const hostSecret = generateSecretKey();
    const hostPublicKey = getPublicKey(hostSecret);
    const hostSigner = LocalKeySigner.fromPrivateKey(hostSecret);
    const cursorStore = makeMemoryCursorStore();
    const deviceSigner = LocalKeySigner.fromPrivateKey(generateSecretKey());
    const base = Math.floor(Date.now() / 1000);

    const firstId = await publishHostAnnouncement(hostSigner, hostPublicKey, sarahPublicKey, base, "before-restart");

    let first: Issue31NostrClientSnapshot | null = null;
    const clientA = createIssue31NostrClient({
      relayUrls: [relayUrl],
      signer: deviceSigner,
      webSocket: NodeSocket,
      admittedHostPublicKeys: [hostPublicKey],
      cursorStore,
      onSnapshot: (next) => {
        first = next;
      },
    });
    await clientA.start();
    await waitFor(
      () => (first?.confirmedEvents ?? []).some((row) => row.event.id === firstId),
      "the pre-restart announcement",
    );
    clientA.close();

    // The cursor is durable state, not client memory.
    expect(cursorStore.dump()).toContain(`${relayUrl}::discovery`);

    // A later event lands while no client is connected.
    const secondId = await publishHostAnnouncement(hostSigner, hostPublicKey, sarahPublicKey, base + 60, "after-restart");

    let second: Issue31NostrClientSnapshot | null = null;
    const clientB = createIssue31NostrClient({
      relayUrls: [relayUrl],
      signer: deviceSigner,
      webSocket: NodeSocket,
      admittedHostPublicKeys: [hostPublicKey],
      cursorStore,
      onSnapshot: (next) => {
        second = next;
      },
    });
    try {
      await clientB.start();
      // Replay from the persisted cursor picks up what was missed while down.
      await waitFor(
        () => (second?.confirmedEvents ?? []).some((row) => row.event.id === secondId),
        "the post-restart announcement after cursor replay",
      );
      await waitFor(
        () => (second?.relays ?? []).some((row) => row.state === "live"),
        "the restarted client to reach live",
      );
    } finally {
      clientB.close();
    }
  });

  test("an unadmitted host's announcement is not confirmed", async () => {
    // The relay stores it; the client's author filter must still refuse it.
    // A relay is not an authority — that is the whole point of the binding.
    const admittedSecret = generateSecretKey();
    const strangerSecret = generateSecretKey();
    const strangerPublicKey = getPublicKey(strangerSecret);
    const strangerSigner = LocalKeySigner.fromPrivateKey(strangerSecret);
    const createdAt = Math.floor(Date.now() / 1000);
    const strangerId = await publishHostAnnouncement(strangerSigner, strangerPublicKey, sarahPublicKey, createdAt, "stranger");

    // Declared through an assertion so the compiler does not narrow the
    // initialiser to `null` and then to `never`: the assignment happens in a
    // callback it cannot see.
    let snapshot = null as Issue31NostrClientSnapshot | null;
    const client = createIssue31NostrClient({
      relayUrls: [relayUrl],
      signer: LocalKeySigner.fromPrivateKey(generateSecretKey()),
      webSocket: NodeSocket,
      admittedHostPublicKeys: [getPublicKey(admittedSecret)],
      cursorStore: makeMemoryCursorStore(),
      onSnapshot: (next) => {
        snapshot = next;
      },
    });
    try {
      await client.start();
      await waitFor(
        () => (snapshot?.relays ?? []).some((row) => row.state === "live"),
        "the relay to reach live",
      );
      // Read through a closure. `snapshot` is only ever assigned from the
      // `onSnapshot` callback, so straight-line narrowing collapses it to
      // `never` here and the assertion stops typechecking.
      const settled = ((): Issue31NostrClientSnapshot | null => snapshot)();
      expect(
        (settled?.confirmedEvents ?? []).some((row) => row.event.id === strangerId),
      ).toBe(false);
    } finally {
      client.close();
    }
  });
});
