import { Effect } from "effect";
import { useFetchImplementation } from "nostr-effect/nip11";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { probeImmortalRelay, RelayProbeError } from "./relay.js";

const originalFetch = globalThis.fetch;
const OriginalWebSocket = globalThis.WebSocket;

class FakeWebSocket {
  onopen: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onmessage: ((event: { readonly data: string }) => void) | null = null;

  constructor(readonly url: string) {
    queueMicrotask(() => this.onopen?.());
  }

  send(message: string): void {
    const parsed = JSON.parse(message) as readonly unknown[];
    if (parsed[0] === "REQ") {
      const subscriptionId = parsed[1];
      queueMicrotask(() => this.onmessage?.({ data: JSON.stringify(["EOSE", subscriptionId]) }));
    }
  }

  close(): void {
    queueMicrotask(() => this.onclose?.());
  }
}

function installRelayInformation(supportedExtensions: readonly string[]): void {
  const relayInformationFetch = async () =>
    new Response(
      JSON.stringify({
        name: "OpenAgents Relay",
        software: "https://github.com/OpenAgentsInc/immortal",
        version: "0.0.1",
        supported_nips: [1, 11],
        supported_extensions: supportedExtensions,
      }),
      { status: 200, headers: { "Content-Type": "application/nostr+json" } },
    );
  globalThis.fetch = relayInformationFetch;
  useFetchImplementation(relayInformationFetch);
  globalThis.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
}

afterEach(() => {
  globalThis.fetch = originalFetch;
  useFetchImplementation(originalFetch);
  globalThis.WebSocket = OriginalWebSocket;
});

describe("Immortal relay probe", () => {
  test("completes NIP-11, WebSocket, REQ, and EOSE", async () => {
    installRelayInformation(["nip-mkt", "mkt-swp:1"]);

    // This package's Vite Plus TestAPI does not expose an Effect test extension.
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests
    const result = await Effect.runPromise(
      probeImmortalRelay("wss://relay.openagents.com", { timeoutMs: 1_000 }),
    );

    expect(result).toMatchObject({
      relayUrl: "wss://relay.openagents.com",
      relayName: "OpenAgents Relay",
      relayVersion: "0.0.1",
      contractVersion: "0.0.1",
      websocketConnected: true,
      snapshotComplete: true,
      validatedEvents: 0,
    });
    expect(result.supportedExtensions).toContain("mkt-swp:1");
  });

  test("fails closed when the relay does not advertise NIP-MKT", async () => {
    installRelayInformation([]);

    // This package's Vite Plus TestAPI does not expose an Effect test extension.
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests
    const error = await Effect.runPromise(
      Effect.flip(probeImmortalRelay("wss://relay.openagents.com", { timeoutMs: 1_000 })),
    );

    expect(error).toBeInstanceOf(RelayProbeError);
    expect(error.code).toBe("nip_mkt_unavailable");
  });
});
