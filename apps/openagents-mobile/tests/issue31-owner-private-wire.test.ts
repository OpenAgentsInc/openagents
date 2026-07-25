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
  decodeIssue31CommandRecordV2,
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
 * so the relay can never serve them back to the device that wrote them. A
 * device therefore cannot rebuild its own grant chain from the wire alone, and
 * these records are supplied here as device-local state.
 *
 * That is not an open gap. `openIssue31MobileNostrRuntime` already persists
 * every pairing record it authors before publishing it
 * (`publishPairingRecord` -> `Issue31LocalPairingRecordStore.put`), keyed by the
 * NIP-59 rumor id, and reloads them at start as `local://issue31-device-outbox`
 * confirmed events. The rumor id is the same identifier the Omega host folds
 * the chain under, so the two halves join.
 * "the device-local half of the grant chain is load bearing" below proves the
 * join and proves the device-local half is required rather than decorative.
 *
 * `createIssue31NostrClient` itself has no such store — the retention lives one
 * layer up, in the runtime. Every other record in these tests crossed a real
 * relay.
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
  /**
   * omega#46 exit 2: "Interrupt stays pending until the terminal record
   * arrives." Proven on a real wire rather than over hand-stapled objects.
   *
   * Three interrupts are issued at once and the host accepts all three. Each
   * accepted result names the source event that will settle it, and none of
   * those source events exists yet. All three must read as pending, because
   * host acceptance is not completion.
   *
   * Then the settling projections arrive over the relay and only ONE of the
   * three is allowed to go terminal:
   *
   *  - A's named source carries `turn.started` — a real turn record at exactly
   *    the source the host named, and still not the terminal one.
   *  - B's named source carries `turn.interrupted` for a DIFFERENT turn — the
   *    terminal entry, on the wrong turn.
   *  - C's named source carries `turn.interrupted` for C's own turn.
   *
   * A and B are the falsification of C: if the fold settled on "a projection
   * arrived" or on "the entry says interrupted", they would go terminal too.
   */
  test("an interrupt stays pending over the wire until its own terminal turn record arrives", async () => {
    const hostSecret = generateSecretKey();
    const hostPublicKey = getPublicKey(hostSecret);
    const hostSigner = LocalKeySigner.fromPrivateKey(hostSecret);
    const deviceSecret = generateSecretKey();
    const devicePublicKey = getPublicKey(deviceSecret);
    const deviceSigner = LocalKeySigner.fromPrivateKey(deviceSecret);
    const sarahSecret = generateSecretKey();
    const sarahPublicKey = getPublicKey(sarahSecret);
    const now = Math.floor(Date.now() / 1000);
    const grantRef = `grant.omega.wire_interrupt_${now}`;
    const hostRef = `omega.host.wire_interrupt_${now}`;
    const conversation = "sarah.0123456789abcdef01234567";
    const requestEventId = "d".repeat(64);
    const challengeEventId = "e".repeat(64);
    const responseEventId = "f".repeat(64);
    const challenge = "0".repeat(64);
    const scopes = ["observe_issue31", "interrupt_turn"] as const;

    const announcement = await hostSigner.signEvent({
      kind: ISSUE31_HOST_ANNOUNCEMENT_KIND,
      created_at: now,
      tags: [
        ["t", "omega-issue31-host"],
        ["d", hostRef],
        ["k", "1059"],
      ],
      content: JSON.stringify({
        schema: "openagents.omega.issue31.host_discovery.v2",
        hostRef,
        hostPublicKeyHex: hostPublicKey,
        sarahPublicKeyHex: sarahPublicKey,
        displayName: "Omega wire host",
        conversation,
        protocols: [
          "openagents.omega.issue31.pairing.v1",
          "openagents.omega.issue31.command.v1",
          "openagents.omega.issue31.command.v2",
        ],
        relayUrls: ["wss://relay.example.com"],
        generation: 1,
        issuedAt: now,
        expiresAt: now + 86_400,
      }),
    });
    await publish(announcement, announcement.id);

    /** Gift wrap one host-authored record to the device and require its OK. */
    const publishToDevice = async (record: Parameters<
      typeof createIssue31PrivateGiftWrap
    >[0]["record"]): Promise<void> => {
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
    };

    await publishToDevice(
      decodeIssue31PairingRecord({
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
        scopes: [...scopes],
        expiresAt: now + 86_400,
      }),
    );

    const cases = [
      { label: "a", turnRef: "turn.issue31.wire_a", settledTurnRef: "turn.issue31.wire_a", entry: "turn.started" },
      { label: "b", turnRef: "turn.issue31.wire_b", settledTurnRef: "turn.issue31.wire_other", entry: "turn.interrupted" },
      { label: "c", turnRef: "turn.issue31.wire_c", settledTurnRef: "turn.issue31.wire_c", entry: "turn.interrupted" },
    ] as const;
    const sourceEventIdFor = (label: string): string => label.repeat(64).slice(0, 64);
    // Each intent is authored by the DEVICE and sealed to the host, so the relay
    // can never serve it back. It is device-local state, as it is on a phone.
    const deviceIntents = cases.map(({ label, turnRef }, index): Issue31ConfirmedEvent => {
      const intentEventId = String(index + 1).repeat(64);
      return {
        relayUrl: "device://local",
        room: "owner_private",
        canonicalRecordId: intentEventId,
        privateRumorId: intentEventId,
        privateRecord: decodeIssue31CommandRecordV2({
          schema: "openagents.omega.issue31.command.v2",
          recordType: "command_intent",
          hostRef,
          hostPublicKeyHex: hostPublicKey,
          devicePublicKeyHex: devicePublicKey,
          grantRef,
          idempotencyRef: `idempotency.issue31.interrupt_${label}`,
          expectedGeneration: 1,
          arguments: {
            kind: "interrupt_turn",
            actionRef: "action.issue31.sarah.interrupt",
            conversation,
            turnRef,
          },
          issuedAt: now,
          expiresAt: now + 300,
        }),
        hostAnnouncement: null,
        event: {
          id: intentEventId,
          pubkey: devicePublicKey,
          created_at: now,
          kind: 1_059,
          tags: [["p", devicePublicKey]],
          content: "device-local",
          sig: "0".repeat(128),
        },
      };
    });

    // The host accepts every intent and names the source event that will settle
    // it. None of those sources has been published yet.
    for (const [index, { label }] of cases.entries()) {
      await publishToDevice(
        decodeIssue31CommandRecordV2({
          schema: "openagents.omega.issue31.command.v2",
          recordType: "command_result",
          hostRef,
          hostPublicKeyHex: hostPublicKey,
          devicePublicKeyHex: devicePublicKey,
          grantRef,
          intentEventId: String(index + 1).repeat(64),
          actionRef: "action.issue31.sarah.interrupt",
          idempotencyRef: `idempotency.issue31.interrupt_${label}`,
          expectedGeneration: 1,
          status: "accepted",
          handlingRef: `handling.issue31.interrupt_${label}`,
          sourceEventId: sourceEventIdFor(label),
          handledAt: now,
        }),
      );
    }

    let snapshot: Issue31NostrClientSnapshot | null = null;
    const client = createIssue31NostrClient({
      relayUrls: [relayUrl],
      signer: deviceSigner,
      webSocket: NodeSocket,
      admittedHostPublicKeys: [hostPublicKey],
      selectedHostPublicKeys: [hostPublicKey],
      ownerAuthors: [sarahPublicKey],
      ownerRecipientPublicKeys: [devicePublicKey],
      cursorStore: memoryCursorStore(),
      onSnapshot: (next) => {
        snapshot = next;
      },
    });

    const readModel = () =>
      projectIssue31OwnerPrivateReadModel(
        {
          ...snapshot!,
          confirmedEvents: [
            ...snapshot!.confirmedEvents,
            ...deviceIntents,
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
              [...scopes],
            ),
          ],
        },
        { nowUnixSeconds: now },
      );
    const stateOf = (label: string): string | undefined =>
      readModel().commands.find(
        (command) => command.idempotencyRef === `idempotency.issue31.interrupt_${label}`,
      )?.state;

    try {
      await client.start();
      const countPrivate = (recordType: string): number =>
        (snapshot?.confirmedEvents ?? []).filter(
          (row) =>
            row.privateRecord?.schema === "openagents.omega.issue31.command.v2" &&
            row.privateRecord.recordType === recordType,
        ).length;
      await waitFor(() => countPrivate("command_result") === cases.length, "every accepted result");

      // Accepted by the host, terminal record absent: pending, all three.
      expect(readModel().grantRef).toBe(grantRef);
      for (const { label } of cases) expect(stateOf(label)).toBe("accepted");

      // Now the settling projections cross the relay.
      for (const { label, settledTurnRef, entry } of cases) {
        await publishToDevice(
          decodeIssue31OwnerProjectionRecord({
            schema: "openagents.omega.issue31.owner_projection.v1",
            recordType: "owner_projection",
            hostRef,
            hostPublicKeyHex: hostPublicKey,
            devicePublicKeyHex: devicePublicKey,
            grantRef,
            expectedGeneration: 1,
            sourceEventId: sourceEventIdFor(label),
            sourceAuthorPublicKeyHex: sarahPublicKey,
            sourceRole: "sarah",
            sourceKind: 44_300,
            sourceCreatedAt: now,
            projectedAt: now,
            projection: {
              kind: "turn",
              payload: {
                schema: "openagents.sarah.turn_record.v1",
                entry,
                conversation,
                turnRef: settledTurnRef,
                seq: 2,
                timestamp: "2026-07-25T00:00:00.000Z",
                parents: [],
                payload: {},
              },
            },
          }),
        );
      }
      await waitFor(
        () =>
          (snapshot?.confirmedEvents ?? []).filter(
            (row) =>
              row.privateRecord?.schema === "openagents.omega.issue31.owner_projection.v1",
          ).length === cases.length,
        "every settling projection to arrive over the wire",
      );

      // Only C's own terminal record settles C.
      expect(stateOf("c")).toBe("terminal");
      // A: a real turn record arrived at exactly the source the host named, and
      // it is not the terminal entry. Still pending.
      expect(stateOf("a")).toBe("accepted");
      // B: the terminal entry arrived at exactly the source the host named, for
      // another turn. Still pending.
      expect(stateOf("b")).toBe("accepted");

      // The activity ladder agrees about which rows are terminal.
      const activity = readModel().activity;
      expect(
        activity.filter((row) => row.terminal).map((row) => row.turnRef).sort(),
      ).toEqual(["turn.issue31.wire_c", "turn.issue31.wire_other"]);
      expect(activity.find((row) => row.turnRef === "turn.issue31.wire_a")?.terminal).toBe(false);
    } finally {
      client.close();
    }
  }, 60_000);

  /**
   * The device-local half of the grant chain is load bearing.
   *
   * `foldIssue31Grant` needs request, challenge, response and grant. The host
   * authors the challenge and the grant and they cross the relay here. The
   * device authors the request and the response and seals them to the host, so
   * the relay can never serve them back — they come from device-local
   * retention, which `openIssue31MobileNostrRuntime` implements by persisting
   * every record `publishPairingRecord` authors under its NIP-59 rumor id.
   *
   * Dropping either device-authored record must collapse the grant, or the
   * retention would be decorative and #49 would discover that on a phone.
   */
  test("the device-local half of the grant chain is load bearing", async () => {
    const hostSecret = generateSecretKey();
    const hostPublicKey = getPublicKey(hostSecret);
    const hostSigner = LocalKeySigner.fromPrivateKey(hostSecret);
    const deviceSecret = generateSecretKey();
    const devicePublicKey = getPublicKey(deviceSecret);
    const deviceSigner = LocalKeySigner.fromPrivateKey(deviceSecret);
    const sarahPublicKey = getPublicKey(generateSecretKey());
    const now = Math.floor(Date.now() / 1000);
    const grantRef = `grant.omega.wire_chain_${now}`;
    const hostRef = `omega.host.wire_chain_${now}`;
    // The device chooses these two: it authors both records, so on a phone they
    // are the NIP-59 rumor ids `publishPairingRecord` stores them under.
    const requestEventId = "7".repeat(64);
    const responseEventId = "9".repeat(64);
    const challenge = "8".repeat(64);
    const scopes = ["observe_issue31"] as const;

    const announcement = await hostSigner.signEvent({
      kind: ISSUE31_HOST_ANNOUNCEMENT_KIND,
      created_at: now,
      tags: [
        ["t", "omega-issue31-host"],
        ["d", hostRef],
        ["k", "1059"],
      ],
      content: JSON.stringify({
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
      }),
    });
    await publish(announcement, announcement.id);

    // Both HOST-authored halves of the chain cross the relay for real.
    for (const record of [
      decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "pairing_challenge",
        hostRef,
        hostPublicKeyHex: hostPublicKey,
        devicePublicKeyHex: devicePublicKey,
        issuedAt: now - 2,
        pairingChallengeRef: "pairing.challenge.chain",
        pairingRequestEventId: requestEventId,
        challenge,
        expiresAt: now + 86_400,
      }),
      decodeIssue31PairingRecord({
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
        scopes: [...scopes],
        expiresAt: now + 86_400,
      }),
    ]) {
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
            (row) => row.privateRecord?.schema === "openagents.omega.issue31.pairing.v1",
          ).length === 2,
        "the host-authored challenge and grant to arrive over the wire",
      );

      // The response must answer the challenge under the identifier the WIRE
      // gave it, not a fabricated one. That is the join between the two halves:
      // the client's canonical record id is the NIP-59 rumor id, which is the
      // same identifier the Omega host folds the chain under.
      const wireChallengeEventId = (snapshot!.confirmedEvents.find(
        (row) => row.privateRecord?.recordType === "pairing_challenge",
      ) ?? null)?.canonicalRecordId;
      expect(wireChallengeEventId).toBeTypeOf("string");

      const deviceLocal = (
        canonicalRecordId: string,
        record: ReturnType<typeof decodeIssue31PairingRecord>,
      ): Issue31ConfirmedEvent => ({
        relayUrl: "local://issue31-device-outbox",
        room: "owner_private",
        canonicalRecordId,
        privateRumorId: canonicalRecordId,
        privateRecord: record,
        hostAnnouncement: null,
        event: {
          id: canonicalRecordId,
          pubkey: devicePublicKey,
          created_at: now,
          kind: 1_059,
          tags: [["p", devicePublicKey]],
          content: "device-local",
          sig: "0".repeat(128),
        },
      });
      const deviceAuthored = [
        deviceLocal(
          requestEventId,
          decodeIssue31PairingRecord({
            schema: ISSUE31_PAIRING_SCHEMA,
            recordType: "pairing_request",
            hostRef,
            hostPublicKeyHex: hostPublicKey,
            devicePublicKeyHex: devicePublicKey,
            issuedAt: now - 3,
            pairingRequestRef: "pairing.request.chain",
            requestedScopes: [...scopes],
            expiresAt: now + 86_400,
          }),
        ),
        deviceLocal(
          responseEventId,
          decodeIssue31PairingRecord({
            schema: ISSUE31_PAIRING_SCHEMA,
            recordType: "pairing_response",
            hostRef,
            hostPublicKeyHex: hostPublicKey,
            devicePublicKeyHex: devicePublicKey,
            issuedAt: now - 1,
            pairingResponseRef: "pairing.response.chain",
            pairingChallengeEventId: wireChallengeEventId!,
            challenge,
            expiresAt: now + 86_400,
          }),
        ),
      ];

      const modelWith = (events: ReadonlyArray<Issue31ConfirmedEvent>) =>
        projectIssue31OwnerPrivateReadModel(
          { ...snapshot!, confirmedEvents: [...snapshot!.confirmedEvents, ...events] },
          { nowUnixSeconds: now },
        );

      // Wire half plus device-local half: the chain resolves.
      expect(modelWith(deviceAuthored).grantRef).toBe(grantRef);
      expect(modelWith(deviceAuthored).generation).toBe(1);

      // The wire alone cannot rebuild it, and neither device-authored record is
      // redundant. Without device-local retention a phone loses its own grant.
      expect(modelWith([]).grantRef).toBeNull();
      for (const dropped of deviceAuthored) {
        expect(
          modelWith(deviceAuthored.filter((event) => event !== dropped)).grantRef,
        ).toBeNull();
      }
    } finally {
      client.close();
    }
  }, 60_000);
});
