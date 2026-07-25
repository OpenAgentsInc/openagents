import { LocalKeySigner } from "nostr-effect/identity";
import { getEventHash } from "nostr-effect/pure";
import { describe, expect, test } from "vite-plus/test";

import {
  ISSUE31_COMMAND_SCHEMA,
  ISSUE31_HOST_ANNOUNCEMENT_KIND,
  ISSUE31_HOST_ANNOUNCEMENT_SCHEMA,
  ISSUE31_PAIRING_SCHEMA,
  ISSUE31_PRIVATE_GIFT_WRAP_KIND,
  ISSUE31_PRIVATE_RUMOR_KIND,
  ISSUE31_PRIVATE_SEAL_KIND,
  createIssue31PrivateGiftWrap,
  decodeIssue31PairingRecord,
  type Issue31SignedNostrEvent,
} from "@openagentsinc/sarah/issue31-nostr";

import {
  ISSUE31_DEVICE_KEYCHAIN_SERVICE,
  ISSUE31_DEVICE_KEY_STORE_KEY,
  Issue31DeviceKeyVaultError,
  clearIssue31DeviceIdentity,
  openIssue31DeviceIdentity,
  type Issue31SecureStore,
  type Issue31SecureStoreOptions,
} from "../src/workroom/issue31-device-key-vault";
import {
  ISSUE31_OWNER_PRIVATE_KINDS,
  createIssue31NostrClient,
  type Issue31NostrRoom,
  type Issue31OutboundEventStore,
  type Issue31RelayCursor,
  type Issue31RelayCursorStore,
  type Issue31WebSocketLike,
} from "../src/workroom/issue31-nostr-client";
import {
  issue31MobileCommandIntentForGrant,
  issue31MobileOwnerPrivateScope,
} from "../src/workroom/issue31-mobile-nostr-runtime";
import { issue31PersistedPairingEventsForRequeue } from "../src/workroom/issue31-outbound-event-store";

const deviceSecret = Uint8Array.from({ length: 32 }, (_, index) => index + 1);
const hostSecret = Uint8Array.from({ length: 32 }, (_, index) => index + 33);
const ephemeralOne = Uint8Array.from({ length: 32 }, (_, index) => index + 65);
const ephemeralTwo = Uint8Array.from({ length: 32 }, (_, index) => index + 97);

const settleCallbacks = (): Promise<void> =>
  new Promise((resolve) => {
    setImmediate(resolve);
  });

const fakeStore = (
  withDeviceOnly = true,
): Readonly<{
  store: Issue31SecureStore;
  values: Map<string, string>;
  options: Issue31SecureStoreOptions[];
}> => {
  const values = new Map<string, string>();
  const options: Issue31SecureStoreOptions[] = [];
  const deviceOnly = { policy: "this-device-only" };
  return {
    values,
    options,
    store: {
      ...(withDeviceOnly ? { AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: deviceOnly } : {}),
      getItemAsync: async (key, nextOptions) => {
        if (nextOptions !== undefined) options.push(nextOptions);
        return values.get(key) ?? null;
      },
      setItemAsync: async (key, value, nextOptions) => {
        if (nextOptions !== undefined) options.push(nextOptions);
        values.set(key, value);
      },
      deleteItemAsync: async (key, nextOptions) => {
        if (nextOptions !== undefined) options.push(nextOptions);
        values.delete(key);
      },
    },
  };
};

