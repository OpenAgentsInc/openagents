/**
 * Revocation on the DEVICE SURFACE, over a real relay (omega#49).
 *
 * The device-surface finding: the owner revoked the grant through the real
 * Omega host, the host's own projection flipped to `revoked · scopes []`, the
 * revocation gift wrap reached `wss://relay.openagents.com` — and the phone
 * kept rendering `Device grant confirmed by signed Nostr records.` and
 * `Owner-private source ready · generation 1` across a full app restart.
 *
 * The gift wraps are sealed, so nothing observed from the relay or the host can
 * say whether the device received the revocation and ignored it or never
 * received it at all. Only the device can answer that, and nothing drove
 * `openIssue31MobileNostrRuntime` end to end: every other mobile test builds
 * `createIssue31NostrClient` directly and hands the read model already-decoded
 * records, so the runtime's own reconciliation — the layer that owns `phase`
 * and `notice` — had never been on a wire.
 *
 * The answer, from "the device really receives the revocation" below, is
 * RECEIVED AND IGNORED: the revocation is unwrapped, NIP-44 decrypted, decoded
 * and present in the device's own snapshot, the owner-private read model
 * already stops reporting `ready` from it — and the control surface went on
 * saying the grant was confirmed.
 *
 * These tests drive the whole runtime: the real device key vault over a real
 * keychain-shaped store, the real SQLite stores over real files, the real
 * client over a real WebSocket to a real in-process relay, and a host simulator
 * that really unwraps the device's NIP-59 gift wraps and answers them under the
 * rumor ids the device minted. The pairing chain is built the way a phone
 * builds it, not stapled together.
 *
 * A local relay is a real relay. It is still not a phone: the omega#49 exits
 * that name a device need a device.
 */
import { webcrypto } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import NodeModule from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  ISSUE31_HOST_ANNOUNCEMENT_KIND,
  ISSUE31_PAIRING_SCHEMA,
  createIssue31PrivateGiftWrap,
  decodeIssue31PairingRecord,
  unwrapIssue31PrivateGiftWrap,
  type Issue31PairingRecord,
  type Issue31SignedNostrEvent,
} from "@openagentsinc/sarah/issue31-nostr";
import { openNodeSqliteDatabase } from "@openagentsinc/sqlite-runtime";
import { LocalKeySigner } from "nostr-effect/identity";
import { finalizeEvent, generateSecretKey, getPublicKey } from "nostr-effect/pure";
import { startTestRelay } from "nostr-effect/relay/node";
import { afterAll, afterEach, beforeAll, describe, expect, test } from "vite-plus/test";

import type { Issue31NostrClientSnapshot } from "../src/workroom/issue31-nostr-client.ts";
import {
  ISSUE31_GRANT_REVOKED_NOTICE,
  openIssue31MobileNostrRuntime,
  type Issue31MobileNostrControlState,
  type Issue31MobileNostrRuntime,
} from "../src/workroom/issue31-mobile-nostr-runtime.ts";
import {
  createIssue31LocalPairingRecordStore,
  issue31PairingEvictionPlan,
  type Issue31SQLiteDatabase,
} from "../src/workroom/issue31-outbound-event-store.ts";
import { projectIssue31OwnerPrivateReadModel } from "../src/workroom/issue31-owner-private-read-model.ts";

const CONFIRMED_NOTICE = "Device grant confirmed by signed Nostr records.";

/**
 * One simulated device: a keychain and a SQLite directory that outlive the
 * runtime using them, so a "restart" below is a real close-and-reopen over the
 * same durable state rather than a fresh install.
 */
interface SimulatedDevice {
  readonly secureStore: Record<string, unknown>;
  readonly directory: string;
  readonly openDatabases: Array<() => void>;
}

let device: SimulatedDevice | null = null;
const temporaryDirectories: string[] = [];

const newTemporaryDirectory = (): string => {
  const directory = mkdtempSync(join(tmpdir(), "omega-issue31-revocation-"));
  temporaryDirectories.push(directory);
  return directory;
};

const newDevice = (): SimulatedDevice => {
  const values = new Map<string, string>();
  return {
    directory: newTemporaryDirectory(),
    openDatabases: [],
    secureStore: {
      AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: "after-first-unlock-device-only",
      getItemAsync: async (key: string) => values.get(key) ?? null,
      setItemAsync: async (key: string, value: string) => {
        values.set(key, value);
      },
      deleteItemAsync: async (key: string) => {
        values.delete(key);
      },
    },
  };
};

