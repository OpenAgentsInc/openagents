/**
 * The omega#47 adjuncts crossing a REAL relay to a paired device (omega#49).
 *
 * `issue31-full-auto-projection-source.test.ts` proves the reader: hand it two
 * adjuncts and a grant and it decides correctly. It hands them over by direct
 * call, so it proves the decision and nothing about delivery — and delivery was
 * the whole gap. `Issue31PrivateRecord` admitted five schemas and neither
 * `host.v1` nor `fullauto.v1` was one of them, so the client discarded both
 * before any reader could see them and every device, paired or not, reported
 * `no_host_projection`.
 *
 * These tests run the real client against a real in-process relay: real
 * WebSocket framing, real relay storage, real `#p` filter matching, real EOSE,
 * real NIP-59 gift wraps really unwrapped and NIP-44 decrypted on the device.
 * A local relay is a real relay and is categorically not a mock. It is also
 * categorically not a phone.
 *
 * What is under test is the *delivery envelope*: an adjunct is admitted only
 * when it states the host that signed the seal, the device that unwrapped it,
 * and the grant it travelled under — and only when those statements do not
 * contradict the pairing chain the device holds.
 */
import { readFileSync } from "node:fs";

import { Effect, Stream } from "@effect-native/core/effect";

import {
  ISSUE31_HOST_ANNOUNCEMENT_KIND,
  ISSUE31_PAIRING_SCHEMA,
  createIssue31PrivateGiftWrap,
  decodeIssue31PairingRecord,
  type Issue31PrivateRecord,
} from "@openagentsinc/sarah/issue31-nostr";
import { LocalKeySigner } from "nostr-effect/identity";
import { generateSecretKey, getEventHash, getPublicKey } from "nostr-effect/pure";
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
import { issue31FullAutoProjectionFromSnapshot } from "../src/workroom/issue31-full-auto-projection-source.ts";
import { projectIssue31Workroom } from "../src/workroom/issue31-workroom-projection.ts";
import { ISSUE31_CAPABILITY_DESCRIPTORS } from "../src/workroom/issue31-workroom-read-model.ts";
import { buildHomeProgram, renderContentView } from "../src/screens/home-core.ts";

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
  timeoutMs = 15_000,
): Promise<void> => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error(`timed out waiting for ${label}`);
};

const workroomFixture = (name: string): Record<string, unknown> =>
  JSON.parse(
    readFileSync(
      new URL(
        `../../../packages/sarah/fixtures/issue31-workroom/openagents.omega.issue31.${name}.json`,
        import.meta.url,
      ),
      "utf8",
    ),
  ) as Record<string, unknown>;

let relay: Awaited<ReturnType<typeof startTestRelay>>;
let relayUrl: string;

beforeAll(async () => {
  relay = await startTestRelay(36_000 + Math.floor(Math.random() * 3_000));
  relayUrl = `ws://127.0.0.1:${relay.port}`;
});

afterAll(async () => {
  await Promise.resolve(relay?.stop());
});

/** Publish one signed event to the relay and wait for the relay's own OK. */
const publish = async (event: unknown, eventId: string): Promise<void> => {
  const socket = new WebSocket(relayUrl);
  await new Promise<void>((resolve, reject) => {
    socket.onopen = () => resolve();
    socket.onerror = () => reject(new Error("publisher socket failed"));
  });
  const accepted = new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`no OK for ${eventId}`)), 10_000);
    socket.onmessage = (message: MessageEvent) => {
      const frame = JSON.parse(String(message.data)) as ReadonlyArray<unknown>;
      if (frame[0] === "OK" && frame[1] === eventId) {
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

/**
 * The device-authored half of its own pairing chain.
 *
 * The device's request and response are NIP-44 sealed to the host, so a relay
 * can never serve them back to their author. `openIssue31MobileNostrRuntime`
 * persists them at publish time and reloads them as local confirmed events;
 * this reproduces that device-local half, exactly as the sibling wire test
 * does. Everything else here crossed the relay.
 */
const deviceLocalEvent = (
  canonicalRecordId: string,
  record: Issue31PrivateRecord,
  author: string,
  devicePublicKey: string,
  createdAt: number,
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
    created_at: createdAt,
    kind: 1_059,
    tags: [["p", devicePublicKey]],
    content: "device-local",
    sig: "0".repeat(128),
  },
});