class ScriptedWebSocket implements Issue31WebSocketLike {
  static instances: ScriptedWebSocket[] = [];
  onopen: ((event: unknown) => void) | null = null;
  onmessage: ((event: Readonly<{ data: unknown }>) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onclose: ((event: Readonly<{ code?: number; reason?: string }>) => void) | null = null;
  readonly sent: unknown[] = [];
  readonly closes: Array<Readonly<{ code?: number; reason?: string }>> = [];

  constructor(readonly url: string) {
    ScriptedWebSocket.instances.push(this);
  }

  send(data: string): void {
    this.sent.push(JSON.parse(data) as unknown);
  }

  close(code?: number, reason?: string): void {
    this.closes.push({
      ...(code === undefined ? {} : { code }),
      ...(reason === undefined ? {} : { reason }),
    });
  }

  open(): void {
    this.onopen?.({});
  }

  message(frame: unknown): void {
    this.onmessage?.({ data: JSON.stringify(frame) });
  }

  remoteClose(): void {
    this.onclose?.({ code: 1006, reason: "relay lost" });
  }
}

const cursorStore = (): Readonly<{
  store: Issue31RelayCursorStore;
  saved: Array<
    Readonly<{
      relayUrl: string;
      room: Issue31NostrRoom;
      cursor: Issue31RelayCursor;
    }>
  >;
}> => {
  const initial: Readonly<Record<Issue31NostrRoom, Issue31RelayCursor>> = {
    discovery: { since: 10, eventIdsAtSince: [] },
    owner_private: { since: 20, eventIdsAtSince: [] },
    community: { since: 30, eventIdsAtSince: [] },
  };
  const saved: Array<
    Readonly<{
      relayUrl: string;
      room: Issue31NostrRoom;
      cursor: Issue31RelayCursor;
    }>
  > = [];
  return {
    saved,
    store: {
      load: async (_relayUrl, room) => initial[room],
      save: async (relayUrl, room, cursor) => {
        saved.push({ relayUrl, room, cursor });
      },
    },
  };
};

describe("Issue 31 mobile device key vault", () => {
  test("keeps the device key in this-device-only custody and exports signer operations only", async () => {
    const firstStore = fakeStore();
    const first = await openIssue31DeviceIdentity({
      store: firstStore.store,
      randomBytes: async () => new Uint8Array(deviceSecret),
    });
    const second = await openIssue31DeviceIdentity({
      store: firstStore.store,
      randomBytes: async () => {
        throw new Error("must not regenerate");
      },
    });
    expect(first.publicKeyHex).toBe(second.publicKeyHex);
    expect(Object.keys(first)).toEqual(["publicKeyHex", "npub", "signer", "close"]);
    expect(Object.keys(first.signer).sort()).toEqual([
      "getPublicKey",
      "nip44Decrypt",
      "nip44Encrypt",
      "signEvent",
    ]);
    expect(firstStore.values.has(ISSUE31_DEVICE_KEY_STORE_KEY)).toBe(true);
    expect(
      firstStore.options.every(
        (options) => options.keychainService === ISSUE31_DEVICE_KEYCHAIN_SERVICE,
      ),
    ).toBe(true);
    expect(firstStore.options.every((options) => options.keychainAccessible !== undefined)).toBe(
      true,
    );
    first.close();
    second.close();
    await clearIssue31DeviceIdentity(firstStore.store);
    expect(firstStore.values.size).toBe(0);
  });

  test("fails closed when the native store cannot guarantee this-device-only custody", async () => {
    await expect(
      openIssue31DeviceIdentity({
        store: fakeStore(false).store,
        randomBytes: async () => new Uint8Array(deviceSecret),
      }),
    ).rejects.toMatchObject({
      reason: "secure_store_unavailable",
    } satisfies Partial<Issue31DeviceKeyVaultError>);
  });
});

describe("Issue 31 mobile Nostr client", () => {
  test("clears Sarah author and owner recipient scope for an expired discovery", () => {
    const host = {
      hostRef: "omega.host.local",
      hostPublicKeyHex: "1".repeat(64),
      sarahPublicKeyHex: "3".repeat(64),
      displayName: "Local Omega",
      hostFingerprint: "111111111111…11111111",
      sarahFingerprint: "333333333333…33333333",
      generation: 1,
      expiresAt: 1_000,
    };
    const grant = {
      grantRef: "grant.omega.device_1",
      hostRef: host.hostRef,
      hostPublicKeyHex: host.hostPublicKeyHex,
      sarahPublicKeyHex: host.sarahPublicKeyHex,
      devicePublicKeyHex: "2".repeat(64),
      generation: 1,
      status: "active" as const,
      scopes: ["observe_issue31"],
      expiresAt: 2_000,
      issuedAt: 900,
      sourceEventId: "4".repeat(64),
    };
    expect(issue31MobileOwnerPrivateScope(host, grant, 999)).toEqual({
      selectedHostPublicKeys: [host.hostPublicKeyHex],
      ownerAuthors: [host.sarahPublicKeyHex],
      ownerRecipientPublicKeys: [host.hostPublicKeyHex],
    });
    expect(issue31MobileOwnerPrivateScope(host, null, 999)).toEqual({
      selectedHostPublicKeys: [host.hostPublicKeyHex],
      ownerAuthors: [],
      ownerRecipientPublicKeys: [],
    });
    expect(
      issue31MobileOwnerPrivateScope({ ...host, sarahPublicKeyHex: "5".repeat(64) }, grant, 999),
    ).toEqual({
      selectedHostPublicKeys: [host.hostPublicKeyHex],
      ownerAuthors: [],
      ownerRecipientPublicKeys: [],
    });
    expect(issue31MobileOwnerPrivateScope(host, grant, 1_000)).toEqual({
      selectedHostPublicKeys: [],
      ownerAuthors: [],
      ownerRecipientPublicKeys: [],
    });
    expect(issue31MobileOwnerPrivateScope(undefined, grant, 999)).toEqual({
      selectedHostPublicKeys: [],
      ownerAuthors: [],
      ownerRecipientPublicKeys: [],
    });
  });

  test("binds command intents to one active grant generation and idempotency reference", () => {
    const intent = issue31MobileCommandIntentForGrant({
      grant: {
        grantRef: "grant.omega.device_1",
        hostRef: "omega.host.local",
        hostPublicKeyHex: "1".repeat(64),
        sarahPublicKeyHex: "3".repeat(64),
        devicePublicKeyHex: "2".repeat(64),
        generation: 7,
        status: "active",
        scopes: ["control_full_auto"],
        expiresAt: 2_000,
        issuedAt: 1_000,
        sourceEventId: "4".repeat(64),
      },
      devicePublicKeyHex: "2".repeat(64),
      actionRef: "action.omega.full_auto.stop",
      idempotencyRef: "idempotency.omega.stop_7",
      argumentsRef: "arguments.omega.none",
      issuedAt: 1_200,
      expiresAt: 1_500,
    });
    expect(intent).toMatchObject({
      recordType: "command_intent",
      grantRef: "grant.omega.device_1",
      idempotencyRef: "idempotency.omega.stop_7",
      expectedGeneration: 7,
    });
  });

  test("requeues the exact persisted outgoing pairing wrap after a crash window", async () => {
    const device = LocalKeySigner.fromPrivateKey(deviceSecret);
    const host = LocalKeySigner.fromPrivateKey(hostSecret);
    const request = decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "pairing_request",
      hostRef: "omega.host.local",
      hostPublicKeyHex: host.publicKey,
      devicePublicKeyHex: device.publicKey,
      issuedAt: 1_000,
      pairingRequestRef: "pairing.request.device_1",
      requestedScopes: ["observe_issue31"],
      expiresAt: 1_300,
    });
    const envelope = await createIssue31PrivateGiftWrap({
      signer: device,
      recipientPublicKeyHex: host.publicKey,
      record: request,
      randomSecretKey: () => new Uint8Array(ephemeralOne),
      createdAt: 1_000,
      sealCreatedAt: 950,
      wrapCreatedAt: 900,
    });
    const stored = {
      canonicalRecordId: "a".repeat(64),
      event: envelope,
      record: request,
    };
    expect(
      issue31PersistedPairingEventsForRequeue([stored], {
        selectedHostPublicKeyHex: host.publicKey,
        devicePublicKeyHex: device.publicKey,
        admittedHostPublicKeys: new Set([host.publicKey]),
      }),
    ).toEqual([envelope]);
    expect(
      issue31PersistedPairingEventsForRequeue([stored], {
        selectedHostPublicKeyHex: host.publicKey,
        devicePublicKeyHex: device.publicKey,
        admittedHostPublicKeys: new Set<string>(),
      }),
    ).toEqual([]);
    host.dispose();
    device.dispose();
  });