const openSqliteFile = (path: string): Issue31SQLiteDatabase => {
  const database = openNodeSqliteDatabase(path);
  let closed = false;
  return {
    execSync: (sql: string) => database.exec(sql),
    runSync: (sql: string, ...params: ReadonlyArray<string | number>) =>
      database.run(sql, [...params]),
    getAllSync: <Row,>(sql: string, ...params: ReadonlyArray<string | number>) =>
      database.all<Row>(sql, [...params]),
    closeSync: () => {
      if (closed) return;
      closed = true;
      database.close();
    },
  };
};

/**
 * The three native modules the runtime reaches for through `require`.
 *
 * Only the platform is simulated. The keychain is keychain-shaped, the random
 * bytes are real WebCrypto bytes, and the databases are real SQLite files on
 * disk that survive a close-and-reopen. Everything under test is shipped code.
 */
const nativeModule = (id: string): unknown => {
  // Narrowed to the window in which a simulated device exists, so the loader
  // patch can never answer for anything but these tests.
  if (device === null) return null;
  if (id === "expo-secure-store") {
    return device.secureStore;
  }
  if (id === "expo-crypto") {
    return {
      getRandomBytesAsync: async (length: number) =>
        webcrypto.getRandomValues(new Uint8Array(length)),
    };
  }
  if (id === "expo-sqlite") {
    return {
      openDatabaseSync: (name: string) => {
        const current = device;
        if (current === null) throw new Error("no simulated device is open");
        const handle = openSqliteFile(join(current.directory, name));
        current.openDatabases.push(handle.closeSync);
        return handle;
      },
    };
  }
  return null;
};

// The runtime's `require` is a real CommonJS resolver under the test runner, so
// the interception belongs in the loader rather than on `globalThis`. Without
// it the real Expo packages load and fail on their missing native side.
const moduleLoader = NodeModule as unknown as {
  _load: (request: string, parent: unknown, isMain: boolean) => unknown;
};
const originalModuleLoad = moduleLoader._load;
moduleLoader._load = (request, parent, isMain) =>
  nativeModule(request) ?? originalModuleLoad.call(moduleLoader, request, parent, isMain);

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

let relay: Awaited<ReturnType<typeof startTestRelay>>;
let relayUrl: string;

beforeAll(async () => {
  relay = await startTestRelay(41_000 + Math.floor(Math.random() * 4_000));
  relayUrl = `ws://127.0.0.1:${relay.port}`;
});

afterAll(async () => {
  moduleLoader._load = originalModuleLoad;
  await Promise.resolve(relay?.stop());
  for (const directory of temporaryDirectories) {
    rmSync(directory, { recursive: true, force: true });
  }
});

afterEach(() => {
  for (const close of device?.openDatabases ?? []) {
    try {
      close();
    } catch {
      /* the runtime already closed it */
    }
  }
  device = null;
});

/** Publish one event to one relay and require that relay's affirmative OK. */
const publish = async (url: string, event: { readonly id: string }): Promise<string> => {
  const socket = new WebSocket(url);
  try {
    await new Promise<void>((resolve, reject) => {
      socket.onopen = () => resolve();
      socket.onerror = () => reject(new Error("publisher socket failed"));
    });
    const accepted = new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`no OK for ${event.id}`)), 15_000);
      socket.onmessage = (message: MessageEvent) => {
        const frame = JSON.parse(String(message.data)) as ReadonlyArray<unknown>;
        if (frame[0] === "OK" && frame[1] === event.id) {
          clearTimeout(timer);
          if (frame[2] === true) resolve();
          else reject(new Error(`relay refused ${event.id}: ${String(frame[3])}`));
        }
      };
    });
    socket.send(JSON.stringify(["EVENT", event]));
    await accepted;
    return event.id;
  } finally {
    socket.close(1000, "done");
  }
};

const signerFor = (localSigner: LocalKeySigner) => ({
  getPublicKey: () => localSigner.getPublicKey(),
  signEvent: (event: Parameters<LocalKeySigner["signEvent"]>[0]) => localSigner.signEvent(event),
  nip44Encrypt: (recipient: string, plaintext: string) =>
    localSigner.nip44Encrypt(recipient, plaintext),
  nip44Decrypt: (sender: string, ciphertext: string) =>
    localSigner.nip44Decrypt(sender, ciphertext),
});

