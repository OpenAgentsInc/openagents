import { describe, expect, test } from "vite-plus/test";

import {
  backoffDelayMs,
  DEFAULT_RECONNECT_POLICY,
  INITIAL_TRANSPORT_STATE,
  pollChunks,
  reduceTransportEvent,
  shouldEmit,
} from "./reconnect.js";

const policy = DEFAULT_RECONNECT_POLICY;

describe("socket discipline (teardown §6 mechanics)", () => {
  test("backoff is exponential 1s..30s with bounded jitter", () => {
    expect(backoffDelayMs(0, policy, 0)).toBe(1_000);
    expect(backoffDelayMs(1, policy, 0)).toBe(2_000);
    expect(backoffDelayMs(2, policy, 0)).toBe(4_000);
    expect(backoffDelayMs(10, policy, 0)).toBe(30_000);
    const jittered = backoffDelayMs(0, policy, 0.999);
    expect(jittered).toBeGreaterThan(1_000);
    expect(jittered).toBeLessThan(1_500.5);
  });

  test("three failed connects degrade to polling with the fallback engaged", () => {
    let state = INITIAL_TRANSPORT_STATE;
    for (let failure = 0; failure < 2; failure += 1) {
      state = reduceTransportEvent(state, { kind: "connect_failed" }, policy).state;
      expect(state.mode).toBe("socket");
    }
    state = reduceTransportEvent(state, { kind: "connect_timeout" }, policy).state;
    expect(state.mode).toBe("polling");
    expect(state.fallbackEngaged).toBe(true);
  });

  test("a stable frame resets backoff and disengages the fallback; a quick frame does not", () => {
    let state = INITIAL_TRANSPORT_STATE;
    for (let failure = 0; failure < 3; failure += 1) {
      state = reduceTransportEvent(state, { kind: "connect_failed" }, policy).state;
    }
    state = reduceTransportEvent(state, { kind: "open" }, policy).state;
    const quick = reduceTransportEvent(state, { kind: "frame", msAfterOpen: 500 }, policy);
    expect(quick.state.fallbackEngaged).toBe(true);
    const stable = reduceTransportEvent(
      state,
      { kind: "frame", msAfterOpen: policy.stabilityResetMs },
      policy,
    );
    expect(stable.state.attempt).toBe(0);
    expect(stable.state.consecutiveFailures).toBe(0);
    expect(stable.state.fallbackEngaged).toBe(false);
    expect(stable.state.mode).toBe("socket");
  });

  test("an unanswered ping forces reconnect (half-open detection); open resubscribes", () => {
    const ping = reduceTransportEvent(
      INITIAL_TRANSPORT_STATE,
      { kind: "ping_unanswered" },
      policy,
    );
    expect(ping.forceReconnect).toBe(true);
    const open = reduceTransportEvent(INITIAL_TRANSPORT_STATE, { kind: "open" }, policy);
    expect(open.resubscribe).toBe(true);
  });

  test("polling emits only on change and chunks bulk requests at 64", () => {
    expect(shouldEmit(null, "a")).toBe(true);
    expect(shouldEmit("a", "a")).toBe(false);
    expect(shouldEmit("a", "b")).toBe(true);
    const ids = Array.from({ length: 130 }, (_, index) => `id-${index}`);
    const chunks = pollChunks(ids, policy);
    expect(chunks.map((chunk) => chunk.length)).toEqual([64, 64, 2]);
  });
});
