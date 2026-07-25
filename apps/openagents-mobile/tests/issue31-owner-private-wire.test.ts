/**
 * The owner-private read state, reminders, and authority receipts over a REAL
 * relay (omega#46 exits 3, 5, 6).
 *
 * `issue31-owner-private-read-model.test.ts` proves the fold. It hands the read
 * model already-decoded records, so it proves arithmetic, not delivery. This
 * runs the real client against a real relay: real WebSocket framing, real
 * storage, real `#p` filter matching, real EOSE, real NIP-59 gift wraps that
 * are really unwrapped and really NIP-44 decrypted on the device side.
 *
 * The projection bodies are read from the fixtures the Omega host emits and
 * pins by digest, so what crosses the wire is what the host sends. Only the
 * identity fields are rebound to live keys, because the fixture keys are
 * placeholders and cannot sign or hold a NIP-44 conversation key.
 *
 * A local relay is a real relay and is categorically not `MockRelayAdapter`.
 * It is also categorically not a phone: the omega#46 and omega#49 exits that
 * name a physical device still need one.
 *
 * Set `OMEGA_ISSUE31_WIRE_RELAY_URL` to run the same suite against a deployed
 * relay instead of the in-process one.
 */
import { readFileSync } from "node:fs";

import {
  ISSUE31_HOST_ANNOUNCEMENT_KIND,
  ISSUE31_PAIRING_SCHEMA,
  createIssue31PrivateGiftWrap,
  decodeIssue31OwnerProjectionRecord,
  decodeIssue31PairingRecord,
} from "@openagentsinc/sarah/issue31-nostr";
import { LocalKeySigner } from "nostr-effect/identity";
import { generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { startTestRelay } from "nostr-effect/relay/node";
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test";

import {
  createIssue31NostrClient,
  type Issue31ConfirmedEvent,
  type Issue31NostrClientSnapshot,
  type Issue31RelayCursor,
  type Issue31RelayCursorStore,
  type Issue31WebSocketLike,
} from "../src/workroom/issue31-nostr-client.ts";
import { projectIssue31OwnerPrivateReadModel } from "../src/workroom/issue31-owner-private-read-model.ts";

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

const memoryCursorStore = (): Issue31RelayCursorStore => {
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
  timeoutMs = 20_000,
): Promise<void> => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`timed out waiting for ${label}`);
};

const deployedRelayUrl = process.env["OMEGA_ISSUE31_WIRE_RELAY_URL"];
let relay: Awaited<ReturnType<typeof startTestRelay>> | null = null;
let relayUrl = deployedRelayUrl ?? "";

beforeAll(async () => {
  if (deployedRelayUrl === undefined) {
    relay = await startTestRelay(36_000 + Math.floor(Math.random() * 4_000));
    relayUrl = `ws://127.0.0.1:${relay.port}`;
  }
});

afterAll(async () => {
  await Promise.resolve(relay?.stop());
});

/** Publish one event and require the relay's own affirmative OK for it. */
const publish = async (event: unknown, eventId: string): Promise<string> => {
  const socket = new WebSocket(relayUrl);
  try {
    await new Promise<void>((resolve, reject) => {
      socket.onopen = () => resolve();
      socket.onerror = () => reject(new Error("publisher socket failed"));
    });
    const accepted = new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`no OK for ${eventId}`)), 15_000);
      socket.onmessage = (message: MessageEvent) => {
        const frame = JSON.parse(String(message.data)) as ReadonlyArray<unknown>;
        if (frame[0] === "OK" && frame[1] === eventId) {
          clearTimeout(timer);
          if (frame[2] === true) resolve();
          else reject(new Error(`relay refused ${eventId}: ${String(frame[3])}`));
        }
      };
    });
    socket.send(JSON.stringify(["EVENT", event]));
    await accepted;
    return eventId;
  } finally {
    socket.close(1000, "done");
  }
};

const hostFixture = (name: string): Record<string, unknown> =>
  JSON.parse(
    readFileSync(
      new URL(
        `../../../packages/sarah/fixtures/issue31-nostr/openagents.omega.issue31.owner_projection.v1.${name}.json`,
        import.meta.url,
      ),
      "utf8",
    ),
  ) as Record<string, unknown>;


/**
 * The two pairing records the DEVICE authored.
 *
 * `foldIssue31Grant` requires the whole chain — request, challenge, response,
 * grant — and the device's request and response are NIP-44 sealed to the host,
 * so the relay can never serve them back to the device that wrote them. The
 * client has no durable store for its own authored records either, so a device
 * cannot rebuild its own grant chain from the wire alone. That is a real gap
 * and it is reported on omega#46. Here the two device-authored records are
 * supplied as device-local state, which is where they would have to live. Every
 * other record in these tests crossed a real relay.
 */
