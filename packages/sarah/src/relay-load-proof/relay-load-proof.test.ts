import { afterAll, describe, expect, test } from "vite-plus/test";
import {
  evaluateThresholds,
  latencyStats,
  percentile,
} from "./metrics.js";
import { startMockRelay } from "./mock-relay.js";
import { runLoadProof, startLocalLoadProofHost } from "./harness.js";
import {
  LOCAL_LOAD_PROOF_THRESHOLDS,
  NOSTR_EFFECT_NODE_EXPORTS,
  NOSTR_EFFECT_NODE_PIN,
} from "./types.js";
import { createSignedEvent, generatePrivateKeyHex } from "./event.js";
import {
  connectRelay,
  publishEvent,
  subscribeOnce,
} from "./client.js";

describe("SARAH-NR-03 relay load proof", () => {
  test("pins the nostr-effect Node host entry", () => {
    expect(NOSTR_EFFECT_NODE_PIN).toMatch(/^[0-9a-f]{40}$/);
    expect(NOSTR_EFFECT_NODE_EXPORTS.host).toBe("nostr-effect/relay/node");
    expect(NOSTR_EFFECT_NODE_EXPORTS.postgres).toBe(
      "nostr-effect/relay/node/postgres",
    );
    expect(NOSTR_EFFECT_NODE_EXPORTS.sqlite).toBe(
      "nostr-effect/relay/node/sqlite",
    );
    expect(NOSTR_EFFECT_NODE_EXPORTS.entry).toBe("src/relay/main.ts");
  });

  test("computes latency percentiles", () => {
    const stats = latencyStats([10, 20, 30, 40, 50, 100]);
    expect(stats.medianMs).toBeGreaterThan(0);
    expect(stats.p99Ms).toBeGreaterThanOrEqual(stats.medianMs);
    expect(percentile([1, 2, 3, 4, 5], 50)).toBe(3);
  });

  test("fails closed when thresholds are not met", () => {
    const result = evaluateThresholds({
      publish: {
        phase: "publish",
        attempts: 10,
        successes: 1,
        failures: 9,
        rps: 1,
        latency: { count: 1, medianMs: 5_000, p99Ms: 9_000, minMs: 5_000, maxMs: 5_000 },
        errorClasses: {
          connect_failed: 0,
          timeout: 9,
          ok_false: 0,
          protocol_error: 0,
          closed: 0,
          other: 0,
        },
      },
      subscribe: {
        phase: "subscribe",
        attempts: 10,
        successes: 1,
        failures: 9,
        rps: 1,
        latency: { count: 1, medianMs: 5_000, p99Ms: 9_000, minMs: 5_000, maxMs: 5_000 },
        errorClasses: {
          connect_failed: 0,
          timeout: 9,
          ok_false: 0,
          protocol_error: 0,
          closed: 0,
          other: 0,
        },
      },
      thresholds: LOCAL_LOAD_PROOF_THRESHOLDS,
    });
    expect(result.pass).toBe(false);
    expect(result.failures.length).toBeGreaterThan(0);
  });

  test("mock relay accepts EVENT and answers REQ with EOSE", async () => {
    const mock = await startMockRelay();
    try {
      const socket = await connectRelay(mock.url, 2_000);
      const sk = generatePrivateKeyHex();
      const event = createSignedEvent({ privateKeyHex: sk, content: "hello" });
      // Drain AUTH
      try {
        await socket.waitFor((m) => m[0] === "AUTH", 500);
      } catch {
        /* optional */
      }
      const publishMs = await publishEvent(socket, event, 2_000);
      expect(publishMs).toBeGreaterThanOrEqual(0);
      const subMs = await subscribeOnce(
        socket,
        { authors: [event.pubkey], kinds: [1], limit: 5 },
        2_000,
      );
      expect(subMs).toBeGreaterThanOrEqual(0);
      socket.close();
    } finally {
      await mock.stop();
    }
  });

  test("local load proof meets thresholds on mock host", async () => {
    const host = await startLocalLoadProofHost({ preferMock: true });
    try {
      const report = await runLoadProof({
        relayUrl: host.relayUrl,
        hostMode: host.mode,
        nostrEffectPin: host.nostrEffectPin,
        config: {
          durationMs: 1_500,
          publishers: 2,
          subscribers: 1,
          publishIntervalMs: 15,
        },
      });
      expect(report.schema).toBe("openagents.sarah.relay_load_proof.v1");
      expect(report.packet).toBe("SARAH-NR-03");
      expect(report.hostMode).toBe("mock");
      expect(report.publish.successes).toBeGreaterThan(0);
      expect(report.subscribe.successes).toBeGreaterThan(0);
      expect(report.pass).toBe(true);
      if (!report.pass) {
        console.error(report.failures);
      }
    } finally {
      await host.stop();
    }
  });
});

// Keep a process-level safety so vite-plus does not hang on open handles.
afterAll(async () => {
  await new Promise((r) => setTimeout(r, 10));
});
