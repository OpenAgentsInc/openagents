/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- @effect/vitest does not support the repository Effect 4 line. */
import { Effect, Option, Stream } from "effect";
import { describe, expect, test } from "vite-plus/test";

import type { Issue31DeviceIdentity } from "../src/workroom/issue31-device-key-vault";
import {
  createMemoryOmegaDeviceBridgeStore,
  createOmegaDeviceBridgeClient,
  OMEGA_DEVICE_BRIDGE_PROTOCOL,
  type OmegaDeviceBridgeClient,
  type OmegaDeviceBridgeWebSocket,
  type OmegaMirrorSnapshot,
} from "../src/workroom/omega-device-bridge-client";
import {
  buildOmegaMobileHomeProgram,
  renderOmegaMobileHome,
} from "../src/screens/omega-mobile-home";

const devicePublicKeyHex = "a".repeat(64);
const hostPublicKeyHex = "b".repeat(64);

class JourneySocket implements OmegaDeviceBridgeWebSocket {
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

const identityFixture = (): Issue31DeviceIdentity => ({
  publicKeyHex: devicePublicKeyHex,
  npub: "npub1journeyfixture",
  close: () => undefined,
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

const snapshotFixture = (
  sequence: number,
  input: Readonly<{ engineUp?: boolean; engineGeneration?: number }> = {},
): OmegaMirrorSnapshot => ({
  desktopName: "Owner Mac",
  generation: input.engineGeneration ?? 7,
  sequence,
  threads: [
    {
      threadRef: "thread-journey",
      title: "Mirror journey",
      executor: {
        executorId: "claude-acp",
        executorName: "Claude Code",
        modelId: "claude-opus-5",
        modelName: "Opus 5",
      },
      state: "running",
      transcript: [],
      updatedAt: 9_000,
    },
  ],
  runs: [
    {
      runRef: "run-journey",
      title: "Journey proof",
      lane: "owner",
      state: "running",
      receiptRefs: ["receipt-journey"],
      updatedAt: 8_000,
    },
  ],
  health: {
    engineUp: input.engineUp ?? true,
    engineGeneration: input.engineGeneration ?? 7,
    laneReady: input.engineUp ?? true,
    observedAt: 9_000,
  },
  projectedAt: 9_000,
});

const tick = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

const admit = (socket: JourneySocket, sequence: number, generation = 7): void => {
  socket.emit(
    "message",
    JSON.stringify({
      type: "grant",
      admitted: true,
      grantRef: "grant-journey",
      hostPublicKeyHex,
      devicePublicKeyHex,
      expiresAt: 50_000,
      generation,
    }),
  );
  socket.emit(
    "message",
    JSON.stringify({
      type: "snapshot",
      snapshot: snapshotFixture(sequence, { engineGeneration: generation }),
    }),
  );
};

const currentView = async (
  program: ReturnType<typeof buildOmegaMobileHomeProgram>,
): Promise<string> => {
  const view = await Effect.runPromise(Stream.runHead(program.viewStream));
  if (Option.isNone(view)) throw new Error("The journey view is unavailable.");
  return JSON.stringify(view.value);
};

describe("TM-06 Omega mobile mirror simulator journeys", () => {
  test("M0 pairs by QR and projects the thread list without a selection tap", async () => {
    const socket = new JourneySocket();
    const store = createMemoryOmegaDeviceBridgeStore();
    const bridge = createOmegaDeviceBridgeClient({
      identity: identityFixture(),
      store,
      createSocket: () => socket,
      now: () => 10_000,
      randomNonce: () => "nonce-m0",
      defaultPort: 4_317,
    });
    const program = buildOmegaMobileHomeProgram({
      bridge,
      connectRequest: {
        announcements: [],
        pairing: {
          endpoint: "wss://owner-mac.tail:4317",
          hostPublicKeyHex,
          pairingSecret: "one-time-pairing",
          expiresAt: 20_000,
        },
        manualMagicDns: null,
      },
      scanPairing: async () => null,
      now: () => 10_000,
    });

    socket.emit("open");
    await tick();
    expect(socket.sent[0]).toMatchObject({
      type: "hello",
      protocol: OMEGA_DEVICE_BRIDGE_PROTOCOL,
      pairingSecret: "one-time-pairing",
      resumeCursor: null,
    });
    admit(socket, 1);
    await tick();

    const rendered = await currentView(program);
    expect(rendered).toContain("Owner Mac");
    expect(rendered).toContain("Mirror journey");
    expect(rendered).toContain("Claude Code · Opus 5 · running");
    await program.close();
  });

  test("M1 reopens from the cursor and recovers through the dial ladder after a network change", async () => {
    const store = createMemoryOmegaDeviceBridgeStore({
      schemaVersion: 1,
      endpoint: { url: "wss://old-network.tail:4317", hostPublicKeyHex },
      grant: {
        grantRef: "grant-journey",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 50_000,
      },
      cursor: { generation: 7, sequence: 4 },
    });
    const cachedSocket = new JourneySocket();
    const announcedSocket = new JourneySocket();
    const dialed: Array<string> = [];
    const admissionDeadlines: Array<() => void> = [];
    const bridge = createOmegaDeviceBridgeClient({
      identity: identityFixture(),
      store,
      createSocket: (url) => {
        dialed.push(url);
        return url.includes("old-network") ? cachedSocket : announcedSocket;
      },
      now: () => 10_000,
      randomNonce: () => "nonce-m1",
      defaultPort: 4_317,
      scheduleAdmissionDeadline: (onDeadline) => {
        let active = true;
        admissionDeadlines.push(() => {
          if (active) onDeadline();
        });
        return () => {
          active = false;
        };
      },
    });
    const connecting = Effect.runPromise(
      bridge.connect({
        announcements: [
          {
            hostPublicKeyHex,
            generation: 8,
            expiresAt: 20_000,
            endpoints: [
              {
                url: "wss://new-network.tail:4317",
                protocol: OMEGA_DEVICE_BRIDGE_PROTOCOL,
              },
            ],
          },
        ],
        pairing: null,
        manualMagicDns: null,
      }),
    );

    cachedSocket.emit("open");
    await tick();
    expect(cachedSocket.sent[0]).toMatchObject({
      type: "hello",
      resumeCursor: { generation: 7, sequence: 4 },
    });
    admissionDeadlines[0]?.();
    await tick();
    expect(cachedSocket.closes).toContainEqual({
      code: 1008,
      reason: "admission timeout",
    });
    admit(cachedSocket, 99);
    expect(bridge.state().mirror).toBeNull();

    announcedSocket.emit("open");
    await tick();
    expect(dialed).toEqual(["wss://old-network.tail:4317", "wss://new-network.tail:4317"]);
    expect(announcedSocket.sent[0]).toMatchObject({
      type: "hello",
      grantRef: "grant-journey",
      resumeCursor: { generation: 7, sequence: 4 },
    });
    admit(announcedSocket, 6);
    await connecting;
    await tick();

    expect(bridge.state().mirror?.sequence).toBe(6);
    expect(store.inspect()?.cursor).toEqual({ generation: 7, sequence: 6 });
    await Effect.runPromise(bridge.close());
  });

  test("M2 renders honest relay/offline staleness and returns to direct after an engine restart", async () => {
    const socket = new JourneySocket();
    const store = createMemoryOmegaDeviceBridgeStore({
      schemaVersion: 1,
      endpoint: { url: "wss://owner-mac.tail:4317", hostPublicKeyHex },
      grant: {
        grantRef: "grant-journey",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 50_000,
      },
      cursor: { generation: 7, sequence: 1 },
    });
    const bridge = createOmegaDeviceBridgeClient({
      identity: identityFixture(),
      store,
      createSocket: () => socket,
      now: () => 10_000,
      randomNonce: () => "nonce-m2",
      defaultPort: 4_317,
    });
    const connecting = Effect.runPromise(
      bridge.connect({ announcements: [], pairing: null, manualMagicDns: null }),
    );
    socket.emit("open");
    await tick();
    admit(socket, 1);
    await connecting;
    await Effect.runPromise(bridge.observeRelay(10_100));
    socket.emit("message", JSON.stringify({ type: "bye", reason: "host_shutdown" }));
    await tick();

    expect(bridge.state().connection.state).toBe("relay");
    const relayView = JSON.stringify(
      renderOmegaMobileHome({
        bridge: bridge.state(),
        selectedThreadRef: null,
        observedAt: 12_000,
        notice: null,
      }),
    );
    expect(relayView).toContain("Relay");
    expect(relayView).toContain("Last desktop update 3s ago");

    const recoverySocket = new JourneySocket();
    const recovered: OmegaDeviceBridgeClient = createOmegaDeviceBridgeClient({
      identity: identityFixture(),
      store,
      createSocket: () => recoverySocket,
      now: () => 13_000,
      randomNonce: () => "nonce-m2-recovery",
      defaultPort: 4_317,
    });
    const recovering = Effect.runPromise(
      recovered.connect({ announcements: [], pairing: null, manualMagicDns: null }),
    );
    recoverySocket.emit("open");
    await tick();
    admit(recoverySocket, 1, 8);
    await recovering;
    await tick();

    expect(recovered.state().connection.state).toBe("direct");
    expect(recovered.state().mirror?.generation).toBe(8);
    expect(recovered.state().mirror?.health.engineGeneration).toBe(8);
    await Effect.runPromise(recovered.close());
    await Effect.runPromise(bridge.close());
  });

  test("revocation clears the shared grant and refuses the next direct admission", async () => {
    const socket = new JourneySocket();
    const store = createMemoryOmegaDeviceBridgeStore({
      schemaVersion: 1,
      endpoint: { url: "wss://owner-mac.tail:4317", hostPublicKeyHex },
      grant: {
        grantRef: "grant-journey",
        hostPublicKeyHex,
        devicePublicKeyHex,
        expiresAt: 50_000,
      },
      cursor: { generation: 7, sequence: 1 },
    });
    const bridge = createOmegaDeviceBridgeClient({
      identity: identityFixture(),
      store,
      createSocket: () => socket,
      now: () => 10_000,
      randomNonce: () => "nonce-revocation",
      defaultPort: 4_317,
    });
    const connecting = Effect.runPromise(
      bridge.connect({ announcements: [], pairing: null, manualMagicDns: null }),
    );
    socket.emit("open");
    await tick();
    admit(socket, 1);
    await connecting;
    socket.emit("message", JSON.stringify({ type: "bye", reason: "grant_revoked" }));
    await tick();

    expect(store.inspect()?.grant).toBeNull();
    expect(bridge.state().paired).toBe(false);
    expect(bridge.state().refusal).toBe("grant_revoked");
    await Effect.runPromise(bridge.close());
  });
});