const deviceAuthoredPairing = (
  hostRef: string,
  hostPublicKey: string,
  devicePublicKey: string,
  issuedAt: number,
  expiresAt: number,
  requestEventId: string,
  challengeEventId: string,
  responseEventId: string,
  challenge: string,
  scopes: ReadonlyArray<string>,
): ReadonlyArray<Issue31ConfirmedEvent> => {
  const localEvent = (
    canonicalRecordId: string,
    record: ReturnType<typeof decodeIssue31PairingRecord>,
    author: string,
  ): Issue31ConfirmedEvent => ({
    relayUrl: "device://local",
    room: "owner_private",
    canonicalRecordId,
    privateRumorId: canonicalRecordId,
    privateRecord: record,
    hostAnnouncement: null,
    event: {
      id: canonicalRecordId,
      pubkey: author,
      created_at: issuedAt,
      kind: 1_059,
      tags: [["p", devicePublicKey]],
      content: "device-local",
      sig: "0".repeat(128),
    },
  });
  return [
    localEvent(
      requestEventId,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "pairing_request",
        hostRef,
        hostPublicKeyHex: hostPublicKey,
        devicePublicKeyHex: devicePublicKey,
        issuedAt: issuedAt - 3,
        pairingRequestRef: "pairing.request.wire",
        requestedScopes: scopes,
        expiresAt,
      }),
      devicePublicKey,
    ),
    localEvent(
      challengeEventId,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "pairing_challenge",
        hostRef,
        hostPublicKeyHex: hostPublicKey,
        devicePublicKeyHex: devicePublicKey,
        issuedAt: issuedAt - 2,
        pairingChallengeRef: "pairing.challenge.wire",
        pairingRequestEventId: requestEventId,
        challenge,
        expiresAt,
      }),
      hostPublicKey,
    ),
    localEvent(
      responseEventId,
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "pairing_response",
        hostRef,
        hostPublicKeyHex: hostPublicKey,
        devicePublicKeyHex: devicePublicKey,
        issuedAt: issuedAt - 1,
        pairingResponseRef: "pairing.response.wire",
        pairingChallengeEventId: challengeEventId,
        challenge,
        expiresAt,
      }),
      devicePublicKey,
    ),
  ];
};