/** One Omega host identity, reusable across relays and restarts. */
interface HostIdentity {
  readonly hostRef: string;
  readonly publicKey: string;
  readonly sarahPublicKey: string;
  readonly signer: LocalKeySigner;
}

const newHost = (label: string): HostIdentity => {
  const secret = generateSecretKey();
  return {
    hostRef: `omega.host.${label}_${Math.floor(Date.now() / 1_000)}`,
    publicKey: getPublicKey(secret),
    sarahPublicKey: getPublicKey(generateSecretKey()),
    signer: LocalKeySigner.fromPrivateKey(secret),
  };
};

const announceHost = async (url: string, host: HostIdentity): Promise<void> => {
  const issuedAt = Math.floor(Date.now() / 1_000);
  const announcement = await host.signer.signEvent({
    kind: ISSUE31_HOST_ANNOUNCEMENT_KIND,
    created_at: issuedAt,
    tags: [
      ["t", "omega-issue31-host"],
      ["d", host.hostRef],
      ["k", "1059"],
    ],
    content: JSON.stringify({
      schema: "openagents.omega.issue31.host_discovery.v2",
      hostRef: host.hostRef,
      hostPublicKeyHex: host.publicKey,
      sarahPublicKeyHex: host.sarahPublicKey,
      displayName: "Omega revocation host",
      conversation: "sarah.0123456789abcdef01234567",
      protocols: [
        "openagents.omega.issue31.pairing.v1",
        "openagents.omega.issue31.command.v1",
        "openagents.omega.issue31.command.v2",
      ],
      relayUrls: ["wss://relay.example.com"],
      generation: 1,
      issuedAt,
      expiresAt: issuedAt + 86_400,
    }),
  });
  await publish(url, announcement);
};

/**
 * The Omega host, listening on the same relay the device publishes to.
 *
 * It really unwraps the device's gift wraps, so the identifiers it answers
 * under are the NIP-59 rumor ids the device minted — the join
 * `foldIssue31Grant` folds the chain by. Nothing here is handed the device's
 * records out of band.
 */
const startHostInbox = async (
  url: string,
  host: HostIdentity,
): Promise<
  Readonly<{
    received: ReadonlyArray<Readonly<{ eventId: string; record: Issue31PairingRecord }>>;
    stop: () => void;
  }>
> => {
  const received: Array<Readonly<{ eventId: string; record: Issue31PairingRecord }>> = [];
  const socket = new WebSocket(url);
  await new Promise<void>((resolve, reject) => {
    socket.onopen = () => resolve();
    socket.onerror = () => reject(new Error("host socket failed"));
  });
  socket.onmessage = (message: MessageEvent) => {
    const frame = JSON.parse(String(message.data)) as ReadonlyArray<unknown>;
    if (frame[0] !== "EVENT") return;
    void (async () => {
      try {
        const opened = await unwrapIssue31PrivateGiftWrap({
          signer: signerFor(host.signer),
          giftWrap: frame[2],
        });
        if (opened.record?.schema === ISSUE31_PAIRING_SCHEMA) {
          received.push({ eventId: opened.rumor.id, record: opened.record });
        }
      } catch {
        /* not addressed to this host */
      }
    })();
  };
  socket.send(JSON.stringify(["REQ", "host-inbox", { kinds: [1_059], "#p": [host.publicKey] }]));
  return { received, stop: () => socket.close(1000, "done") };
};

/** Seal one host-authored pairing record to the device and publish it. */
const wrapToDevice = async (
  url: string,
  host: HostIdentity,
  devicePublicKey: string,
  record: Issue31PairingRecord,
): Promise<void> => {
  const createdAt = Math.floor(Date.now() / 1_000);
  const wrap = await createIssue31PrivateGiftWrap({
    signer: signerFor(host.signer),
    recipientPublicKeyHex: devicePublicKey,
    record,
    randomSecretKey: generateSecretKey,
    createdAt,
    sealCreatedAt: createdAt,
    wrapCreatedAt: createdAt,
  });
  await publish(url, wrap);
};

interface OpenedRuntime {
  readonly runtime: Issue31MobileNostrRuntime;
  readonly control: () => Issue31MobileNostrControlState | null;
  readonly snapshot: () => Issue31NostrClientSnapshot | null;
}

