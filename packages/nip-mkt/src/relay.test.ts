import { Effect } from "effect";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { probeImmortalRelay, RelayProbeError } from "./relay.js";

const originalFetch = globalThis.fetch;
const OriginalWebSocket = globalThis.WebSocket;

class FakeWebSocket {
  private openListener: (() => void) | undefined;
  private closeListener: (() => void) | undefined;
  private messageListener: ((event: { readonly data: string }) => void) | undefined;

  constructor(readonly url: string) {
    queueMicrotask(() => this.openListener?.());
  }

  addEventListener(type: string, listener: (event: { readonly data: string }) => void): void {
    if (type === "open") this.openListener = () => listener({ data: "" });
    else if (type === "close") this.closeListener = () => listener({ data: "" });
    else if (type === "message") this.messageListener = listener;
  }

  send(message: string): void {
    const parsed = JSON.parse(message) as readonly unknown[];
    if (parsed[0] === "REQ") {
      const subscriptionId = parsed[1];
      queueMicrotask(() =>
        this.messageListener?.({ data: JSON.stringify(["EOSE", subscriptionId]) }),
      );
    }
  }

  close(): void {
    queueMicrotask(() => this.closeListener?.());
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
  globalThis.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
}

afterEach(() => {
  globalThis.fetch = originalFetch;
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