interface Harness {
  readonly hostPublicKey: string;
  readonly hostSigner: LocalKeySigner;
  readonly devicePublicKey: string;
  readonly deviceSigner: LocalKeySigner;
  readonly sarahPublicKey: string;
  readonly now: number;
  readonly hostRef: string;
  readonly grantRef: string;
  readonly snapshotRef: string;
  readonly deviceLocalPairing: ReadonlyArray<Issue31ConfirmedEvent>;
}

/**
 * A real host, a real device, a real host announcement, and a real grant chain
 * whose host-authored half went over the relay.
 */
const pairedHarness = async (label: string): Promise<Harness> => {
  const hostSecret = generateSecretKey();
  const hostPublicKey = getPublicKey(hostSecret);
  const hostSigner = LocalKeySigner.fromPrivateKey(hostSecret);
  const deviceSecret = generateSecretKey();
  const devicePublicKey = getPublicKey(deviceSecret);
  const deviceSigner = LocalKeySigner.fromPrivateKey(deviceSecret);
  const sarahPublicKey = getPublicKey(generateSecretKey());
  const now = Math.floor(Date.now() / 1000);
  const hostRef = `omega.host.adjunct-${label}`;
  const grantRef = `grant.omega.adjunct-${label}`;
  const snapshotRef = `snapshot.omega.adjunct-${label}`;
  const requestEventId = "1".repeat(64);
  const challengeEventId = "2".repeat(64);
  const responseEventId = "3".repeat(64);
  const challenge = "4".repeat(64);
  const identity = {
    hostRef,
    hostPublicKeyHex: hostPublicKey,
    devicePublicKeyHex: devicePublicKey,
  };
  const scopes = ["observe_issue31", "control_full_auto"] as const;

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
      ...identity,
      sarahPublicKeyHex: sarahPublicKey,
      displayName: "Omega adjunct host",
      conversation: "sarah.0123456789abcdef01234567",
      protocols: [
        ISSUE31_PAIRING_SCHEMA,
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

  const grant = decodeIssue31PairingRecord({
    schema: ISSUE31_PAIRING_SCHEMA,
    recordType: "scoped_grant",
    ...identity,
    sarahPublicKeyHex: sarahPublicKey,
    issuedAt: now,
    pairingResponseEventId: responseEventId,
    grantRef,
    generation: 1,
    scopes,
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

  return {
    hostPublicKey,
    hostSigner,
    devicePublicKey,
    deviceSigner,
    sarahPublicKey,
    now,
    hostRef,
    grantRef,
    snapshotRef,
    deviceLocalPairing: [
      deviceLocalEvent(
        requestEventId,
        decodeIssue31PairingRecord({
          schema: ISSUE31_PAIRING_SCHEMA,
          recordType: "pairing_request",
          ...identity,
          issuedAt: now - 3,
          pairingRequestRef: "pairing.request.adjunct",
          requestedScopes: scopes,
          expiresAt: now + 86_400,
        }),
        devicePublicKey,
        devicePublicKey,
        now - 3,
      ),
      deviceLocalEvent(
        challengeEventId,
        decodeIssue31PairingRecord({
          schema: ISSUE31_PAIRING_SCHEMA,
          recordType: "pairing_challenge",
          ...identity,
          issuedAt: now - 2,
          pairingChallengeRef: "pairing.challenge.adjunct",
          pairingRequestEventId: requestEventId,
          challenge,
          expiresAt: now + 86_400,
        }),
        hostPublicKey,
        devicePublicKey,
        now - 2,
      ),
      deviceLocalEvent(
        responseEventId,
        decodeIssue31PairingRecord({
          schema: ISSUE31_PAIRING_SCHEMA,
          recordType: "pairing_response",
          ...identity,
          issuedAt: now - 1,
          pairingResponseRef: "pairing.response.adjunct",
          pairingChallengeEventId: challengeEventId,
          challenge,
          expiresAt: now + 86_400,
        }),
        devicePublicKey,
        devicePublicKey,
        now - 1,
      ),
    ],
  };
};

/** The delivery binding an adjunct must state to be admitted at all. */
const delivery = (
  harness: Harness,
  recordType: "host_snapshot" | "full_auto_detail",
  overrides: Readonly<Record<string, unknown>> = {},
): Record<string, unknown> => ({
  recordType,
  hostPublicKeyHex: harness.hostPublicKey,
  devicePublicKeyHex: harness.devicePublicKey,
  grantRef: harness.grantRef,
  expectedGeneration: 1,
  ...overrides,
});

/**
 * The pinned host fixture, rebound to this run's live host, snapshot and keys.
 *
 * Only identity is rebound: the projections, refs, and states are the host's
 * own pinned bytes, so what crosses the wire is what the host emits.
 */
const deliveredHostAdjunct = (
  harness: Harness,
  overrides: Readonly<Record<string, unknown>> = {},
): Record<string, unknown> => ({
  ...workroomFixture("host.v1.canonical"),
  hostRef: harness.hostRef,
  snapshotRef: harness.snapshotRef,
  ...delivery(harness, "host_snapshot"),
  ...overrides,
});

const deliveredFullAutoAdjunct = (
  harness: Harness,
  overrides: Readonly<Record<string, unknown>> = {},
): Record<string, unknown> => ({
  ...workroomFixture("fullauto.v1.canonical"),
  hostRef: harness.hostRef,
  snapshotRef: harness.snapshotRef,
  ...delivery(harness, "full_auto_detail"),
  ...overrides,
});

/** Gift wrap a valid delivered record through the shipped envelope builder. */
const publishHostRumor = async (
  harness: Harness,
  body: Record<string, unknown>,
): Promise<string> => {
  const wrap = await createIssue31PrivateGiftWrap({
    signer: harness.hostSigner,
    recipientPublicKeyHex: harness.devicePublicKey,
    record: body as unknown as Issue31PrivateRecord,
    randomSecretKey: generateSecretKey,
    createdAt: harness.now,
    sealCreatedAt: harness.now,
    wrapCreatedAt: harness.now,
  });
  await publish(wrap, wrap.id);
  return wrap.id;
};

/**
 * The same NIP-59 envelope, built by hand so a body the contract REFUSES can
 * still reach the wire.
 *
 * `createIssue31PrivateGiftWrap` validates before it seals, which is correct
 * and is asserted separately — but it also means the shipped builder cannot
 * produce the events these negative cases need. A hostile or broken host has
 * no such scruple, so the device has to be tested against what one would
 * actually send. The sealing, NIP-44 encryption and signing here are the real
 * ones; only the sender-side validation is skipped.
 */
const publishRawHostRumor = async (
  harness: Harness,
  body: unknown,
): Promise<string> => {
  const rumorBase = {
    pubkey: harness.hostPublicKey,
    created_at: harness.now,
    kind: 14,
    tags: [["p", harness.devicePublicKey]],
    content: JSON.stringify(body),
  };
  const rumor = { ...rumorBase, id: getEventHash(rumorBase) };
  const seal = await harness.hostSigner.signEvent({
    kind: 13,
    created_at: harness.now,
    tags: [],
    content: await harness.hostSigner.nip44Encrypt(
      harness.devicePublicKey,
      JSON.stringify(rumor),
    ),
  });
  const ephemeral = LocalKeySigner.fromPrivateKey(generateSecretKey());
  try {
    const wrap = await ephemeral.signEvent({
      kind: 1_059,
      created_at: harness.now,
      tags: [["p", harness.devicePublicKey]],
      content: await ephemeral.nip44Encrypt(harness.devicePublicKey, JSON.stringify(seal)),
    });
    await publish(wrap, wrap.id);
    return wrap.id;
  } finally {
    ephemeral.dispose();
  }
};

const startDeviceClient = async (
  harness: Harness,
): Promise<{
  readonly close: () => void;
  readonly snapshot: () => Issue31NostrClientSnapshot;
  readonly awaitSnapshot: (
    predicate: (snapshot: Issue31NostrClientSnapshot) => boolean,
    label: string,
  ) => Promise<void>;
}> => {
  let latest: Issue31NostrClientSnapshot | null = null;
  const client = createIssue31NostrClient({
    relayUrls: [relayUrl],
    signer: harness.deviceSigner,
    webSocket: NodeSocket,
    admittedHostPublicKeys: [harness.hostPublicKey],
    selectedHostPublicKeys: [harness.hostPublicKey],
    ownerAuthors: [harness.sarahPublicKey],
    ownerRecipientPublicKeys: [harness.devicePublicKey],
    cursorStore: memoryCursorStore(),
    onSnapshot: (next) => {
      latest = next;
    },
  });
  await client.start();
  return {
    close: () => client.close(),
    snapshot: () => client.snapshot(),
    awaitSnapshot: (predicate, label) =>
      waitFor(() => latest !== null && predicate(latest), label),
  };
};

/**
 * What the mobile runtime hands the Full Auto reader: the relay-confirmed
 * events plus the device's own pairing records.
 */
const readerSnapshot = (
  harness: Harness,
  snapshot: Issue31NostrClientSnapshot,
): Parameters<typeof issue31FullAutoProjectionFromSnapshot>[0] => ({
  devicePublicKeyHex: snapshot.devicePublicKeyHex,
  admittedHostPublicKeys: snapshot.admittedHostPublicKeys,
  selectedHostPublicKeys: snapshot.selectedHostPublicKeys,
  confirmedEvents: [...harness.deviceLocalPairing, ...snapshot.confirmedEvents],
});

const recordsWithSchema = (
  snapshot: Issue31NostrClientSnapshot,
  schema: string,
): ReadonlyArray<Issue31ConfirmedEvent> =>
  snapshot.confirmedEvents.filter((row) => row.privateRecord?.schema === schema);

describe("omega#47 adjuncts delivered to a paired device over a real relay", () => {
  test("the phone reads the host snapshot and its Full Auto detail", async () => {
    const harness = await pairedHarness(`ok-${Date.now()}`);
    await publishHostRumor(harness, deliveredHostAdjunct(harness));
    await publishHostRumor(harness, deliveredFullAutoAdjunct(harness));
    const device = await startDeviceClient(harness);
    try {
      await device.awaitSnapshot(
        (snapshot) =>
          recordsWithSchema(snapshot, "openagents.omega.issue31.host.v1").length === 1 &&
          recordsWithSchema(snapshot, "openagents.omega.issue31.fullauto.v1").length === 1,
        "both adjuncts to arrive over the wire",
      );
      const snapshot = device.snapshot();

      // The record survived the round trip with its delivery binding intact —
      // this is the part that used to be dropped on the floor.
      const host = recordsWithSchema(snapshot, "openagents.omega.issue31.host.v1")[0]!;
      expect(host.privateRecord).toMatchObject({
        recordType: "host_snapshot",
        hostPublicKeyHex: harness.hostPublicKey,
        devicePublicKeyHex: harness.devicePublicKey,
        grantRef: harness.grantRef,
        hostRef: harness.hostRef,
      });

      const model = issue31FullAutoProjectionFromSnapshot(
        readerSnapshot(harness, snapshot),
        harness.now,
      );
      if (model.state !== "ready") {
        throw new Error(`expected ready, got ${model.reason}`);
      }
      expect(model.hostRef).toBe(harness.hostRef);
      expect(model.snapshotRef).toBe(harness.snapshotRef);
      expect(model.runs.map((run) => run.runRef)).toContain("run.full-auto.run-01");
      expect(model.accounts.length).toBeGreaterThan(0);
    } finally {
      device.close();
    }
  });

  test("a host that is running nothing publishes an empty view, not an absent one", async () => {
    // omega#49 forbids inventing a run. A host with no Full Auto work must
    // still say so out loud: silence is read as "not paired", which is the
    // exact lie this issue exists to remove.
    const harness = await pairedHarness(`empty-${Date.now()}`);
    await publishHostRumor(harness, deliveredHostAdjunct(harness));
    await publishHostRumor(
      harness,
      deliveredFullAutoAdjunct(harness, {
        runs: [],
        accounts: [],
        handoffs: [],
        evidence: [],
      }),
    );
    const device = await startDeviceClient(harness);
    try {
      await device.awaitSnapshot(
        (snapshot) =>
          recordsWithSchema(snapshot, "openagents.omega.issue31.fullauto.v1").length === 1,
        "the empty detail projection to arrive",
      );
      const model = issue31FullAutoProjectionFromSnapshot(
        readerSnapshot(harness, device.snapshot()),
        harness.now,
      );
      expect(model.state).toBe("ready");
      if (model.state !== "ready") throw new Error("unreachable");
      expect(model.runs).toEqual([]);
      expect(model.accounts).toEqual([]);
    } finally {
      device.close();
    }
  });

  test("an adjunct that states no delivery binding is not an owner-private record", async () => {
    // The bare omega#47 document — exactly what the host builds for its own
    // panels. It names no device, so nothing binds it to this one, so the
    // device must refuse it rather than render another machine's state.
    const harness = await pairedHarness(`bare-${Date.now()}`);
    const bare = {
      ...workroomFixture("host.v1.canonical"),
      hostRef: harness.hostRef,
      snapshotRef: harness.snapshotRef,
    };
    // The shipped envelope builder refuses to seal it in the first place, so a
    // well-behaved host cannot send this by accident.
    await expect(publishHostRumor(harness, bare)).rejects.toThrow(
      /delivered without stating/,
    );
    const wrapId = await publishRawHostRumor(harness, bare);
    const device = await startDeviceClient(harness);
    try {
      await device.awaitSnapshot(
        (snapshot) => snapshot.relays.some((row) => row.state === "live"),
        "the device to finish its replay",
      );
      const snapshot = device.snapshot();
      expect(snapshot.confirmedEvents.some((row) => row.event.id === wrapId)).toBe(false);
      expect(snapshot.relays.some((row) => row.rejectedEventCount > 0)).toBe(true);
      expect(
        issue31FullAutoProjectionFromSnapshot(readerSnapshot(harness, snapshot), harness.now),
      ).toMatchObject({ state: "unavailable", reason: "no_host_snapshot" });
    } finally {
      device.close();
    }
  });

  test("an adjunct addressed to another device never reaches this one", async () => {
    const harness = await pairedHarness(`other-device-${Date.now()}`);
    const otherDevice = getPublicKey(generateSecretKey());
    // Sealed and encrypted to THIS device, but claiming to be for another. The
    // gift wrap is unwrappable here; the identity check is what refuses it.
    const misaddressed = deliveredHostAdjunct(harness, {
      ...delivery(harness, "host_snapshot", { devicePublicKeyHex: otherDevice }),
    });
    await expect(publishHostRumor(harness, misaddressed)).rejects.toThrow(
      /recipient does not match/,
    );
    const wrapId = await publishRawHostRumor(harness, misaddressed);
    const device = await startDeviceClient(harness);
    try {
      await device.awaitSnapshot(
        (snapshot) => snapshot.relays.some((row) => row.state === "live"),
        "the device to finish its replay",
      );
      const snapshot = device.snapshot();
      expect(snapshot.confirmedEvents.some((row) => row.event.id === wrapId)).toBe(false);
      expect(
        issue31FullAutoProjectionFromSnapshot(readerSnapshot(harness, snapshot), harness.now),
      ).toMatchObject({ state: "unavailable", reason: "no_host_snapshot" });
    } finally {
      device.close();
    }
  });

  test("an adjunct delivered under a grant this device does not hold is withheld", async () => {
    // The host key is admitted, the device key is right, and the seal is this
    // host's. Only the grant is wrong — a snapshot minted under some other
    // device's entitlement, or under a pairing this device never made.
    const harness = await pairedHarness(`foreign-grant-${Date.now()}`);
    await publishHostRumor(
      harness,
      deliveredHostAdjunct(harness, {
        ...delivery(harness, "host_snapshot", { grantRef: "grant.omega.someone-else" }),
      }),
    );
    await publishHostRumor(harness, deliveredFullAutoAdjunct(harness));
    const device = await startDeviceClient(harness);
    try {
      await device.awaitSnapshot(
        (snapshot) =>
          recordsWithSchema(snapshot, "openagents.omega.issue31.fullauto.v1").length === 1,
        "the detail projection to arrive",
      );
      const snapshot = device.snapshot();
      expect(recordsWithSchema(snapshot, "openagents.omega.issue31.host.v1")).toEqual([]);
      // No host snapshot means no binding, and a detail with nothing to bind to
      // is reported as absence rather than drawn on its own authority.
      expect(
        issue31FullAutoProjectionFromSnapshot(readerSnapshot(harness, snapshot), harness.now),
      ).toMatchObject({ state: "unavailable", reason: "no_host_snapshot" });
    } finally {
      device.close();
    }
  });

  test("a host relabelling its snapshot with another host's reference is withheld", async () => {
    // The seal proves who signed; it cannot prove which host the body is about.
    // The pairing chain is the only statement that relates a host key to a host
    // reference, so it is what refuses this.
    const harness = await pairedHarness(`foreign-hostref-${Date.now()}`);
    await publishHostRumor(
      harness,
      deliveredHostAdjunct(harness, { hostRef: "omega.host.some-other-machine" }),
    );
    await publishHostRumor(harness, deliveredFullAutoAdjunct(harness));
    const device = await startDeviceClient(harness);
    try {
      await device.awaitSnapshot(
        (snapshot) =>
          recordsWithSchema(snapshot, "openagents.omega.issue31.fullauto.v1").length === 1,
        "the detail projection to arrive",
      );
      const snapshot = device.snapshot();
      expect(recordsWithSchema(snapshot, "openagents.omega.issue31.host.v1")).toEqual([]);
    } finally {
      device.close();
    }
  });
});

/**
 * The wire-up, over the same real relay (omega#97).
 *
 * The tests above prove the *reader*: hand it a delivered adjunct and it
 * decides correctly. That was never the gap. `readIssue31FullAutoProjection`
 * was called once in `src/`, with `(null, null)`; there was no setter for
 * `issue31FullAuto`; and `projectIssue31WorkroomReadModel` accepted a
 * `hostAdjunct` that `home-screen.tsx` passed zero times. A real host could
 * publish a real reading to a real relay and the phone still rendered
 * `reason.issue31.source_not_connected` on all three host-authority rows.
 *
 * These drive `projectIssue31Workroom` — the exact function the screen calls —
 * on records that crossed the relay, and then push its output through the
 * shipped program into `renderContentView`. Nothing here re-implements the
 * screen; a test that did would prove the test.
 */
const clientSnapshotFor = (
  harness: Harness,
  snapshot: Issue31NostrClientSnapshot,
): Issue31NostrClientSnapshot => ({
  ...snapshot,
  confirmedEvents: [...harness.deviceLocalPairing, ...snapshot.confirmedEvents],
});

const hostRowsOf = (workroom: ReturnType<typeof projectIssue31Workroom>["workroom"]) =>
  workroom.rows.filter((row) => row.expectedAuthority === "omega_host_adjunct");

const settle = Effect.gen(function* () {
  yield* Effect.yieldNow;
  yield* Effect.yieldNow;
});

/** The Workroom exactly as a device would draw it from this projection. */
const renderWorkroom = async (
  projection: ReturnType<typeof projectIssue31Workroom>,
): Promise<string> => {
  const program = buildHomeProgram();
  program.workroom.open();
  program.workroom.setReadModel(projection.workroom);
  program.workroom.setFullAutoReadModel(projection.fullAuto);
  await Effect.runPromise(settle);
  const state = await Effect.runPromise(
    Effect.map(Stream.runHead(program.stateChanges), (option) => {
      if (option._tag !== "Some") throw new Error("expected state");
      return option.value;
    }),
  );
  return JSON.stringify(renderContentView(state));
};

describe("the Workroom the owner sees, from what the host published", () => {
  test("the three host rows carry the host's own state instead of source_not_connected", async () => {
    const harness = await pairedHarness(`rows-${Date.now()}`);
    await publishHostRumor(harness, deliveredHostAdjunct(harness));
    await publishHostRumor(harness, deliveredFullAutoAdjunct(harness));
    const device = await startDeviceClient(harness);
    try {
      await device.awaitSnapshot(
        (snapshot) =>
          recordsWithSchema(snapshot, "openagents.omega.issue31.host.v1").length === 1 &&
          recordsWithSchema(snapshot, "openagents.omega.issue31.fullauto.v1").length === 1,
        "both adjuncts to arrive over the wire",
      );
      const projection = projectIssue31Workroom(
        clientSnapshotFor(harness, device.snapshot()),
        harness.now,
      );

      // The defect, stated as an assertion: not one of the three may still be
      // reporting that nothing is connected.
      for (const row of hostRowsOf(projection.workroom)) {
        expect(row.source.reasonRef ?? "").not.toContain("source_not_connected");
        expect(row.hostObservation?.hostRef).toBe(harness.hostRef);
        expect(row.hostObservation?.snapshotRef).toBe(harness.snapshotRef);
      }
      // And each carries the state THIS host published, not a uniform "ready".
      // The host's own `full_auto_runs` projection is a partial, stale gap with
      // a pause still pending; reporting it as ready would be the same class of
      // lie the row used to tell.
      const byId = (id: string) => projection.workroom.rows.find((row) => row.id === id);
      expect(byId("full_auto")?.source).toMatchObject({
        authority: "omega_host_adjunct",
        status: "gap",
        freshness: "stale",
        // The host's own capability name, not the device's row id: the reason
        // states what the host said about `full_auto_runs`.
        reasonRef: "reason.issue31.host_gap:full_auto_runs",
      });
      expect(byId("full_auto")?.source.actionState.kind).toBe("pending");
      expect(byId("provider_accounts")?.source.status).toBe("ready");
      expect(byId("evidence_chain")?.source.status).toBe("ready");

      // The section below the rows, from the same binding.
      expect(projection.hostBinding.state).toBe("bound");
      if (projection.fullAuto.state !== "ready") {
        throw new Error(`expected ready, got ${projection.fullAuto.reason}`);
      }
      const run = projection.fullAuto.runs.find((row) => row.runRef === "run.full-auto.run-01");
      if (run === undefined) throw new Error("the host's run must reach the read model");

      const view = await renderWorkroom(projection);
      // Objective, lane, lifecycle, live work, the host's exact measured
      // unattended duration, and the account-to-lane relation — the whole of
      // what omega#97 asks a paired device to render.
      expect(view).toContain(run.objective);
      expect(view).toContain(run.laneRef);
      expect(view).toContain(run.lifecycle);
      expect(view).toContain(run.liveWorkRef ?? "no unit reported");
      const seconds = Math.floor(run.unattendedMs / 1000);
      const exact = `${String(Math.floor(seconds / 3600)).padStart(2, "0")}:${String(
        Math.floor((seconds % 3600) / 60),
      ).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")} unattended`;
      expect(view).toContain(exact);
      // A lane is not an account, so the relation is stated rather than implied
      // by putting the two on one row.
      const account = projection.fullAuto.accounts[0];
      if (account === undefined) throw new Error("the host published a provider account");
      expect(view).toContain(
        `Account ${account.accountRef} serves lane ${account.laneRef}`,
      );
      expect(view).not.toContain("This device is paired, but your Omega host has not published");
    } finally {
      device.close();
    }
  });

  test("a paired host that has published nothing says so, and does not say nothing arrived", async () => {
    // `source_not_connected` means no host reached this device. Under a live
    // grant with a silent host that is false, and it is the sentence the rows
    // used to show for every state at once.
    const harness = await pairedHarness(`silent-${Date.now()}`);
    const device = await startDeviceClient(harness);
    try {
      // The grant itself is host-authored and crosses the relay, so a device
      // that has not received it yet is honestly unpaired. Wait for the grant
      // and publish no snapshot: that is the state under test.
      await device.awaitSnapshot(
        (snapshot) =>
          projectIssue31Workroom(clientSnapshotFor(harness, snapshot), harness.now).hostBinding
            .state !== "unpaired",
        "the host-authored grant to arrive over the wire",
      );
      const projection = projectIssue31Workroom(
        clientSnapshotFor(harness, device.snapshot()),
        harness.now,
      );
      expect(projection.hostBinding.state).toBe("absent");
      for (const row of hostRowsOf(projection.workroom)) {
        expect(row.source.reasonRef).toBe(`reason.issue31.no_host_snapshot:${row.id}`);
        expect(row.source.status).toBe("unavailable");
      }
      expect(projection.fullAuto).toMatchObject({ reason: "no_host_snapshot" });
    } finally {
      device.close();
    }
  });

  test("a malformed host record is refused at the envelope and takes nothing else down", async () => {
    // One malformed record blanking every surface on the device is a defect
    // this contract has already paid for once, so this asserts the blast radius
    // rather than only the refusal.
    //
    // Where the refusal happens is worth stating plainly: `host.v1` bodies are
    // decoded inside `unwrapIssue31PrivateGiftWrap`, so a malformed one never
    // becomes a confirmed record at all. On the shipped client path the device
    // therefore reads `no_host_snapshot` — "this host has published nothing I
    // can read" — and not `host_projection_unreadable`. The read model keeps a
    // distinct gap for the unreadable case (proven against a snapshot that
    // carries such a record directly, in
    // `issue31-full-auto-projection-source.test.ts`); it is defence in depth
    // behind an envelope that already refuses, not the wire behaviour.
    const harness = await pairedHarness(`unreadable-${Date.now()}`);
    await publishHostRumor(harness, deliveredFullAutoAdjunct(harness));
    await publishRawHostRumor(harness, {
      ...deliveredHostAdjunct(harness),
      projections: "not an array of projections",
    });
    const device = await startDeviceClient(harness);
    try {
      await device.awaitSnapshot(
        (snapshot) =>
          snapshot.relays.some((row) => row.rejectedEventCount > 0) &&
          recordsWithSchema(snapshot, "openagents.omega.issue31.fullauto.v1").length === 1,
        "the relay to serve both the refused snapshot and the readable detail",
      );
      const snapshot = device.snapshot();
      // Refused at the envelope: it is not a confirmed record in any form.
      expect(recordsWithSchema(snapshot, "openagents.omega.issue31.host.v1")).toEqual([]);

      const projection = projectIssue31Workroom(clientSnapshotFor(harness, snapshot), harness.now);
      // The rest of the room is still projected: every row, a coverage summary,
      // and an owner-private model — not one blanket refusal.
      expect(projection.workroom.rows).toHaveLength(ISSUE31_CAPABILITY_DESCRIPTORS.length);
      expect(projection.workroom.coverage.total).toBe(ISSUE31_CAPABILITY_DESCRIPTORS.length);
      expect(projection.workroom.ownerPrivate).not.toBeUndefined();
      // The three host rows say the host published nothing readable. They do
      // not say nothing arrived, and they do not claim a reading.
      for (const row of hostRowsOf(projection.workroom)) {
        expect(row.source.reasonRef).toBe(`reason.issue31.no_host_snapshot:${row.id}`);
        expect(row.source.status).toBe("unavailable");
      }
      // A signed-Nostr row is not made unavailable by the state of the host.
      for (const row of projection.workroom.rows) {
        if (row.expectedAuthority === "omega_host_adjunct") continue;
        expect(row.source.reasonRef ?? "").not.toContain("no_host_snapshot");
      }
      // A detail with no snapshot to bind to is withheld rather than drawn on
      // its own authority — the payload does not get to select itself into view.
      expect(projection.fullAuto).toMatchObject({ reason: "no_host_snapshot" });
    } finally {
      device.close();
    }
  });

  test("an unpaired device says nothing arrived, and never claims a host went silent", async () => {
    const harness = await pairedHarness(`unpaired-${Date.now()}`);
    await publishHostRumor(harness, deliveredHostAdjunct(harness));
    const device = await startDeviceClient(harness);
    try {
      await device.awaitSnapshot(
        (snapshot) => recordsWithSchema(snapshot, "openagents.omega.issue31.host.v1").length === 1,
        "the host snapshot to arrive",
      );
      // Same wire, same records — only the device's own pairing half withheld,
      // which is what an unpaired device actually holds.
      const projection = projectIssue31Workroom(device.snapshot(), harness.now);
      expect(projection.hostBinding.state).toBe("unpaired");
      for (const row of hostRowsOf(projection.workroom)) {
        expect(row.source.reasonRef).toBe(`reason.issue31.source_not_connected:${row.id}`);
      }
      expect(projection.fullAuto).toMatchObject({ reason: "no_host_projection" });
    } finally {
      device.close();
    }
  });

  test("a broken hop renders as unavailable with its reason, never as proof", async () => {
    // omega#43's chain is refused hop by hop. `hop_missing`, `hop_mismatched`,
    // `hop_private` and `self_reported` are four different facts, and the one
    // thing none of them may become is a chain the owner reads as complete.
    const harness = await pairedHarness(`hops-${Date.now()}`);
    const canonical = workroomFixture("fullauto.v1.canonical");
    const runs = canonical["runs"] as ReadonlyArray<Record<string, unknown>>;
    const broken = ["hop_mismatched", "hop_private", "self_reported"] as const;
    await publishHostRumor(harness, deliveredHostAdjunct(harness));
    await publishHostRumor(
      harness,
      deliveredFullAutoAdjunct(harness, {
        evidence: runs.map((run, index) => ({
          completeness: "unavailable",
          runRef: run["runRef"],
          reasonClass: broken[index % broken.length],
          brokenAt: "host_verification",
        })),
      }),
    );
    const device = await startDeviceClient(harness);
    try {
      await device.awaitSnapshot(
        (snapshot) =>
          recordsWithSchema(snapshot, "openagents.omega.issue31.fullauto.v1").length === 1,
        "the detail with refused chains to arrive",
      );
      const projection = projectIssue31Workroom(
        clientSnapshotFor(harness, device.snapshot()),
        harness.now,
      );
      if (projection.fullAuto.state !== "ready") {
        throw new Error(`expected ready, got ${projection.fullAuto.reason}`);
      }
      const classes = projection.fullAuto.runs.map((run) =>
        run.evidence.state === "unavailable" ? run.evidence.reasonClass : "complete",
      );
      expect(classes).not.toContain("complete");
      expect(new Set(classes).size).toBeGreaterThan(1);

      const view = await renderWorkroom(projection);
      // The refusal is legible, names which hop broke, and the run beside it is
      // still rendered — a refused chain does not delete its own run.
      expect(view).toContain("Evidence unavailable");
      expect(view).toContain("first broken at host_verification");
      expect(view).not.toContain("Evidence complete");
      for (const reasonClass of new Set(classes)) {
        expect(view).toContain(
          {
            hop_mismatched: "two records disagree about this run",
            hop_private: "a step cannot be shown on this device",
            self_reported: "the run reported its own success",
          }[reasonClass as "hop_mismatched" | "hop_private" | "self_reported"],
        );
      }
    } finally {
      device.close();
    }
  });
});