const openRuntime = async (
  url: string,
  admittedHostPublicKeys: ReadonlyArray<string>,
): Promise<OpenedRuntime> => {
  let control: Issue31MobileNostrControlState | null = null;
  let snapshot: Issue31NostrClientSnapshot | null = null;
  const runtime = await openIssue31MobileNostrRuntime({
    relayUrls: [url],
    admittedHostPublicKeys,
    onControlState: (next) => {
      control = next;
    },
    onSnapshot: (next) => {
      snapshot = next;
    },
  });
  // Read through closures: these are only ever assigned from callbacks the
  // compiler cannot see, so straight-line narrowing would collapse them.
  return { runtime, control: () => control, snapshot: () => snapshot };
};

/**
 * Take one device from discovery to a confirmed grant over a real relay, the
 * way a phone does it: signed announcement, device-authored request, host
 * challenge, device-authored answer, host-authored scoped grant.
 */
const pairDevice = async (
  url: string,
  host: HostIdentity,
  opened: OpenedRuntime,
  grantRef: string,
): Promise<Readonly<{ stopHost: () => void }>> => {
  const inbox = await startHostInbox(url, host);
  await announceHost(url, host);
  await waitFor(
    () => (opened.control()?.hosts ?? []).some((row) => row.hostPublicKeyHex === host.publicKey),
    "the signed host announcement to reach the device",
  );
  await opened.runtime.selectHost(host.publicKey);
  await opened.runtime.requestPairing();

  await waitFor(
    () => inbox.received.some((row) => row.record.recordType === "pairing_request"),
    "the device's sealed pairing request to reach the host",
  );
  const request = inbox.received.find((row) => row.record.recordType === "pairing_request")!;
  await wrapToDevice(
    url,
    host,
    opened.runtime.publicKeyHex,
    decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "pairing_challenge",
      hostRef: host.hostRef,
      hostPublicKeyHex: host.publicKey,
      devicePublicKeyHex: opened.runtime.publicKeyHex,
      issuedAt: request.record.issuedAt,
      pairingChallengeRef: "pairing.challenge.revocation",
      pairingRequestEventId: request.eventId,
      challenge: "a".repeat(64),
      expiresAt: request.record.issuedAt + 86_400,
    }),
  );

  await waitFor(
    () => inbox.received.some((row) => row.record.recordType === "pairing_response"),
    "the device to answer the challenge on its own",
  );
  const response = inbox.received.find((row) => row.record.recordType === "pairing_response")!;
  await wrapToDevice(
    url,
    host,
    opened.runtime.publicKeyHex,
    decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "scoped_grant",
      hostRef: host.hostRef,
      hostPublicKeyHex: host.publicKey,
      sarahPublicKeyHex: host.sarahPublicKey,
      devicePublicKeyHex: opened.runtime.publicKeyHex,
      issuedAt: response.record.issuedAt,
      pairingResponseEventId: response.eventId,
      grantRef,
      generation: 1,
      scopes: ["observe_issue31", "send_message"],
      expiresAt: response.record.issuedAt + 86_400,
    }),
  );

  await waitFor(() => opened.control()?.phase === "paired", "the device to confirm the grant");
  return { stopHost: inbox.stop };
};

const revokeGrant = async (
  url: string,
  host: HostIdentity,
  devicePublicKey: string,
  grantRef: string,
): Promise<void> =>
  wrapToDevice(
    url,
    host,
    devicePublicKey,
    decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "grant_revocation",
      hostRef: host.hostRef,
      hostPublicKeyHex: host.publicKey,
      sarahPublicKeyHex: host.sarahPublicKey,
      devicePublicKeyHex: devicePublicKey,
      issuedAt: Math.floor(Date.now() / 1_000),
      grantRef,
      generation: 1,
      reasonRef: "reason.omega.owner_revoked",
    }),
  );

const ownerPrivateStatus = (opened: OpenedRuntime): string =>
  projectIssue31OwnerPrivateReadModel(opened.snapshot()!, {
    nowUnixSeconds: Math.floor(Date.now() / 1_000),
  }).status;