describe("owner-private read state, reminders, and receipts over a real relay", () => {
  test("the phone reads every owner-state body the host published", async () => {
    const hostSecret = generateSecretKey();
    const hostPublicKey = getPublicKey(hostSecret);
    const hostSigner = LocalKeySigner.fromPrivateKey(hostSecret);
    const deviceSecret = generateSecretKey();
    const devicePublicKey = getPublicKey(deviceSecret);
    const deviceSigner = LocalKeySigner.fromPrivateKey(deviceSecret);
    const sarahPublicKey = getPublicKey(generateSecretKey());
    const now = Math.floor(Date.now() / 1000);
    const grantRef = `grant.omega.wire_${now}`;
    // A relay run keeps every earlier run's events. A unique host reference
    // keeps this run's discovery record from colliding on a deployed relay.
    const hostRef = `omega.host.wire_${now}`;
    const requestEventId = "1".repeat(64);
    const challengeEventId = "2".repeat(64);
    const responseEventId = "3".repeat(64);
    const challenge = "4".repeat(64);

    const announcementBody = {
      schema: "openagents.omega.issue31.host_discovery.v2",
      hostRef,
      hostPublicKeyHex: hostPublicKey,
      sarahPublicKeyHex: sarahPublicKey,
      displayName: "Omega wire host",
      conversation: "sarah.0123456789abcdef01234567",
      protocols: [
        "openagents.omega.issue31.pairing.v1",
        "openagents.omega.issue31.command.v1",
        "openagents.omega.issue31.command.v2",
      ],
      relayUrls: ["wss://relay.example.com"],
      generation: 1,
      issuedAt: now,
      expiresAt: now + 86_400,
    };
    const announcement = await hostSigner.signEvent({
      kind: ISSUE31_HOST_ANNOUNCEMENT_KIND,
      created_at: now,
      tags: [
        ["t", "omega-issue31-host"],
        ["d", hostRef],
        ["k", "1059"],
      ],
      content: JSON.stringify(announcementBody),
    });
    await publish(announcement, announcement.id);

    // Only the host-authored grant is needed on the device: the fold is keyed
    // by grant reference and the device's own request and response are sealed
    // to the host, not to itself.
    const grant = decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "scoped_grant",
      hostRef,
      hostPublicKeyHex: hostPublicKey,
      sarahPublicKeyHex: sarahPublicKey,
      devicePublicKeyHex: devicePublicKey,
      issuedAt: now,
      pairingResponseEventId: responseEventId,
      grantRef,
      generation: 1,
      scopes: ["observe_issue31", "send_message"],
      expiresAt: now + 86_400,
    });
    const grantWrap = await createIssue31PrivateGiftWrap({
      signer: hostSigner,
      recipientPublicKeyHex: devicePublicKey,
      record: grant,
      randomSecretKey: generateSecretKey,
      createdAt: now,
      sealCreatedAt: now,
      wrapCreatedAt: now,
    });
    await publish(grantWrap, grantWrap.id);

    const bodies = [
      "canonical-read-state",
      "canonical-reminder",
      "canonical-authority-receipt",
      "canonical-engram",
    ] as const;
    for (const name of bodies) {
      const fixture = hostFixture(name);
      const record = decodeIssue31OwnerProjectionRecord({
        ...fixture,
        hostRef,
        hostPublicKeyHex: hostPublicKey,
        devicePublicKeyHex: devicePublicKey,
        grantRef,
        expectedGeneration: 1,
        sourceAuthorPublicKeyHex:
          fixture["sourceRole"] === "sarah" ? sarahPublicKey : hostPublicKey,
      });
      const wrap = await createIssue31PrivateGiftWrap({
        signer: hostSigner,
        recipientPublicKeyHex: devicePublicKey,
        record,
        randomSecretKey: generateSecretKey,
        createdAt: now,
        sealCreatedAt: now,
        wrapCreatedAt: now,
      });
      await publish(wrap, wrap.id);
    }

    let snapshot: Issue31NostrClientSnapshot | null = null;
    const client = createIssue31NostrClient({
      relayUrls: [relayUrl],
      signer: deviceSigner,
      webSocket: NodeSocket,
      admittedHostPublicKeys: [hostPublicKey],
      // Host selection is the device's admission decision. Without it the
      // client refuses every gift wrap the relay serves, which is the point.
      selectedHostPublicKeys: [hostPublicKey],
      ownerAuthors: [sarahPublicKey],
      ownerRecipientPublicKeys: [devicePublicKey],
      cursorStore: memoryCursorStore(),
      onSnapshot: (next) => {
        snapshot = next;
      },
    });

    try {
      await client.start();
      await waitFor(
        () =>
          (snapshot?.confirmedEvents ?? []).filter(
            (row) =>
              row.privateRecord?.schema === "openagents.omega.issue31.owner_projection.v1",
          ).length === bodies.length,
        "every owner projection to arrive over the wire",
      );
      await waitFor(
        () => (snapshot?.relays ?? []).some((row) => row.state === "live"),
        "the relay to reach live after a real EOSE",
      );

      const model = projectIssue31OwnerPrivateReadModel(
        {
          ...snapshot!,
          confirmedEvents: [
            ...snapshot!.confirmedEvents,
            ...deviceAuthoredPairing(
              hostRef,
              hostPublicKey,
              devicePublicKey,
              now,
              now + 86_400,
              requestEventId,
              challengeEventId,
              responseEventId,
              challenge,
              ["observe_issue31", "send_message"],
            ),
          ],
        },
        { nowUnixSeconds: now },
      );
      expect(model.grantRef).toBe(grantRef);
      expect(model.readContexts).toEqual({
        "sarah.0123456789abcdef01234567": 1_784_937_608,
      });
      expect(model.reminders).toHaveLength(1);
      expect(model.reminders[0]).toMatchObject({
        reminderId: "0123456789abcdef0123456789abcdef",
        content: { status: "pending", note: "Check the release evidence." },
        notBefore: 1_784_938_000,
      });
      expect(model.receipts).toHaveLength(1);
      expect(model.receipts[0]).toMatchObject({
        receiptRef: `receipt.issue31.${"a".repeat(24)}`,
        turnRef: "turn.issue31.release_evidence",
        authorityState: "refused",
        authorityReasonRef: "reason.openagents.reserved_custody",
        targetState: "pending",
        outcomeRef: null,
      });
      expect(model.memory[0]?.body).toEqual({
        slug: "mem/release_evidence",
        value: "The release candidate is notarized.",
      });
    } finally {
      client.close();
    }
  }, 60_000);

  test("a projection bound to another device is stored by the relay and still refused", async () => {
    // The relay accepts and serves it. Admission is the device's decision, not
    // the relay's, so the record must not become a row.
    const hostSecret = generateSecretKey();
    const hostPublicKey = getPublicKey(hostSecret);
    const hostSigner = LocalKeySigner.fromPrivateKey(hostSecret);
    const deviceSecret = generateSecretKey();
    const devicePublicKey = getPublicKey(deviceSecret);
    const deviceSigner = LocalKeySigner.fromPrivateKey(deviceSecret);
    const sarahPublicKey = getPublicKey(generateSecretKey());
    const now = Math.floor(Date.now() / 1000);
    const grantRef = `grant.omega.wire_refused_${now}`;
    const hostRef = `omega.host.wire_refused_${now}`;
    const requestEventId = "5".repeat(64);
    const challengeEventId = "6".repeat(64);
    const responseEventId = "7".repeat(64);
    const challenge = "8".repeat(64);

    const announcementBody = {
      schema: "openagents.omega.issue31.host_discovery.v2",
      hostRef,
      hostPublicKeyHex: hostPublicKey,
      sarahPublicKeyHex: sarahPublicKey,
      displayName: "Omega wire host",
      conversation: "sarah.0123456789abcdef01234567",
      protocols: [
        "openagents.omega.issue31.pairing.v1",
        "openagents.omega.issue31.command.v1",
        "openagents.omega.issue31.command.v2",
      ],
      relayUrls: ["wss://relay.example.com"],
      generation: 1,
      issuedAt: now,
      expiresAt: now + 86_400,
    };
    const announcement = await hostSigner.signEvent({
      kind: ISSUE31_HOST_ANNOUNCEMENT_KIND,
      created_at: now,
      tags: [
        ["t", "omega-issue31-host"],
        ["d", hostRef],
        ["k", "1059"],
      ],
      content: JSON.stringify(announcementBody),
    });
    await publish(announcement, announcement.id);

    const grant = decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "scoped_grant",
      hostRef,
      hostPublicKeyHex: hostPublicKey,
      sarahPublicKeyHex: sarahPublicKey,
      devicePublicKeyHex: devicePublicKey,
      issuedAt: now,
      pairingResponseEventId: responseEventId,
      grantRef,
      generation: 1,
      scopes: ["observe_issue31"],
      expiresAt: now + 86_400,
    });
    const grantWrap = await createIssue31PrivateGiftWrap({
      signer: hostSigner,
      recipientPublicKeyHex: devicePublicKey,
      record: grant,
      randomSecretKey: generateSecretKey,
      createdAt: now,
      sealCreatedAt: now,
      wrapCreatedAt: now,
    });
    await publish(grantWrap, grantWrap.id);

    const fixture = hostFixture("canonical-read-state");
    const staleGeneration = decodeIssue31OwnerProjectionRecord({
      ...fixture,
      hostRef,
      hostPublicKeyHex: hostPublicKey,
      devicePublicKeyHex: devicePublicKey,
      grantRef,
      expectedGeneration: 2,
      sourceAuthorPublicKeyHex: hostPublicKey,
    });
    const staleWrap = await createIssue31PrivateGiftWrap({
      signer: hostSigner,
      recipientPublicKeyHex: devicePublicKey,
      record: staleGeneration,
      randomSecretKey: generateSecretKey,
      createdAt: now,
      sealCreatedAt: now,
      wrapCreatedAt: now,
    });
    const staleId = await publish(staleWrap, staleWrap.id);

    let snapshot: Issue31NostrClientSnapshot | null = null;
    const client = createIssue31NostrClient({
      relayUrls: [relayUrl],
      signer: deviceSigner,
      webSocket: NodeSocket,
      admittedHostPublicKeys: [hostPublicKey],
      // Host selection is the device's admission decision. Without it the
      // client refuses every gift wrap the relay serves, which is the point.
      selectedHostPublicKeys: [hostPublicKey],
      ownerAuthors: [sarahPublicKey],
      ownerRecipientPublicKeys: [devicePublicKey],
      cursorStore: memoryCursorStore(),
      onSnapshot: (next) => {
        snapshot = next;
      },
    });

    try {
      await client.start();
      await waitFor(
        () => (snapshot?.confirmedEvents ?? []).some((row) => row.event.id === staleId),
        "the stale-generation projection to arrive over the wire",
      );
      await waitFor(
        () => (snapshot?.relays ?? []).some((row) => row.state === "live"),
        "the relay to reach live",
      );

      const model = projectIssue31OwnerPrivateReadModel(
        {
          ...snapshot!,
          confirmedEvents: [
            ...snapshot!.confirmedEvents,
            ...deviceAuthoredPairing(
              hostRef,
              hostPublicKey,
              devicePublicKey,
              now,
              now + 86_400,
              requestEventId,
              challengeEventId,
              responseEventId,
              challenge,
              ["observe_issue31"],
            ),
          ],
        },
        { nowUnixSeconds: now },
      );
      expect(model.grantRef).toBe(grantRef);
      expect(model.readContexts).toEqual({});
      expect(model.status).toBe("gap");
      expect(model.reasonRef).toBe("reason.issue31.owner_projection_rejected");
    } finally {
      client.close();
    }
  }, 60_000);
});
