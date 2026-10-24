import assert from "node:assert/strict";
import test from "node:test";
import fixture from "./fixtures/mmorpg-terrain.json";
import {
  mmorpgHeight,
  mmorpgGroundHeight,
  sampleMmorpgNoise,
} from "../packages/runtime/mmorpg-terrain.ts";

test("world terrain matches the original seed-one JavaScript height generator", () => {
  for (const { x, z, height } of fixture.samples)
    assert.ok(Math.abs(mmorpgHeight(x, z) - height) < 1e-10, `${x},${z}`);
});
test("moving terrain patches keep globally aligned vertices and triangle interpolation", () => {
  for (const [x, z] of [
    [-10, -20],
    [0, 0],
    [80, -50],
    [250, 320],
  ])
    assert.equal(mmorpgGroundHeight(x, z), mmorpgHeight(x, z));
  const a = mmorpgHeight(0, 0),
    b = mmorpgHeight(2, 0),
    c = mmorpgHeight(0, 2);
  assert.ok(
    Math.abs(mmorpgGroundHeight(0.5, 0.5) - (a * 0.5 + b * 0.25 + c * 0.25)) <
      1e-12,
  );
});
test("generic source noise retains the terrain's unusual persistence exponent", () => {
  const params = {
    seed: 1 as const,
    octaves: 10,
    persistence: 0.5,
    lacunarity: 1.6,
    exponentiation: 7.5,
    scale: 1800,
    height: 800,
  };
  for (const { x, z, height } of fixture.samples)
    assert.ok(
      Math.abs(sampleMmorpgNoise(params, x * 6, 0, z * 6) / 6 - height) < 1e-10,
    );
});