describe("a revoked Omega grant on the device surface", () => {
  /**
   * Received, or never received? This settles it, and then holds the surface
   * to the answer.
   */
  test("the device really receives the revocation and stops claiming a grant", async () => {
    device = newDevice();
    const host = newHost("live");
    const grantRef = `grant.omega.live_${Math.floor(Date.now() / 1_000)}`;
    const opened = await openRuntime(relayUrl, [host.publicKey]);
    let stopHost = (): void => {};
    try {
      ({ stopHost } = await pairDevice(relayUrl, host, opened, grantRef));
      expect(opened.control()?.notice).toBe(CONFIRMED_NOTICE);
      expect(ownerPrivateStatus(opened)).toBe("ready");

      await revokeGrant(relayUrl, host, opened.runtime.publicKeyHex, grantRef);

      // RECEIVED. The record is unwrapped, NIP-44 decrypted, decoded, and in
      // the device's own snapshot. Nothing below can be explained by absence.
      await waitFor(
        () =>
          (opened.snapshot()?.confirmedEvents ?? []).some(
            (row) => row.privateRecord?.recordType === "grant_revocation",
          ),
        "the revocation to be unwrapped and decoded on the device",
      );
      // The owner-private read model already folds it correctly. Every part of
      // the failure below is the control surface's own.
      expect(ownerPrivateStatus(opened)).not.toBe("ready");

      await waitFor(
        () => opened.control()?.phase !== "paired",
        "the control surface to drop the confirmed-grant claim",
      );
      expect(opened.control()?.notice).not.toBe(CONFIRMED_NOTICE);
      // Named revocation, not discovery. A revoked device is not a device that
      // cannot find its host.
      expect(opened.control()?.notice).toBe(ISSUE31_GRANT_REVOKED_NOTICE);
      expect(opened.control()?.notice ?? "").not.toMatch(/announcement|discover/i);
    } finally {
      stopHost();
      opened.runtime.close();
    }
  }, 90_000);

  /**
   * The restart, which is where the phone was found.
   *
   * The relaunch is pointed at a SECOND relay that has never carried the
   * revocation — or the grant, or the challenge. Only a fresh host announcement
   * lives there. So the only place the revoked chain can come from is the
   * device's own durable history, which is exactly the claim: a revocation must
   * hold across a restart, not merely for as long as a relay keeps re-serving
   * it. A cursor that has moved past the record is one way that goes wrong; an
   * emptier relay is the same condition with nothing left to hide behind.
   */
  test("a revocation received before a restart still holds after one", async () => {
    device = newDevice();
    const host = newHost("restart");
    const grantRef = `grant.omega.restart_${Math.floor(Date.now() / 1_000)}`;
    const first = await openRuntime(relayUrl, [host.publicKey]);
    let stopHost = (): void => {};
    try {
      ({ stopHost } = await pairDevice(relayUrl, host, first, grantRef));
      await revokeGrant(relayUrl, host, first.runtime.publicKeyHex, grantRef);
      await waitFor(
        () =>
          (first.snapshot()?.confirmedEvents ?? []).some(
            (row) => row.privateRecord?.recordType === "grant_revocation",
          ),
        "the revocation to reach the device before the restart",
      );
      await waitFor(
        () => first.control()?.phase !== "paired",
        "the device to register the revocation before the restart",
      );
    } finally {
      stopHost();
      first.runtime.close();
    }

    const secondRelay = await startTestRelay(45_000 + Math.floor(Math.random() * 4_000));
    const secondRelayUrl = `ws://127.0.0.1:${secondRelay.port}`;
    const second = await openRuntime(secondRelayUrl, [host.publicKey]);
    try {
      await announceHost(secondRelayUrl, host);
      await waitFor(
        () =>
          (second.control()?.hosts ?? []).some((row) => row.hostPublicKeyHex === host.publicKey),
        "the relaunched device to rediscover the host",
      );
      // The revoked chain came off disk: this relay has never carried it.
      await waitFor(
        () =>
          (second.snapshot()?.confirmedEvents ?? []).some(
            (row) => row.privateRecord?.recordType === "grant_revocation",
          ),
        "the revocation to be restored from the device's own history",
      );
      await waitFor(
        () => second.control()?.notice === ISSUE31_GRANT_REVOKED_NOTICE,
        "the relaunched device to say the grant was revoked",
      );
      expect(second.control()?.phase).not.toBe("paired");
      expect(second.control()?.notice).not.toBe(CONFIRMED_NOTICE);
      expect(ownerPrivateStatus(second)).not.toBe("ready");
    } finally {
      second.runtime.close();
      await Promise.resolve(secondRelay.stop());
    }
  }, 120_000);
});

