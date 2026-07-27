/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- @effect/vitest does not support the repository Effect 4 line. */
import { Effect, Option, Stream } from "effect";
import { describe, expect, test } from "vite-plus/test";
import { IntentRef, StaticPayload } from "@effect-native/core";

import {
  buildOmegaMobileHomeProgram,
  renderOmegaMobileHome,
  type OmegaMobileHomeState,
} from "../src/screens/omega-mobile-home";
import type {
  OmegaDeviceBridgeClient,
  OmegaDeviceBridgeState,
} from "../src/workroom/omega-device-bridge-client";

const automaticActivityContract = "openagents_mobile.home_automatic_desktop_activity.v1";
const honestConnectionContract = "openagents_mobile.home_honest_connection_state.v1";

const bridgeState = (connection: "direct" | "relay" | "offline"): OmegaDeviceBridgeState => ({
  paired: true,
  connection: {
    state: connection,
    endpoint: connection === "direct" ? "wss://owner-mac.tail:4317" : null,
    heartbeatAt: connection === "direct" ? 2_000 : null,
    relayObservedAt: connection === "relay" ? 1_900 : null,
    staleSince: connection === "direct" ? null : 1_000,
  },
  mirror: {
    desktopName: "Owner Mac",
    generation: 2,
    sequence: 8,
    threads: [
      {
        threadRef: "thread-recent",
        title: "Ship the mobile mirror",
        executor: {
          executorId: "claude-acp",
          executorName: "Claude Code",
          modelId: "claude-opus-5",
          modelName: "Opus 5",
        },
        state: "running",
        transcript: [
          {
            messageRef: "message-1",
            role: "assistant",
            text: "The bridge is live.",
            createdAt: 1_900,
          },
        ],
        updatedAt: 1_900,
      },
      {
        threadRef: "thread-old",
        title: "Older task",
        executor: {
          executorId: "codex-acp",
          executorName: "Codex",
          modelId: null,
          modelName: null,
        },
        state: "completed",
        transcript: [],
        updatedAt: 1_100,
      },
    ],
    runs: [
      {
        runRef: "run-1",
        title: "Mobile release proof",
        lane: "owner",
        state: "queued",
        receiptRefs: [],
        updatedAt: 1_500,
      },
    ],
    health: {
      engineUp: true,
      engineGeneration: 2,
      laneReady: true,
      observedAt: 1_900,
    },
    projectedAt: 1_900,
  },
  recovery: "none",
  refusal: null,
});

const homeState = (bridge: OmegaDeviceBridgeState): OmegaMobileHomeState => ({
  bridge,
  selectedThreadRef: null,
  observedAt: 2_000,
  notice: null,
});

const fakeBridge = (initial: OmegaDeviceBridgeState) => {
  let current = initial;
  const listeners = new Set<(state: OmegaDeviceBridgeState) => void>();
  const client: OmegaDeviceBridgeClient = {
    state: () => current,
    subscribe: (listener) => {
      listeners.add(listener);
      listener(current);
      return () => listeners.delete(listener);
    },
    observeRelay: () => Effect.void,
    connect: () => Effect.void,
    close: () => Effect.void,
  };
  return {
    client,
    emit: (next: OmegaDeviceBridgeState): void => {
      current = next;
      for (const listener of listeners) listener(next);
    },
  };
};

const currentView = async (program: ReturnType<typeof buildOmegaMobileHomeProgram>) => {
  const option = await Effect.runPromise(Stream.runHead(program.viewStream));
  if (Option.isNone(option)) throw new Error("The mobile home view is unavailable.");
  return option.value;
};

describe("Omega zero-based mobile home", () => {
  test("renders one functional pairing action and none of the removed home surfaces", () => {
    const serialized = JSON.stringify(
      renderOmegaMobileHome({
        ...homeState({
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
        }),
      }),
    );

    expect(serialized).toContain("Scan desktop QR");
    expect(serialized).not.toContain("Khala");
    expect(serialized).not.toContain("Sarah");
    expect(serialized).not.toContain("Full Auto");
    expect(serialized).not.toContain("managed sandbox");
    expect(serialized).not.toContain("terminal");
    expect(serialized).not.toContain("settings");
  });

  test(`${automaticActivityContract}: projects new mirror activity without a tap`, async () => {
    const bridge = fakeBridge({
      paired: true,
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
    });
    const program = buildOmegaMobileHomeProgram({
      bridge: bridge.client,
      connectRequest: {
        announcements: [],
        pairing: null,
        manualMagicDns: null,
      },
      scanPairing: async () => null,
      now: () => 2_000,
    });

    bridge.emit(bridgeState("direct"));
    await new Promise((resolve) => setTimeout(resolve, 0));
    const next = await currentView(program);
    const serialized = JSON.stringify(next);

    expect(serialized).toContain("Owner Mac");
    expect(serialized).toContain("Ship the mobile mirror");
    expect(serialized).toContain("Claude Code · Opus 5 · running");
    expect(serialized.indexOf("Ship the mobile mirror")).toBeLessThan(
      serialized.indexOf("Mobile release proof"),
    );
    expect(serialized.indexOf("Mobile release proof")).toBeLessThan(
      serialized.indexOf("Older task"),
    );
    await program.close();
  });

  test(`${honestConnectionContract}: keeps direct, relay, and offline evidence in the header`, () => {
    for (const connection of ["direct", "relay", "offline"] as const) {
      const serialized = JSON.stringify(renderOmegaMobileHome(homeState(bridgeState(connection))));
      const expected =
        connection === "direct" ? "Direct" : connection === "relay" ? "Relay" : "Offline";
      expect(serialized).toContain(expected);
      expect(serialized).toContain(
        connection === "direct" ? "Live from your desktop" : "Last desktop update 1s ago",
      );
    }
  });

  test("opens only a read-only thread view behind the activity tap", async () => {
    const bridge = fakeBridge(bridgeState("direct"));
    const program = buildOmegaMobileHomeProgram({
      bridge: bridge.client,
      connectRequest: {
        announcements: [],
        pairing: null,
        manualMagicDns: null,
      },
      scanPairing: async () => null,
      now: () => 2_000,
    });

    await Effect.runPromise(
      program.report(
        IntentRef("OmegaActivitySelected", StaticPayload({ threadRef: "thread-recent" })),
      ) as Effect.Effect<void>,
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    const selected = await currentView(program);
    const serialized = JSON.stringify(selected);

    expect(serialized).toContain("The bridge is live.");
    expect(serialized).toContain("Read only");
    expect(serialized).toContain("Back to activity");
    expect(serialized).not.toContain("TextField");
    expect(serialized).not.toContain("Composer");
    await program.close();
  });
});
