import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { startOmegaBridgeSession } from "../src/screens/omega-bridge-session.ts";
import type {
  OmegaDeviceBridgeClient,
  OmegaDeviceBridgeState,
} from "../src/workroom/omega-device-bridge-client.ts";

const offlineState: OmegaDeviceBridgeState = {
  paired: false,
  connection: {
    state: "offline",
    endpoint: null,
    heartbeatAt: null,
    relayObservedAt: null,
    staleSince: null,
  },
  mirror: null,
  recovery: "none",
  refusal: null,
};

interface FixtureClient extends OmegaDeviceBridgeClient {
  readonly closeCount: () => number;
  readonly subscriberCount: () => number;
  readonly connectCount: () => number;
}

const fixtureClient = (): FixtureClient => {
  let closes = 0;
  let connects = 0;
  const listeners = new Set<(state: OmegaDeviceBridgeState) => void>();
  return {
    state: () => offlineState,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    observeRelay: () => Effect.void,
    connect: () =>
      Effect.sync(() => {
        connects += 1;
      }),
    // The real client counts a repeat close as a no-op, so the fixture counts
    // every call instead: a double close is the ownership bug, not a nuisance.
    close: () =>
      Effect.sync(() => {
        closes += 1;
      }),
    closeCount: () => closes,
    subscriberCount: () => listeners.size,
    connectCount: () => connects,
  };
};

/** Drains every pending microtask, which is where the whole race lives. */
const settle = (): Promise<void> => new Promise<void>((done) => setImmediate(done));

describe("startOmegaBridgeSession", () => {
  test("closes a bridge that opens after teardown, exactly once", async () => {
    const client = fixtureClient();
    // The open stays pending until the teardown has already run.
    const open = Promise.withResolvers<OmegaDeviceBridgeClient>();
    const notices: Array<string> = [];
    let handed: OmegaDeviceBridgeClient | null = null;

    const stop = startOmegaBridgeSession({
      bridge: undefined,
      openBridge: () => open.promise,
      pairing: async () => null,
      onClient: (next) => {
        handed = next;
      },
      onState: () => undefined,
      onNotice: (notice) => notices.push(notice),
    });

    stop();
    open.resolve(client);
    await settle();

    expect(client.closeCount()).toBe(1);
    expect(handed).toBeNull();
    expect(client.subscriberCount()).toBe(0);
    expect(notices).toEqual([]);
  });

  test("never closes a caller-injected bridge", async () => {
    const client = fixtureClient();

    const stop = startOmegaBridgeSession({
      bridge: client,
      openBridge: () => Promise.reject(new Error("must not open a bridge")),
      pairing: async () => null,
      onClient: () => undefined,
      onState: () => undefined,
      onNotice: () => undefined,
    });

    await settle();
    stop();
    await settle();

    expect(client.closeCount()).toBe(0);
    expect(client.connectCount()).toBe(1);
    expect(client.subscriberCount()).toBe(0);
  });

  test("connects, subscribes, and closes its own bridge on teardown", async () => {
    const client = fixtureClient();
    const states: Array<OmegaDeviceBridgeState> = [];
    let handed: OmegaDeviceBridgeClient | null = null;

    const stop = startOmegaBridgeSession({
      bridge: undefined,
      openBridge: async () => client,
      pairing: async () => null,
      onClient: (next) => {
        handed = next;
      },
      onState: (next) => states.push(next),
      onNotice: () => undefined,
    });

    await settle();

    expect(handed).toBe(client);
    expect(client.connectCount()).toBe(1);
    expect(client.subscriberCount()).toBe(1);

    stop();
    await settle();

    expect(client.closeCount()).toBe(1);
    expect(client.subscriberCount()).toBe(0);
    expect(states).toEqual([]);
  });

  test("reports the bridge as unavailable when the open fails", async () => {
    const notices: Array<string> = [];

    const stop = startOmegaBridgeSession({
      bridge: undefined,
      openBridge: () => Promise.reject(new Error("no vault")),
      pairing: async () => null,
      onClient: () => undefined,
      onState: () => undefined,
      onNotice: (notice) => notices.push(notice),
    });

    await settle();
    stop();

    expect(notices).toEqual(["The Omega device bridge is unavailable."]);
  });
});