describe("the local pairing bound must not launder a revocation", () => {
  const rowsOf = (types: ReadonlyArray<string | null>) =>
    types.map((recordType, index) => ({ sequence: index + 1, recordType }));

  test("eviction discards the oldest evictable row and never a revocation", () => {
    expect(
      issue31PairingEvictionPlan({
        rows: rowsOf(["pairing_request", "grant_revocation", "pairing_challenge"]),
        maximumRecords: 8,
      }),
    ).toEqual([]);
    // Full: the revocation is skipped and the oldest ordinary row goes instead.
    expect(
      issue31PairingEvictionPlan({
        rows: rowsOf(["grant_revocation", "pairing_request", "pairing_challenge"]),
        maximumRecords: 3,
      }),
    ).toEqual([2]);
    // A row whose type cannot be read might BE the revocation, so it is not
    // treated as spare capacity.
    expect(() =>
      issue31PairingEvictionPlan({
        rows: rowsOf(["grant_revocation", null]),
        maximumRecords: 2,
      }),
    ).toThrow(/full of revocations/);
    // Refuses rather than forgetting one.
    expect(() =>
      issue31PairingEvictionPlan({
        rows: rowsOf(["grant_revocation", "grant_revocation"]),
        maximumRecords: 2,
      }),
    ).toThrow(/full of revocations/);
  });

  /**
   * The same rule through real SQLite and a real close-and-reopen.
   *
   * Under the old plain-FIFO bound the revocation is written first, then pushed
   * out by ordinary pairing traffic while the grant it revokes stays inside the
   * window — and the next launch folds an active grant the owner had already
   * cut off. That is a silent un-revocation across a restart, which is the
   * failure shape the device showed.
   */
  test("a revocation survives a bound that overflows many times over", () => {
    const hostSecret = generateSecretKey();
    const hostPublicKey = getPublicKey(hostSecret);
    const devicePublicKey = getPublicKey(generateSecretKey());
    const hostRef = "omega.host.bound";
    const grantRef = "grant.omega.bound";
    const issuedAt = Math.floor(Date.now() / 1_000);
    const path = join(newTemporaryDirectory(), "openagents-omega-issue31.db");

    const signed = (content: string): Issue31SignedNostrEvent =>
      finalizeEvent(
        { kind: 1_059, created_at: issuedAt, tags: [["p", devicePublicKey]], content },
        hostSecret,
      ) as unknown as Issue31SignedNostrEvent;

    const store = createIssue31LocalPairingRecordStore(openSqliteFile(path), 8);
    const revocation = decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "grant_revocation",
      hostRef,
      hostPublicKeyHex: hostPublicKey,
      sarahPublicKeyHex: getPublicKey(generateSecretKey()),
      devicePublicKeyHex: devicePublicKey,
      issuedAt,
      grantRef,
      generation: 1,
      reasonRef: "reason.omega.owner_revoked",
    });
    const revocationEvent = signed("revocation");
    store.put({
      canonicalRecordId: revocationEvent.id,
      event: revocationEvent,
      record: revocation,
    });
    // Ordinary pairing traffic, many times the bound.
    for (let index = 0; index < 40; index += 1) {
      const event = signed(`request-${index}`);
      store.put({
        canonicalRecordId: event.id,
        event,
        record: decodeIssue31PairingRecord({
          schema: ISSUE31_PAIRING_SCHEMA,
          recordType: "pairing_request",
          hostRef,
          hostPublicKeyHex: hostPublicKey,
          devicePublicKeyHex: devicePublicKey,
          issuedAt: issuedAt + index,
          pairingRequestRef: `pairing.request.bound_${index}`,
          requestedScopes: ["observe_issue31"],
          expiresAt: issuedAt + index + 300,
        }),
      });
    }
    store.close();

    // A genuine restart: a new handle on the same file.
    const reopened = createIssue31LocalPairingRecordStore(openSqliteFile(path), 8);
    try {
      const loaded = reopened.load();
      expect(loaded.length).toBeLessThanOrEqual(8);
      expect(loaded.some((row) => row.record.recordType === "grant_revocation")).toBe(true);
    } finally {
      reopened.close();
    }
  });
});