  test("rejects credential-bearing relay query strings", () => {
    const device = LocalKeySigner.fromPrivateKey(deviceSecret);
    expect(() =>
      createIssue31NostrClient({
        relayUrls: ["wss://relay.example.com/?token=private"],
        signer: device,
        webSocket: ScriptedWebSocket,
      }),
    ).toThrow(/relay URL is unsafe/);
    device.dispose();
  });

  test("uses isolated cursors and bounded subscriptions, waits for every EOSE, and supports NIP-42", async () => {
    ScriptedWebSocket.instances = [];
    const device = LocalKeySigner.fromPrivateKey(deviceSecret);
    const host = LocalKeySigner.fromPrivateKey(hostSecret);
    const cursors = cursorStore();
    const timers: Array<Readonly<{ callback: () => void; delayMs: number }>> = [];
    const client = createIssue31NostrClient({
      relayUrls: ["wss://relay.example.com"],
      signer: device,
      webSocket: ScriptedWebSocket,
      cursorStore: cursors.store,
      admittedHostPublicKeys: [host.publicKey],
      ownerAuthors: [host.publicKey],
      ownerRecipientPublicKeys: [host.publicKey],
      communityAuthors: [host.publicKey],
      communityGroupIds: ["omega-community"],
      setTimer: (callback, delayMs) => timers.push({ callback, delayMs }),
    });
    await client.start();
    const socket = ScriptedWebSocket.instances[0];
    if (socket === undefined) throw new Error("expected socket");
    socket.open();
    await settleCallbacks();
    const requests = socket.sent.filter(
      (frame): frame is unknown[] => Array.isArray(frame) && frame[0] === "REQ",
    );
    expect(requests).toHaveLength(5);
    expect(requests.filter((frame) => String(frame[1]).includes("owner_private"))).toHaveLength(2);
    expect(
      requests.find((frame) => String(frame[1]).includes("owner_private-0"))?.[2],
    ).toMatchObject({
      kinds: [1059],
      since: 19,
      "#p": [device.publicKey],
    });
    expect(
      requests.find((frame) => String(frame[1]).includes("owner_private-1"))?.[2],
    ).toMatchObject({
      authors: [host.publicKey],
      "#p": [host.publicKey],
    });
    expect(requests.find((frame) => String(frame[1]).includes("community-0"))?.[2]).toMatchObject({
      kinds: [9],
      since: 29,
      "#h": ["omega-community"],
    });
    const discoveryRequest = requests.find((frame) => String(frame[1]).includes("discovery-0"));
    if (discoveryRequest === undefined) throw new Error("expected discovery request");
    socket.message(["EOSE", discoveryRequest[1]]);
    expect(client.snapshot().relays[0]?.state).toBe("replaying");
    for (const request of requests.slice(1)) socket.message(["EOSE", request[1]]);
    await settleCallbacks();
    expect(client.snapshot().relays[0]?.state).toBe("live");

    socket.message(["AUTH", "challenge-1"]);
    await settleCallbacks();
    const authFrame = socket.sent.find(
      (frame): frame is unknown[] => Array.isArray(frame) && frame[0] === "AUTH",
    );
    expect((authFrame?.[1] as Readonly<{ kind?: number }> | undefined)?.kind).toBe(22_242);
    const authId = (authFrame?.[1] as Readonly<{ id?: string }> | undefined)?.id;
    socket.message(["OK", authId, true, "authenticated"]);
    await settleCallbacks();
    expect(socket.sent.filter((frame) => Array.isArray(frame) && frame[0] === "REQ")).toHaveLength(
      10,
    );
    const closeFrames = socket.sent.filter(
      (frame): frame is unknown[] => Array.isArray(frame) && frame[0] === "CLOSE",
    );
    expect(closeFrames.map((frame) => frame[1])).toEqual(requests.map((frame) => frame[1]));
    socket.message(["EVENT", requests[0]?.[1], null]);
    socket.message(["EOSE", requests[0]?.[1]]);
    socket.message(["CLOSED", requests[0]?.[1], "closed after auth"]);
    await settleCallbacks();
    expect(socket.closes).toHaveLength(0);
    const requestsAfterAuth = socket.sent.filter(
      (frame): frame is unknown[] => Array.isArray(frame) && frame[0] === "REQ",
    );
    const retiredOwnerRequest = requestsAfterAuth.find((frame) =>
      String(frame[1]).includes("issue31-2-owner_private-1"),
    );
    if (retiredOwnerRequest === undefined) throw new Error("expected authenticated owner request");
    await client.updateSubscriptionScope({
      selectedHostPublicKeys: [],
      ownerAuthors: [],
      ownerRecipientPublicKeys: [],
    });
    const requestsAfterClear = socket.sent
      .filter((frame): frame is unknown[] => Array.isArray(frame) && frame[0] === "REQ")
      .slice(10);
    expect(
      requestsAfterClear.filter((frame) => String(frame[1]).includes("owner_private")),
    ).toHaveLength(1);
    const staleOwnerKind = ISSUE31_OWNER_PRIVATE_KINDS.find(
      (kind) => kind !== ISSUE31_PRIVATE_GIFT_WRAP_KIND,
    );
    if (staleOwnerKind === undefined) throw new Error("expected direct owner kind");
    const staleOwnerEvent = await host.signEvent({
      kind: staleOwnerKind,
      tags: [["p", host.publicKey]],
      content: "stale selected owner",
    });
    socket.message(["EVENT", retiredOwnerRequest[1], staleOwnerEvent]);
    await settleCallbacks();
    expect(client.snapshot()).toMatchObject({
      selectedHostPublicKeys: [],
      ownerPrivateAuthors: [],
      ownerRecipientPublicKeys: [],
      confirmedEvents: [],
    });
    expect(timers.some((timer) => timer.delayMs === 10_000)).toBe(true);
    timers[0]?.callback();
    expect(socket.closes.some((close) => close.reason === "issue31_eose_timeout")).toBe(false);
    client.close();
    host.dispose();
    device.dispose();
  });

