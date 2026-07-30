/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- @effect/vitest does not support the repository Effect 4 line. */
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import type { Issue31DeviceIdentity } from "../src/workroom/issue31-device-key-vault.ts";
import {
  OMEGA_DEVICE_BRIDGE_PROTOCOL,
  createMemoryOmegaDeviceBridgeStore,
  createOmegaDeviceBridgeClient,
  decodeOmegaBridgePairingBootstrap,
  omegaBridgeDialLadder,
  type OmegaDeviceBridgeWebSocket,
  type OmegaMirrorSnapshot,
} from "../src/workroom/omega-device-bridge-client.ts";

const devicePublicKeyHex = "a".repeat(64);
const hostPublicKeyHex = "b".repeat(64);

class FixtureSocket implements OmegaDeviceBridgeWebSocket {
  readyState = 0;
  readonly sent: Array<Record<string, unknown>> = [];
  readonly closes: Array<Readonly<{ code?: number; reason?: string }>> = [];
  readonly listeners = new Map<string, Array<(event: { readonly data?: unknown }) => void>>();

  addEventListener(
    type: "open" | "message" | "close" | "error",
    listener: (event: { readonly data?: unknown }) => void,
  ): void {
    const listeners = this.listeners.get(type) ?? [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }

  send(data: string): void {
    this.sent.push(JSON.parse(data) as Record<string, unknown>);
  }

  close(code?: number, reason?: string): void {
    this.closes.push({
      ...(code === undefined ? {} : { code }),
      ...(reason === undefined ? {} : { reason }),
    });
    this.readyState = 3;
  }

  emit(type: "open" | "message" | "close" | "error", data?: unknown): void {
    if (type === "open") this.readyState = 1;
    if (type === "close") this.readyState = 3;
    for (const listener of this.listeners.get(type) ?? []) listener({ data });
  }
}

const identityFixture = (close: () => void = () => undefined): Issue31DeviceIdentity => ({
  publicKeyHex: devicePublicKeyHex,
  npub: "npub1fixture",
  close,
  signer: {
    getPublicKey: async () => devicePublicKeyHex,
    signEvent: async (event) => ({
      id: "d".repeat(64),
      pubkey: devicePublicKeyHex,
      created_at: event.created_at ?? 0,
      kind: event.kind,
      tags: event.tags,
      content: event.content,
      sig: "e".repeat(128),
    }),
    nip44Encrypt: async () => "encrypted",
    nip44Decrypt: async () => "decrypted",
  },
});

const snapshotFixture = (sequence = 0): OmegaMirrorSnapshot => ({
  desktopName: "Owner Mac",
  generation: 7,
  sequence,
  threads: [
    {
      threadRef: "thread-1",
      title: "Mobile bridge",
      executor: {
        executorId: "claude-acp",
        executorName: "Claude Code",
        modelId: "claude-opus-5",
        modelName: "Opus 5",
      },
      state: "running",
      transcript: [],
      updatedAt: 1_000,
    },
  ],
  runs: [],
  health: {
    engineUp: true,
    engineGeneration: 7,
    laneReady: true,
    observedAt: 1_000,
  },
  projectedAt: 1_000,
});

const tick = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

describe("openagents.omega.device_bridge.v1 mobile client", () => {
  test("decodes the exact desktop pairing QR payload", () => {
    expect(
      decodeOmegaBridgePairingBootstrap({
        endpoint: "wss://owner-mac.tail:4317",
        hostPublicKeyHex,
        pairingSecret: "one-time-secret",
        expiresAt: 20_000,
      }),
    ).toEqual({
      endpoint: "wss://owner-mac.tail:4317",
      hostPublicKeyHex,
      pairingSecret: "one-time-secret",
      expiresAt: 20_000,
    });
  });

  test("prioritizes an explicit pairing before cached and discovered endpoints", () => {
    const ladder = omegaBridgeDialLadder({
      stored: {
        schemaVersion: 1,
        endpoint: { url: "wss://cached.tail:4317", hostPublicKeyHex },
        grant: null,
        cursor: { generation: 3, sequence: 9 },
      },
      announcements: [
        {
          hostPublicKeyHex,
          generation: 5,
          expiresAt: 2_000,
          endpoints: [{ url: "wss://announced.tail:4317", protocol: OMEGA_DEVICE_BRIDGE_PROTOCOL }],
        },
        {
          hostPublicKeyHex,
          generation: 4,
          expiresAt: 999,
          endpoints: [{ url: "wss://expired.tail:4317", protocol: OMEGA_DEVICE_BRIDGE_PROTOCOL }],
        },
      ],
      pairing: {
        endpoint: "wss://pair.tail:4317",
        hostPublicKeyHex,
        pairingSecret: "one-use",
        expiresAt: 2_000,
      },
      manualMagicDns: "manual.tail",
      now: 1_000,
      defaultPort: 4_317,
    });

    expect(ladder.map((entry) => entry.source)).toEqual(["qr", "cached", "announcement", "manual"]);
    expect(ladder.map((entry) => entry.url)).not.toContain("wss://expired.tail:4317");
  });

  test("uses a fresh pairing secret instead of replaying an older grant", async () => {
    const socket = new FixtureSocket();
    const store = createMemoryOmegaDeviceBridgeStore({
      schemaVersion: 1,
      endpoint: { url: "wss://cached.tail:4317", hostPublicKeyHex },
      grant: {
        grantRef: "grant-old",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 50_000,
      },
      cursor: { generation: 7, sequence: 4 },
    });
    const client = createOmegaDeviceBridgeClient({
      identity: identityFixture(),
      store,
      createSocket: () => socket,
      now: () => 10_000,
      randomNonce: () => "nonce-repair",
      defaultPort: 4_317,
    });

    const connecting = Effect.runPromise(
      client.connect({
        announcements: [],
        pairing: {
          endpoint: "wss://pair.tail:4317",
          hostPublicKeyHex,
          pairingSecret: "fresh-one-use-secret",
          expiresAt: 20_000,
        },
        manualMagicDns: null,
      }),
    );
    socket.emit("open");
    await tick();

    expect(socket.sent[0]).toMatchObject({
      grantRef: null,
      pairingSecret: "fresh-one-use-secret",
    });
    socket.emit(
      "message",
      JSON.stringify({
        type: "grant",
        admitted: true,
        grantRef: "grant-new",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 60_000,
        generation: 8,
      }),
    );
    await connecting;
    expect(store.inspect()?.grant).toMatchObject({
      grantRef: "grant-new",
      generation: 8,
    });
  });

  test("proves the device key, resumes, admits a grant, and keeps the mirror ephemeral", async () => {
    const socket = new FixtureSocket();
    const store = createMemoryOmegaDeviceBridgeStore({
      schemaVersion: 1,
      endpoint: { url: "wss://cached.tail:4317", hostPublicKeyHex },
      grant: {
        grantRef: "grant-1",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 50_000,
      },
      cursor: { generation: 7, sequence: 4 },
    });
    const client = createOmegaDeviceBridgeClient({
      identity: identityFixture(),
      store,
      createSocket: () => socket,
      now: () => 10_000,
      randomNonce: () => "nonce-1",
      defaultPort: 4_317,
    });

    const connecting = Effect.runPromise(
      client.connect({ announcements: [], pairing: null, manualMagicDns: null }),
    );
    socket.emit("open");
    await tick();

    expect(socket.sent[0]).toMatchObject({
      type: "hello",
      protocol: OMEGA_DEVICE_BRIDGE_PROTOCOL,
      devicePublicKeyHex,
      hostPublicKeyHex,
      grantRef: "grant-1",
      pairingSecret: null,
      resumeCursor: { generation: 7, sequence: 4 },
    });
    const proof = socket.sent[0]?.proof as Record<string, unknown>;
    expect(proof.pubkey).toBe(devicePublicKeyHex);
    expect(String(proof.content)).toContain("nonce-1");

    socket.emit(
      "message",
      JSON.stringify({
        type: "grant",
        admitted: true,
        grantRef: "grant-1",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 50_000,
        generation: 7,
      }),
    );
    socket.emit("message", JSON.stringify({ type: "snapshot", snapshot: snapshotFixture(4) }));
    await connecting;
    await tick();

    expect(client.state().connection).toMatchObject({
      state: "direct",
      endpoint: "wss://cached.tail:4317",
      heartbeatAt: 10_000,
    });
    expect(client.state().paired).toBe(true);
    expect(client.state().mirror?.threads[0]?.title).toBe("Mobile bridge");
    expect(store.inspect()?.cursor).toEqual({ generation: 7, sequence: 4 });
    expect(JSON.stringify(store.inspect())).not.toContain("threads");
    expect(JSON.stringify(store.inspect())).not.toContain("Mobile bridge");
  });

  test("accepts only monotonic deltas and asks for a fresh snapshot on a gap", async () => {
    const socket = new FixtureSocket();
    const store = createMemoryOmegaDeviceBridgeStore({
      schemaVersion: 1,
      endpoint: { url: "wss://cached.tail:4317", hostPublicKeyHex },
      grant: {
        grantRef: "grant-1",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 50_000,
      },
      cursor: null,
    });
    const client = createOmegaDeviceBridgeClient({
      identity: identityFixture(),
      store,
      createSocket: () => socket,
      now: () => 10_000,
      randomNonce: () => `nonce-${socket.sent.length + 1}`,
      defaultPort: 4_317,
    });
    const connecting = Effect.runPromise(
      client.connect({ announcements: [], pairing: null, manualMagicDns: null }),
    );
    socket.emit("open");
    await tick();
    socket.emit(
      "message",
      JSON.stringify({
        type: "grant",
        admitted: true,
        grantRef: "grant-1",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 50_000,
        generation: 7,
      }),
    );
    socket.emit("message", JSON.stringify({ type: "snapshot", snapshot: snapshotFixture(0) }));
    await connecting;

    socket.emit(
      "message",
      JSON.stringify({
        type: "delta",
        generation: 7,
        sequence: 1,
        change: {
          type: "health",
          health: {
            engineUp: true,
            engineGeneration: 7,
            laneReady: false,
            observedAt: 1_001,
          },
        },
      }),
    );
    expect(client.state().mirror?.sequence).toBe(1);
    expect(client.state().mirror?.health.laneReady).toBe(false);

    socket.emit(
      "message",
      JSON.stringify({
        type: "delta",
        generation: 7,
        sequence: 3,
        change: {
          type: "thread_remove",
          threadRef: "thread-1",
        },
      }),
    );
    await tick();

    expect(client.state().mirror?.sequence).toBe(1);
    expect(client.state().recovery).toBe("resnapshot_requested");
    expect(store.inspect()?.cursor).toBeNull();
    expect(socket.sent.at(-1)).toMatchObject({
      type: "hello",
      resumeCursor: null,
    });
  });

  test("reports relay and offline honestly and clears a revoked grant", async () => {
    const socket = new FixtureSocket();
    const store = createMemoryOmegaDeviceBridgeStore({
      schemaVersion: 1,
      endpoint: { url: "wss://cached.tail:4317", hostPublicKeyHex },
      grant: {
        grantRef: "grant-1",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 50_000,
      },
      cursor: { generation: 7, sequence: 1 },
    });
    let identityClosed = false;
    const client = createOmegaDeviceBridgeClient({
      identity: identityFixture(() => {
        identityClosed = true;
      }),
      store,
      createSocket: () => socket,
      now: () => 10_000,
      randomNonce: () => "nonce",
      defaultPort: 4_317,
    });

    expect(client.state().connection.state).toBe("offline");
    await Effect.runPromise(client.observeRelay(9_000));
    expect(client.state().connection.state).toBe("relay");

    const connecting = Effect.runPromise(
      client.connect({ announcements: [], pairing: null, manualMagicDns: null }),
    );
    socket.emit("open");
    await tick();
    socket.emit(
      "message",
      JSON.stringify({
        type: "grant",
        admitted: true,
        grantRef: "grant-1",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 50_000,
        generation: 7,
      }),
    );
    await connecting;
    socket.emit("message", JSON.stringify({ type: "snapshot", snapshot: snapshotFixture(1) }));
    expect(client.state().connection.state).toBe("direct");

    socket.emit("message", JSON.stringify({ type: "bye", reason: "grant_revoked" }));
    await tick();
    expect(client.state().connection.state).toBe("relay");
    expect(client.state().connection.staleSince).toBe(1_000);
    expect(client.state().paired).toBe(false);
    expect(store.inspect()?.grant).toBeNull();

    await Effect.runPromise(client.close());
    expect(identityClosed).toBe(true);
  });
});
