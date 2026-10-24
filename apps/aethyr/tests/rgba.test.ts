import assert from "node:assert/strict";
import test from "node:test";
import { decodeRgba8 } from "../packages/assets/src/rgba.ts";
const fixture = () => {
  const buffer = new ArrayBuffer(20),
    header = new DataView(buffer);
  header.setUint32(0, 0x41465238, false);
  header.setUint32(4, 1, true);
  header.setUint32(8, 1, true);
  header.setUint32(12, 1, true);
  new Uint8Array(buffer, 16).set([64, 128, 192, 0]);
  return buffer;
};
test("straight-alpha map decoding preserves RGB independently of zero alpha", () => {
  const map = decodeRgba8(fixture());
  assert.equal(map.width, 1);
  assert.equal(map.height, 1);
  assert.deepEqual([...map.pixels], [64, 128, 192, 0]);
});
test("truncated, wrong-version, oversized, and trailing map data fail", () => {
  assert.throws(() => decodeRgba8(new ArrayBuffer(4)), /header/);
  const bad = fixture();
  new DataView(bad).setUint32(4, 2, true);
  assert.throws(() => decodeRgba8(bad), /header/);
  const large = fixture();
  new DataView(large).setUint32(8, 4097, true);
  assert.throws(() => decodeRgba8(large), /dimensions/);
  assert.throws(() => decodeRgba8(fixture().slice(0, 19)), /dimensions/);
  const padded = new Uint8Array(21);
  padded.set(new Uint8Array(fixture()));
  assert.throws(() => decodeRgba8(padded.buffer), /dimensions/);
});
