import assert from "node:assert/strict";
import test from "node:test";
import type { BattleCrowdTerrain } from "../packages/renderer/battle-crowd.ts";
import { battleTerrainHeight } from "../packages/renderer/battle-crowd.ts";
import { normalize, type V3 } from "../packages/renderer/math.ts";
import {
  battleMeteorTarget,
  battleTerrainRaycast,
} from "../packages/runtime/battle-spell-targeting.ts";

function field(
  height: (x: number, z: number) => number = () => 0,
): BattleCrowdTerrain {
  const position = (i: number) => {
    const offset = i - 128,
      n = Math.abs(offset);
    return Math.sign(offset) * (n <= 64 ? n * 2 : 128 + (n - 64) * 24);
  };
  return {
    layout: "mmorpg-patch",
    resolution: 257,
    size: 3328,
    center: [0, 0],
    heights: Float32Array.from({ length: 257 ** 2 }, (_, i) =>
      height(position(i % 257), position(Math.floor(i / 257))),
    ),
  };
}

test("meteor picking hits the exact displayed flat terrain rather than the character", () => {
  const target = battleMeteorTarget(
    field(),
    { eye: [0, 1.62, 0], yaw: 0, pitch: -0.2, fov: Math.PI / 3 },
    16 / 9,
    { x: 0, y: 0, locked: true },
    [0, 1.62, 0],
    false,
  )!;
  assert(target);
  assert(Math.abs(target.point[1]) < 1e-6);
  assert(Math.abs(target.point[2] + 1.62 / Math.tan(0.2)) < 1e-6);
  assert(target.normal.every((n, i) => Math.abs(n - [0, 1, 0][i]) < 1e-8));
  assert(target.distance < 80);
});

test("terrain traversal covers fine/coarse seams, mixed cells, and reverse directions", () => {
  const terrain = field((x, z) => 0.03 * x + 0.02 * z);
  for (const [origin, delta] of [
    [
      [120, 15, 2],
      [1, -0.4, 0.15],
    ],
    [
      [160, 15, 2],
      [-1, -0.4, 0.15],
    ],
    [
      [2, 15, 120],
      [0.15, -0.4, 1],
    ],
    [
      [2, 15, 160],
      [0.15, -0.4, -1],
    ],
    [
      [-120, 15, -2],
      [-1, -0.4, -0.15],
    ],
    [
      [-160, 15, -2],
      [1, -0.4, -0.15],
    ],
  ] as [V3, V3][]) {
    const hit = battleTerrainRaycast(
      terrain,
      { origin, direction: normalize(delta) },
      90,
    )!;
    assert(hit, origin.join());
    assert(
      Math.abs(
        hit.point[1] - battleTerrainHeight(terrain, hit.point[0], hit.point[2]),
      ) < 1e-5,
    );
    assert(Math.abs(Math.hypot(...hit.normal) - 1) < 1e-8);
    assert(hit.normal[1] > 0.99);
  }
});

test("terrain picking preserves a nonplanar cell's two separate triangles", () => {
  const terrain = field();
  terrain.heights[129 * 257 + 129] = 6;
  for (const [x, z] of [
    [0.25, 0.5],
    [1.75, 1.5],
    [1, 1],
  ]) {
    const hit = battleTerrainRaycast(
      terrain,
      { origin: [x, 10, z], direction: [0, -1, 0] },
      20,
    )!;
    assert(hit);
    assert(Math.abs(hit.point[1] - battleTerrainHeight(terrain, x, z)) < 1e-6);
  }
});

test("meteor range is measured from the player, with no target for sky or distant ground", () => {
  const terrain = field();
  for (const pitch of [0.2, 0, -0.01])
    assert.equal(
      battleMeteorTarget(
        terrain,
        { eye: [0, 1.62, 0], yaw: 0, pitch },
        16 / 9,
        { x: 0, y: 0, locked: true },
        [0, 1.62, 0],
        false,
      ),
      undefined,
    );
  assert.equal(
    battleMeteorTarget(
      terrain,
      { eye: [0, 1.62, 0], yaw: 0, pitch: -0.2 },
      16 / 9,
      { x: 0, y: 0, locked: true },
      [100, 1.62, 0],
      false,
    ),
    undefined,
  );
});

test("third-person and unlocked pointer targeting reuse Town's camera convention", () => {
  const terrain = field(),
    camera = { eye: [0, 4, 6] as V3, yaw: 0, pitch: -0.5 };
  const third = battleMeteorTarget(
    terrain,
    camera,
    16 / 9,
    { x: 0, y: 0, locked: true },
    [0, 1.62, 0],
    true,
  )!;
  const center = battleMeteorTarget(
    terrain,
    camera,
    16 / 9,
    { x: 0, y: 0, locked: true },
    [0, 1.62, 0],
    false,
  )!;
  const cursor = battleMeteorTarget(
    terrain,
    camera,
    16 / 9,
    { x: 0.5, y: -0.2, locked: false },
    [0, 1.62, 0],
    true,
  )!;
  assert(third && center && cursor);
  assert(third.point[2] < center.point[2]);
  assert(cursor.point[0] > third.point[0]);
});

test("finite field edges and invalid rays cannot create a remote target", () => {
  const terrain = field();
  assert.equal(
    battleTerrainRaycast(
      terrain,
      { origin: [1700, 2, 0], direction: [0, -1, 0] },
      10,
    ),
    undefined,
  );
  assert(
    battleTerrainRaycast(
      terrain,
      { origin: [1700, 40, 0], direction: normalize([-1, -1, 0]) },
      90,
    ),
  );
  for (const ray of [
    { origin: [0, NaN, 0], direction: [0, -1, 0] },
    { origin: [0, 2, 0], direction: [0, -2, 0] },
    { origin: [0, 2, 0], direction: [0, 0, 0] },
  ])
    assert.equal(
      battleTerrainRaycast(terrain, ray as { origin: V3; direction: V3 }, 10),
      undefined,
    );
});
