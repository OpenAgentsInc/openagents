/**
 * A backdated gift wrap must still reach the device (omega#49).
 *
 * Found on `ios-simulator (iPhone 17 Pro, iOS 26.5)` against the deployed
 * relay, where pairing stalled on `Pairing request signed and queued; waiting
 * for the host challenge.` while the host's durable state showed the challenge
 * issued *and* published. The challenge was on the relay. The device could not
 * see it.
 *
 * NIP-59 requires the wrap's `created_at` to be randomized into the past — that
 * is the point of it, so a relay operator cannot infer who is talking to whom
 * and when from timestamps. The relay filters on the wrap's time, not the
 * sealed rumor's. The client kept a strictly monotonic `since` high-water mark,
 * so any wrap whose randomized time landed below the newest already admitted
 * was filtered out by the relay and never delivered.
 *
 * Nothing reports it. The host publishes and gets an `OK`. The device's
 * subscription simply never mentions the event — no gap, no refusal, nothing to
 * quarantine. It is nondeterministic, so the same build pairs fine whenever the
 * roll lands above the cursor, which makes it read as a flaky phone rather than
 * a protocol error. And it degrades with uptime, because the cursor only rises.
 *
 * It applies to every owner-private record — challenge, grant, revocation,
 * owner projection, host adjuncts — because all of them travel as gift wraps.
 *
 * The relay here is a real in-process relay enforcing `since` exactly as a
 * deployed one does (`FilterMatcher`: `created_at < since` is not a match).
 */
import {
  ISSUE31_PAIRING_SCHEMA,
  createIssue31PrivateGiftWrap,
  decodeIssue31PairingRecord,
} from "@openagentsinc/sarah/issue31-nostr";
import { LocalKeySigner } from "nostr-effect/identity";
import { generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { startTestRelay } from "nostr-effect/relay/node";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import {
  createIssue31NostrClient,
  type Issue31NostrClientSnapshot,
  type Issue31RelayCursor,
  type Issue31RelayCursorStore,
  type Issue31WebSocketLike,
} from "../src/workroom/issue31-nostr-client.ts";

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

/** One cursor store shared across both sessions, so the restart is real. */
const sharedCursorStore = (): Issue31RelayCursorStore => {
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

beforeAll(async () => {
  relay = await startTestRelay(39_000 + Math.floor(Math.random() * 2_000));
  relayUrl = `ws://127.0.0.1:${relay.port}`;
});

afterAll(async () => {
  await Promise.resolve(relay?.stop());
});

const publish = async (event: { readonly id: string }): Promise<void> => {
  const socket = new WebSocket(relayUrl);
  await new Promise<void>((resolve, reject) => {
    socket.onopen = () => resolve();
    socket.onerror = () => reject(new Error("publisher socket failed"));
  });
  const accepted = new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`no OK for ${event.id}`)), 10_000);
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
};

describe("omega#49 a gift wrap backdated below the cursor still reaches the device", () => {
  test("a wrap randomized hours into the past is delivered after a restart", async () => {
    const hostSecret = generateSecretKey();
    const hostPublicKey = getPublicKey(hostSecret);
    const hostSigner = LocalKeySigner.fromPrivateKey(hostSecret);
    const deviceSecret = generateSecretKey();
    const devicePublicKey = getPublicKey(deviceSecret);
    const deviceSigner = LocalKeySigner.fromPrivateKey(deviceSecret);
    const sarahPublicKey = getPublicKey(generateSecretKey());
    const now = Math.floor(Date.now() / 1000);
    const hostRef = "omega.host.backdate";
    const identity = {
      hostRef,
      hostPublicKeyHex: hostPublicKey,
      devicePublicKeyHex: devicePublicKey,
    };

    const grantWrap = async (grantRef: string, wrapCreatedAt: number) =>
      createIssue31PrivateGiftWrap({
        signer: hostSigner,
        recipientPublicKeyHex: devicePublicKey,
        record: decodeIssue31PairingRecord({
          schema: ISSUE31_PAIRING_SCHEMA,
          recordType: "scoped_grant",
          ...identity,
          sarahPublicKeyHex: sarahPublicKey,
          issuedAt: now,
          pairingResponseEventId: "3".repeat(64),
          grantRef,
          generation: 1,
          scopes: ["observe_issue31"] as const,
          expiresAt: now + 86_400,
        }),
        randomSecretKey: generateSecretKey,
        createdAt: now,
        sealCreatedAt: now,
        wrapCreatedAt,
      });

    // The first wrap lands at `now` and pins the device's high-water mark.
    const leading = await grantWrap("grant.omega.leading", now);
    await publish(leading);

    const cursorStore = sharedCursorStore();
    const startClient = async () => {
      let latest: Issue31NostrClientSnapshot | null = null;
      const client = createIssue31NostrClient({
        relayUrls: [relayUrl],
        signer: deviceSigner,
        webSocket: NodeSocket,
        admittedHostPublicKeys: [hostPublicKey],
        selectedHostPublicKeys: [hostPublicKey],
        ownerAuthors: [sarahPublicKey],
        ownerRecipientPublicKeys: [devicePublicKey],
        cursorStore,
        onSnapshot: (next) => {
          latest = next;
        },
      });
      await client.start();
      return {
        client,
        has: (id: string) =>
          (latest?.confirmedEvents ?? []).some((row) => row.event.id === id),
      };
    };

    const first = await startClient();
    try {
      await waitFor(() => first.has(leading.id), "the leading wrap to arrive");
    } finally {
      first.client.close();
    }

    // A second wrap, published later, randomized six hours into the past — well
    // within what NIP-59 asks of a sender, and below the cursor the device now
    // holds. Before the fix the relay filtered this out and the device waited
    // forever with nothing to show for it.
    const backdated = await grantWrap("grant.omega.backdated", now - 6 * 60 * 60);
    expect(backdated.created_at).toBeLessThan(leading.created_at);
    await publish(backdated);

    const second = await startClient();
    try {
      await waitFor(
        () => second.has(backdated.id),
        "the backdated wrap to arrive after the restart",
      );
      expect(second.has(backdated.id)).toBe(true);
    } finally {
      second.client.close();
    }
  });
});
