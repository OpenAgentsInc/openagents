import assert from "node:assert/strict";
import test from "node:test";
import {
  validSpellSwarmCount,
  validateAgentAction,
} from "../packages/runtime/agent-contract.ts";
import { validateCTownRequest } from "../experiments/c-town/metrics.ts";
import {
  INITIAL_STATUS,
  PREVIEW_PROTOCOL,
  isPreviewEvent,
} from "../packages/editor-services/protocol.ts";

test("spell swarm actions accept only explicit supported counts and reject extra authority", () => {
  for (const count of [0, 10000, 100000, 200000]) {
    assert.equal(validSpellSwarmCount(count), true);
    const action = { type: "spellSwarm", count };
    const checked = validateAgentAction(action);
    assert.deepEqual(checked, action);
    assert.notEqual(checked, action);
  }
  for (const count of [
    -1,
    1,
    100000.5,
    200001,
    NaN,
    Infinity,
    "100000",
    null,
  ]) {
    assert.equal(validSpellSwarmCount(count), false);
    assert.throws(() => validateAgentAction({ type: "spellSwarm", count }));
  }
  for (const extra of [
    { shader: "untrusted" },
    { center: [0, 0, 0] },
    { countOverride: 1000000 },
  ])
    assert.throws(() =>
      validateAgentAction({ type: "spellSwarm", count: 100000, ...extra }),
    );
});

test("swarm readback has a closed diagnostic request and cannot start or reconfigure recording", () => {
  assert.doesNotThrow(() => validateCTownRequest({ op: "readSwarm" }));
  for (const extra of [
    { count: 100000 },
    { capacity: 10 },
    { route: "sway" },
    { drain: true },
    { targetHz: "display" },
  ])
    assert.throws(() =>
      validateCTownRequest({ op: "readSwarm", ...extra } as never),
    );
  const array = Object.assign([], { op: "readSwarm" });
  assert.throws(() => validateCTownRequest(array as never));
  assert.throws(() => validateCTownRequest(null as never));
  assert.doesNotThrow(() =>
    validateCTownRequest({ op: "start", targetHz: "display", capacity: 2000 }),
  );
});

test("swarm UI summaries require bounded counts and a consistent enabled state", () => {
  const event = (spellSwarm: unknown) => ({
    type: "status",
    protocol: PREVIEW_PROTOCOL,
    sessionId: "swarm-test",
    status: { ...INITIAL_STATUS, spellSwarm },
  });
  for (const count of [0, 10000, 100000, 200000])
    assert.equal(
      isPreviewEvent(event({ count, enabled: count > 0 }), "swarm-test"),
      true,
    );
  for (const value of [
    { count: 1, enabled: true },
    { count: 0, enabled: true },
    { count: 100000, enabled: false },
    { count: 100000, enabled: true, positions: [] },
    { count: 100000 },
    null,
    [],
  ])
    assert.equal(isPreviewEvent(event(value), "swarm-test"), false);
});
