import assert from "node:assert/strict";
import test from "node:test";
import { planarLightingChanged } from "../packages/renderer/planar-lighting-state.ts";

const uniform = () => new Float32Array(1428);
function detectors() {
  const previous = new Float32Array(556);
  let oldSignature = "",
    oldRevision = 0,
    newRevision = 0;
  return {
    previous,
    step(u: Float32Array) {
      const signature = Array.from(u.subarray(52, 608)).join(",");
      if (signature !== oldSignature) {
        oldSignature = signature;
        oldRevision++;
      }
      if (planarLightingChanged(previous, u) || newRevision === 0)
        newRevision++;
      assert.equal(newRevision, oldRevision);
      return newRevision;
    },
  };
}
test("first eligible frame increments once, including all-zero input and signed-zero changes", () => {
  const d = detectors(),
    u = uniform(),
    buffer = d.previous.buffer;
  assert.equal(d.step(u), 1);
  assert.equal(d.step(u), 1);
  u.fill(-0, 52, 608);
  assert.equal(d.step(u), 1);
  u[52] = 1;
  assert.equal(d.step(u), 2);
  assert.equal(d.step(u), 2);
  u[52] = 0;
  assert.equal(d.step(u), 3);
  assert.equal(d.previous.buffer, buffer, "Detector storage stays fixed.");
});
test("slice boundaries and stale unused lighting slots retain original revision behavior", () => {
  const d = detectors(),
    u = uniform();
  d.step(u);
  for (const index of [0, 48, 51, 608, 1248]) {
    u[index] = 37;
    assert.equal(d.step(u), 1);
  }
  for (const index of [
    52, 55, 63, 80, 207, 208, 335, 336, 351, 352, 479, 480, 607,
  ]) {
    const before = d.step(u);
    u[index] = Math.fround(index * 0.013);
    assert.equal(d.step(u), before + 1);
  }
});
test("NaN payloads and infinities compare like their original number strings", () => {
  const d = detectors(),
    u = uniform(),
    bits = new Uint32Array(u.buffer);
  let revision = d.step(u);
  for (const bitsValue of [0x7fc00001, 0xffc00002, 0x7fffffff]) {
    bits[52] = bitsValue;
    assert.equal(d.step(u), 2);
    revision = 2;
  }
  u[52] = Infinity;
  assert.equal(d.step(u), ++revision);
  assert.equal(d.step(u), revision);
  u[52] = -Infinity;
  assert.equal(d.step(u), ++revision);
  assert.equal(d.step(u), revision);
  u[52] = Number.NaN;
  assert.equal(d.step(u), ++revision);
  assert.equal(d.step(u), revision);
});
test("snapshot is copied, never borrowed from a subsequently changed source buffer", () => {
  const d = detectors(),
    u = uniform();
  u[607] = 8;
  d.step(u);
  u[607] = 9;
  assert.equal(d.previous[555], 8);
  assert.equal(d.step(u), 2);
  assert.equal(d.previous[555], 9);
});
test("random float32 bit patterns and one-ULP mutations match original signatures", () => {
  const d = detectors(),
    u = uniform(),
    bits = new Uint32Array(u.buffer);
  let seed = 0x12500;
  const next = () => {
    seed ^= seed << 13;
    seed ^= seed >>> 17;
    seed ^= seed << 5;
    return seed >>> 0;
  };
  for (let frame = 0; frame < 1600; frame++) {
    if (frame % 7 === 0) for (let i = 52; i < 608; i++) bits[i] = next();
    else if (frame % 3 === 0) bits[52 + (next() % 556)]++;
    d.step(u);
    d.step(u);
  }
});
test("every one of the556 lighting words participates without reading neighboring values", () => {
  const d = detectors(),
    u = uniform();
  d.step(u);
  for (let i = 0; i < 556; i++) {
    u[i + 52] = Math.fround(1 + i / 17);
    assert.equal(d.step(u), i + 2);
    assert.equal(d.step(u), i + 2);
  }
  u[51] = Infinity;
  u[608] = NaN;
  assert.equal(d.step(u), 557);
});