  test("deduplicates device-targeted copies by the inner rumor and never treats OK as completion", async () => {
    ScriptedWebSocket.instances = [];
    const device = LocalKeySigner.fromPrivateKey(deviceSecret);
    const host = LocalKeySigner.fromPrivateKey(hostSecret);
    const client = createIssue31NostrClient({
      relayUrls: ["wss://relay.example.com"],
      signer: device,
      webSocket: ScriptedWebSocket,
      admittedHostPublicKeys: [host.publicKey],
      selectedHostPublicKeys: [host.publicKey],
      ownerAuthors: ["3".repeat(64)],
    });
    await client.start();
    const socket = ScriptedWebSocket.instances[0];
    if (socket === undefined) throw new Error("expected socket");
    socket.open();
    await settleCallbacks();
    for (const frame of socket.sent.filter((value) => Array.isArray(value) && value[0] === "REQ")) {
      socket.message(["EOSE", (frame as unknown[])[1]]);
    }

    const record = decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "scoped_grant",
      hostRef: "omega.host.local",
      hostPublicKeyHex: host.publicKey,
      sarahPublicKeyHex: "3".repeat(64),
      devicePublicKeyHex: device.publicKey,
      issuedAt: 1_000,
      pairingResponseEventId: "a".repeat(64),
      grantRef: "grant.omega.device_1",
      generation: 1,
      scopes: ["observe_issue31"],
      expiresAt: 2_000,
    });
    const first = await createIssue31PrivateGiftWrap({
      signer: host,
      recipientPublicKeyHex: device.publicKey,
      record,
      randomSecretKey: () => new Uint8Array(ephemeralOne),
      createdAt: 1_100,
      sealCreatedAt: 1_000,
      wrapCreatedAt: 900,
    });
    const second = await createIssue31PrivateGiftWrap({
      signer: host,
      recipientPublicKeyHex: device.publicKey,
      record,
      randomSecretKey: () => new Uint8Array(ephemeralTwo),
      createdAt: 1_100,
      sealCreatedAt: 1_000,
      wrapCreatedAt: 800,
    });
    const ownerRequest = socket.sent.find(
      (value): value is unknown[] =>
        Array.isArray(value) && value[0] === "REQ" && String(value[1]).includes("owner_private-0"),
    );
    if (ownerRequest === undefined) throw new Error("expected owner request");
    socket.message(["EVENT", ownerRequest[1], first]);
    socket.message(["EVENT", ownerRequest[1], second]);
    await settleCallbacks();
    expect(client.snapshot().confirmedEvents).toHaveLength(1);
    expect(client.snapshot().confirmedEvents[0]?.canonicalRecordId).not.toBe(first.id);

    const outbound = await host.signEvent({
      kind: ISSUE31_HOST_ANNOUNCEMENT_KIND,
      tags: [],
      content: "{}",
    });
    const receipt = client.publish(outbound);
    expect(receipt).toMatchObject({
      transportState: "sent",
      relayAcknowledgement: "pending",
      commandCompletion: "pending_terminal_record",
    });
    socket.message(["OK", outbound.id, false, "blocked: policy"]);
    await settleCallbacks();
    expect(client.snapshot().publishRefusals["wss://relay.example.com"]?.[outbound.id]).toBe(
      "blocked: policy",
    );
    const sentBeforeRetry = socket.sent.filter(
      (frame) => Array.isArray(frame) && frame[0] === "EVENT" && frame[1]?.id === outbound.id,
    ).length;
    expect(client.retryPublish(outbound.id)).toBe(true);
    expect(
      socket.sent.filter(
        (frame) => Array.isArray(frame) && frame[0] === "EVENT" && frame[1]?.id === outbound.id,
      ),
    ).toHaveLength(sentBeforeRetry + 1);
    expect(
      client.snapshot().publishRefusals["wss://relay.example.com"]?.[outbound.id],
    ).toBeUndefined();
    socket.message(["OK", outbound.id, false, "blocked again"]);
    await settleCallbacks();
    expect(client.discardPublish(outbound.id)).toBe(true);
    expect(client.retryPublish(outbound.id)).toBe(false);
    expect(
      client.snapshot().publishRefusals["wss://relay.example.com"]?.[outbound.id],
    ).toBeUndefined();
    expect(receipt.commandCompletion).toBe("pending_terminal_record");
    client.close();
    host.dispose();
    device.dispose();
  });

  test("closes a replay that does not reach EOSE and exposes an exact gap", async () => {
    ScriptedWebSocket.instances = [];
    const device = LocalKeySigner.fromPrivateKey(deviceSecret);
    const timers: Array<Readonly<{ callback: () => void; delayMs: number }>> = [];
    const client = createIssue31NostrClient({
      relayUrls: ["wss://relay.example.com"],
      signer: device,
      webSocket: ScriptedWebSocket,
      eoseTimeoutMs: 250,
      setTimer: (callback, delayMs) => timers.push({ callback, delayMs }),
    });
    await client.start();
    const socket = ScriptedWebSocket.instances[0];
    if (socket === undefined) throw new Error("expected socket");
    socket.open();
    await settleCallbacks();
    const timeout = timers.find((timer) => timer.delayMs === 250);
    if (timeout === undefined) throw new Error("expected EOSE timeout");
    timeout.callback();
    expect(socket.closes.at(-1)).toMatchObject({ code: 1002, reason: "issue31_eose_timeout" });
    expect(client.snapshot().relays[0]?.gapReason).toBe("disconnect_before_eose");
    client.close();
    device.dispose();
  });

  test("converges discovery by content generation and fails closed on same-generation forks", async () => {
    ScriptedWebSocket.instances = [];
    const device = LocalKeySigner.fromPrivateKey(deviceSecret);
    const host = LocalKeySigner.fromPrivateKey(hostSecret);
    const client = createIssue31NostrClient({
      relayUrls: ["wss://relay.example.com"],
      signer: device,
      webSocket: ScriptedWebSocket,
      admittedHostPublicKeys: [host.publicKey],
    });
    await client.start();
    const socket = ScriptedWebSocket.instances[0];
    if (socket === undefined) throw new Error("expected socket");
    socket.open();
    await settleCallbacks();
    const discoveryRequest = socket.sent.find(
      (value): value is unknown[] =>
        Array.isArray(value) && value[0] === "REQ" && String(value[1]).includes("discovery-0"),
    );
    if (discoveryRequest === undefined) throw new Error("expected discovery request");
    const discovery = async (generation: number, createdAt: number, displayName: string) =>
      host.signEvent({
        kind: ISSUE31_HOST_ANNOUNCEMENT_KIND,
        created_at: createdAt,
        tags: [
          ["d", "omega.host.local"],
          ["k", "1059"],
          ["t", "omega-issue31-host"],
          ["alt", "Omega Issue 31 host"],
        ],
        content: JSON.stringify({
          schema: ISSUE31_HOST_ANNOUNCEMENT_SCHEMA,
          hostRef: "omega.host.local",
          hostPublicKeyHex: host.publicKey,
          sarahPublicKeyHex: "3".repeat(64),
          displayName,
          protocols: [ISSUE31_PAIRING_SCHEMA, ISSUE31_COMMAND_SCHEMA],
          relayUrls: ["wss://relay.example.com"],
          generation,
          issuedAt: 1,
          expiresAt: 10_000,
        }),
      });
    socket.message(["EVENT", discoveryRequest[1], await discovery(2, 200, "Omega v2")]);
    socket.message(["EVENT", discoveryRequest[1], await discovery(1, 300, "Omega stale")]);
    await settleCallbacks();
    expect(client.snapshot().confirmedEvents[0]?.hostAnnouncement?.generation).toBe(2);
    socket.message(["EVENT", discoveryRequest[1], await discovery(2, 400, "Omega fork")]);
    await settleCallbacks();
    expect(client.snapshot().relays[0]?.gapReason).toBe("discovery_conflict");
    for (const request of socket.sent.filter(
      (value): value is unknown[] => Array.isArray(value) && value[0] === "REQ",
    )) {
      socket.message(["EOSE", request[1]]);
    }
    await settleCallbacks();
    expect(client.snapshot().relays[0]?.gapReason).toBe("discovery_conflict");
    socket.message(["EVENT", discoveryRequest[1], await discovery(3, 100, "Omega v3")]);
    await settleCallbacks();
    expect(client.snapshot().confirmedEvents[0]?.hostAnnouncement?.generation).toBe(3);
    expect(client.snapshot().relays[0]?.gapReason).toBeNull();
    client.close();
    host.dispose();
    device.dispose();
  });

  test("rejects a spoofed same-name discovery announcement outside the admitted host keys", async () => {
    ScriptedWebSocket.instances = [];
    const device = LocalKeySigner.fromPrivateKey(deviceSecret);
    const host = LocalKeySigner.fromPrivateKey(hostSecret);
    const attacker = LocalKeySigner.fromPrivateKey(ephemeralOne);
    const client = createIssue31NostrClient({
      relayUrls: ["wss://relay.example.com"],
      signer: device,
      webSocket: ScriptedWebSocket,
      admittedHostPublicKeys: [host.publicKey],
    });
    await client.start();
    const socket = ScriptedWebSocket.instances[0];
    if (socket === undefined) throw new Error("expected socket");
    socket.open();
    await settleCallbacks();
    const discoveryRequest = socket.sent.find(
      (value): value is unknown[] =>
        Array.isArray(value) && value[0] === "REQ" && String(value[1]).includes("discovery-0"),
    );
    if (discoveryRequest === undefined) throw new Error("expected discovery request");
    const announcement = async (signer: LocalKeySigner) =>
      signer.signEvent({
        kind: ISSUE31_HOST_ANNOUNCEMENT_KIND,
        created_at: 1_000,
        tags: [
          ["d", "omega.host.local"],
          ["k", "1059"],
          ["t", "omega-issue31-host"],
        ],
        content: JSON.stringify({
          schema: ISSUE31_HOST_ANNOUNCEMENT_SCHEMA,
          hostRef: "omega.host.local",
          hostPublicKeyHex: signer.publicKey,
          sarahPublicKeyHex: "3".repeat(64),
          displayName: "Local Omega",
          protocols: [ISSUE31_PAIRING_SCHEMA, ISSUE31_COMMAND_SCHEMA],
          relayUrls: ["wss://relay.example.com"],
          generation: 1,
          issuedAt: 1_000,
          expiresAt: 2_000,
        }),
      });
    socket.message(["EVENT", discoveryRequest[1], await announcement(attacker)]);
    socket.message(["EVENT", discoveryRequest[1], await announcement(host)]);
    for (const request of socket.sent.filter(
      (value): value is unknown[] => Array.isArray(value) && value[0] === "REQ",
    )) {
      socket.message(["EOSE", request[1]]);
    }
    await settleCallbacks();
    expect(client.snapshot().confirmedEvents).toHaveLength(1);
    expect(client.snapshot().confirmedEvents[0]?.event.pubkey).toBe(host.publicKey);
    expect(client.snapshot().relays[0]).toMatchObject({
      gapReason: null,
      rejectedEventCount: 1,
    });
    client.close();
    attacker.dispose();
    host.dispose();
    device.dispose();
  });

  test("replays exact queued events after CLOSED and exposes NOTICE degradation", async () => {
    ScriptedWebSocket.instances = [];
    const device = LocalKeySigner.fromPrivateKey(deviceSecret);
    const timers: Array<Readonly<{ callback: () => void; delayMs: number }>> = [];
    const client = createIssue31NostrClient({
      relayUrls: ["wss://relay.example.com"],
      signer: device,
      webSocket: ScriptedWebSocket,
      maxQueuedEvents: 1,
      setTimer: (callback, delayMs) => timers.push({ callback, delayMs }),
    });
    await client.start();
    const firstSocket = ScriptedWebSocket.instances[0];
    if (firstSocket === undefined) throw new Error("expected first socket");
    firstSocket.open();
    await settleCallbacks();
    const requests = firstSocket.sent.filter(
      (value): value is unknown[] => Array.isArray(value) && value[0] === "REQ",
    );
    for (const request of requests) firstSocket.message(["EOSE", request[1]]);
    const outbound = await device.signEvent({ kind: 1, tags: [], content: "exact-on-retry" });
    client.publish(outbound);
    const overflow = await device.signEvent({ kind: 1, tags: [], content: "bounded-overflow" });
    expect(() => client.publish(overflow)).toThrow(/queue is full/);
    firstSocket.message(["NOTICE", "maintenance"]);
    await settleCallbacks();
    expect(client.snapshot().relays[0]?.gapReason).toBe("relay_notice");
    firstSocket.message(["CLOSED", requests[0]?.[1], "rate-limited"]);
    expect(firstSocket.closes.at(-1)?.reason).toBe("issue31_subscription_closed");
    firstSocket.remoteClose();
    const reconnect = timers.find((timer) => timer.delayMs === 500);
    if (reconnect === undefined) throw new Error("expected reconnect timer");
    reconnect.callback();
    await settleCallbacks();
    const secondSocket = ScriptedWebSocket.instances[1];
    if (secondSocket === undefined) throw new Error("expected second socket");
    secondSocket.open();
    await settleCallbacks();
    expect(
      secondSocket.sent.some(
        (frame) => Array.isArray(frame) && frame[0] === "EVENT" && frame[1]?.id === outbound.id,
      ),
    ).toBe(true);
    client.close();
    device.dispose();
  });

  test("ignores signed cross-room and undecryptable injections without reconnect exhaustion", async () => {
    ScriptedWebSocket.instances = [];
    const device = LocalKeySigner.fromPrivateKey(deviceSecret);
    const host = LocalKeySigner.fromPrivateKey(hostSecret);
    const attacker = LocalKeySigner.fromPrivateKey(ephemeralOne);
    const sarah = LocalKeySigner.fromPrivateKey(ephemeralTwo);
    const cursors = cursorStore();
    const client = createIssue31NostrClient({
      relayUrls: ["wss://relay.example.com"],
      signer: device,
      webSocket: ScriptedWebSocket,
      cursorStore: cursors.store,
      admittedHostPublicKeys: [host.publicKey],
      selectedHostPublicKeys: [host.publicKey],
      ownerAuthors: [sarah.publicKey],
      ownerRecipientPublicKeys: [host.publicKey],
      communityGroupIds: ["omega-community"],
    });
    await client.start();
    const socket = ScriptedWebSocket.instances[0];
    if (socket === undefined) throw new Error("expected socket");
    socket.open();
    await settleCallbacks();
    const requests = socket.sent.filter(
      (value): value is unknown[] => Array.isArray(value) && value[0] === "REQ",
    );
    const ownerPublicRequest = requests.find(
      (request) =>
        String(request[1]).includes("owner_private-1") &&
        (request[2] as Readonly<{ authors?: ReadonlyArray<string> }>).authors !== undefined,
    );
    const privateRequest = requests.find((request) =>
      String(request[1]).includes("owner_private-0"),
    );
    const communityRequest = requests.find((request) => String(request[1]).includes("community-0"));
    if (
      ownerPublicRequest === undefined ||
      privateRequest === undefined ||
      communityRequest === undefined
    ) {
      throw new Error("expected isolated requests");
    }
    for (const request of requests) socket.message(["EOSE", request[1]]);
    await settleCallbacks();
    expect(client.snapshot().relays[0]?.gapReason).toBeNull();
    const ownerKind = ISSUE31_OWNER_PRIVATE_KINDS.find((kind) => kind !== 1_059);
    if (ownerKind === undefined) throw new Error("expected owner kind");
    const wrongAuthor = await attacker.signEvent({
      kind: ownerKind,
      tags: [["p", host.publicKey]],
      content: "signed but not admitted",
    });
    const correctOwner = await sarah.signEvent({
      kind: ownerKind,
      tags: [["p", host.publicKey]],
      content: "signed for the selected owner",
    });
    const wrongOwner = await sarah.signEvent({
      kind: ownerKind,
      tags: [["p", attacker.publicKey]],
      content: "signed for a different owner",
    });
    const undecryptable = await attacker.signEvent({
      kind: 1_059,
      tags: [["p", device.publicKey]],
      content: "valid signature, invalid NIP-44 payload",
    });
    const rumorBase = {
      pubkey: attacker.publicKey,
      created_at: 1_000,
      kind: ISSUE31_PRIVATE_RUMOR_KIND,
      tags: [["p", device.publicKey]],
      content: "hostile but structurally valid generic NIP-17 content",
    };
    const rumor = { ...rumorBase, id: getEventHash(rumorBase) };
    const seal = await attacker.signEvent({
      kind: ISSUE31_PRIVATE_SEAL_KIND,
      created_at: 1_000,
      tags: [],
      content: await attacker.nip44Encrypt(device.publicKey, JSON.stringify(rumor)),
    });
    const wrapper = LocalKeySigner.fromPrivateKey(ephemeralTwo);
    const hostileGenericWrap = await wrapper.signEvent({
      kind: ISSUE31_PRIVATE_GIFT_WRAP_KIND,
      created_at: 900,
      tags: [["p", device.publicKey]],
      content: await wrapper.nip44Encrypt(device.publicKey, JSON.stringify(seal)),
    });
    wrapper.dispose();
    const hostileStructuredWrap = await createIssue31PrivateGiftWrap({
      signer: attacker,
      recipientPublicKeyHex: device.publicKey,
      record: decodeIssue31PairingRecord({
        schema: ISSUE31_PAIRING_SCHEMA,
        recordType: "scoped_grant",
        hostRef: "omega.host.spoof",
        hostPublicKeyHex: attacker.publicKey,
        sarahPublicKeyHex: "3".repeat(64),
        devicePublicKeyHex: device.publicKey,
        issuedAt: 1_000,
        pairingResponseEventId: "a".repeat(64),
        grantRef: "grant.omega.spoof_1",
        generation: 1,
        scopes: ["observe_issue31"],
        expiresAt: 2_000,
      }),
      randomSecretKey: () => new Uint8Array(ephemeralTwo),
      createdAt: 1_000,
      sealCreatedAt: 950,
      wrapCreatedAt: 900,
    });
    const sarahRumorBase = {
      pubkey: sarah.publicKey,
      created_at: 1_000,
      kind: ISSUE31_PRIVATE_RUMOR_KIND,
      tags: [["p", device.publicKey]],
      content: "authenticated Sarah owner-private content",
    };
    const sarahRumor = { ...sarahRumorBase, id: getEventHash(sarahRumorBase) };
    const sarahSeal = await sarah.signEvent({
      kind: ISSUE31_PRIVATE_SEAL_KIND,
      created_at: 1_000,
      tags: [],
      content: await sarah.nip44Encrypt(device.publicKey, JSON.stringify(sarahRumor)),
    });
    const sarahGenericWrap = await host.signEvent({
      kind: ISSUE31_PRIVATE_GIFT_WRAP_KIND,
      created_at: 900,
      tags: [["p", device.publicKey]],
      content: await host.nip44Encrypt(device.publicKey, JSON.stringify(sarahSeal)),
    });
    const wrongGroup = await attacker.signEvent({
      kind: 9,
      tags: [["h", "different-community"]],
      content: "signed wrong group",
    });
    socket.message(["EVENT", ownerPublicRequest[1], wrongAuthor]);
    socket.message(["EVENT", ownerPublicRequest[1], correctOwner]);
    socket.message(["EVENT", ownerPublicRequest[1], wrongOwner]);
    socket.message(["EVENT", privateRequest[1], undecryptable]);
    socket.message(["EVENT", privateRequest[1], hostileGenericWrap]);
    socket.message(["EVENT", privateRequest[1], hostileStructuredWrap]);
    socket.message(["EVENT", privateRequest[1], sarahGenericWrap]);
    socket.message(["EVENT", communityRequest[1], wrongGroup]);
    await settleCallbacks();
    expect(client.snapshot().confirmedEvents).toHaveLength(2);
    expect(client.snapshot().confirmedEvents.map((event) => event.canonicalRecordId)).toEqual(
      expect.arrayContaining([correctOwner.id, sarahRumor.id]),
    );
    expect(client.snapshot().relays[0]).toMatchObject({
      gapReason: null,
      rejectedEventCount: 6,
    });
    expect(socket.closes).toHaveLength(0);
    expect(cursors.saved).toHaveLength(2);
    expect(cursors.saved.every((saved) => saved.room === "owner_private")).toBe(true);
    client.close();
    sarah.dispose();
    attacker.dispose();
    host.dispose();
    device.dispose();
  });

  test("loads exact persisted publishes after restart and retains relay refusals", async () => {
    ScriptedWebSocket.instances = [];
    const device = LocalKeySigner.fromPrivateKey(deviceSecret);
    const persisted = new Map<string, Issue31SignedNostrEvent>();
    const deleted: string[] = [];
    const outboundStore: Issue31OutboundEventStore = {
      load: () => [...persisted.values()],
      put: (event) => {
        persisted.set(event.id, event);
      },
      delete: (eventId) => {
        deleted.push(eventId);
        persisted.delete(eventId);
      },
      close: () => undefined,
    };
    const first = createIssue31NostrClient({
      relayUrls: ["wss://relay.example.com"],
      signer: device,
      webSocket: ScriptedWebSocket,
      outboundStore,
    });
    await first.start();
    const firstSocket = ScriptedWebSocket.instances[0];
    if (firstSocket === undefined) throw new Error("expected first socket");
    firstSocket.open();
    await settleCallbacks();
    const event = await device.signEvent({ kind: 1, tags: [], content: "survive-restart" });
    first.publish(event);
    firstSocket.message(["OK", event.id, false, "rate-limited"]);
    await settleCallbacks();
    expect(persisted.has(event.id)).toBe(true);
    expect(deleted).toHaveLength(0);
    first.close();

    const second = createIssue31NostrClient({
      relayUrls: ["wss://relay.example.com"],
      signer: device,
      webSocket: ScriptedWebSocket,
      outboundStore,
    });
    await second.start();
    const secondSocket = ScriptedWebSocket.instances[1];
    if (secondSocket === undefined) throw new Error("expected restarted socket");
    secondSocket.open();
    await settleCallbacks();
    expect(
      secondSocket.sent.some(
        (frame) => Array.isArray(frame) && frame[0] === "EVENT" && frame[1]?.id === event.id,
      ),
    ).toBe(true);
    secondSocket.message(["OK", event.id, true, "stored"]);
    await settleCallbacks();
    expect(persisted.has(event.id)).toBe(false);
    expect(deleted).toEqual([event.id]);
    second.close();
    device.dispose();
  });
});
