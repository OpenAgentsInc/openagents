import assert from "node:assert/strict";
import test from "node:test";
import {
  battleCamera,
  battleTerrain,
} from "../packages/runtime/battle-view.ts";
import { battleTerrainHeight, validateBattleCrowdOptions } from "../packages/renderer/battle-crowd.ts";
import { mmorpgHeight } from "../packages/runtime/mmorpg-terrain.ts";

/** Independent explicit vertices of the displayed 257² terrain, including mixed cells. */
function displayedHeight(center: readonly number[], x: number, z: number) {
  const coordinate = (index: number) => center[index] + 0;
  const vertices = (origin: number) =>
    Array.from({ length: 257 }, (_, i) => {
      const offset = i - 128,
        absolute = Math.abs(offset);
      return (
        origin +
        Math.sign(offset) *
          (absolute <= 64 ? absolute * 2 : 128 + (absolute - 64) * 24)
      );
    });
  const xs = vertices(coordinate(0)),
    zs = vertices(coordinate(1));
  const i = xs.findIndex(
    (n, index) => index < 256 && n <= x && x <= xs[index + 1],
  );
  const j = zs.findIndex(
    (n, index) => index < 256 && n <= z && z <= zs[index + 1],
  );
  assert.ok(i >= 0 && j >= 0);
  const fx = (x - xs[i]) / (xs[i + 1] - xs[i]),
    fz = (z - zs[j]) / (zs[j + 1] - zs[j]);
  const a = mmorpgHeight(xs[i], zs[j]),
    b = mmorpgHeight(xs[i + 1], zs[j]),
    c = mmorpgHeight(xs[i], zs[j + 1]),
    d = mmorpgHeight(xs[i + 1], zs[j + 1]);
  return fx + fz <= 1
    ? a + (b - a) * fx + (c - a) * fz
    : d + (c - d) * (1 - fx) + (b - d) * (1 - fz);
}

test("battle height grid snaps to the same 16 metre center as rendered terrain", () => {
  const terrain = battleTerrain([-10, 31]);
  assert.deepEqual(terrain.center, [-16, 16]);
  assert.equal(terrain.resolution, 257);
  assert.equal(terrain.size, 3328);
  assert.equal(terrain.layout, "mmorpg-patch");
  assert.equal(terrain.heights.length, 257 ** 2);
  assert.ok(terrain.heights.every(Number.isFinite));
  assert.equal(
    battleTerrainHeight(terrain, -16, 16),
    Math.fround(mmorpgHeight(-16, 16)),
  );
  for (const position of [
    [-16, 16],
    [-22, 46],
    [144, 184],
    [-240, -214],
    [100, 188],
    [-170, 40],
  ]) {
    const [x, z] = position;
    assert.ok(
      Math.abs(
        battleTerrainHeight(terrain, x, z) -
          displayedHeight(terrain.center, x, z),
      ) < 2e-6,
      `${x},${z}`,
    );
  }
});

test("inner and coarse square cells retain the visible triangle surface between samples", () => {
  const terrain = battleTerrain([-10, 0]);
  for (const [x, z] of [
    [28.833490534, -20.760679227],
    [-5.75, 3.125],
    [161.7, 173.3],
    [-190.1, -177.8],
    [100.7, 171.3],
    [-170.25, 40.75],
    [142.1, -112.4],
  ])
    assert.ok(
      Math.abs(
        battleTerrainHeight(terrain, x, z) -
          displayedHeight(terrain.center, x, z),
      ) < 2e-6,
      `${x},${z}`,
    );
  assert.throws(() => battleTerrain([NaN, 0]));
  assert.throws(() => battleTerrain([20001, 0]));
});

test("battle spectator camera leaves its input center intact", () => {
  const center = [-10, 2, 0],
    copy = [...center];
  const camera = battleCamera(center, 55, "overview", 10);
  assert.deepEqual(center, copy);
  assert.equal(camera.far, 5000);
  assert.ok(camera.eye.every(Number.isFinite));
  assert.notDeepEqual(
    battleCamera(center, 55, "orbit", 0).eye,
    battleCamera(center, 55, "orbit", 10).eye,
  );
});

test("first person uses the real local actor eye and heading without moving it", () => {
  const participant = {
    position: [-10, 2, 100] as [number, number, number],
    eye: [-10, 3.62, 100] as [number, number, number],
    yaw: 1.1,
    pitch: -0.15,
    height: 1.8,
  };
  const original = structuredClone(participant);
  const camera = battleCamera([-10, 2, 100], 173, "first", 20, participant);
  assert.deepEqual(camera.eye, participant.eye);
  assert.equal(camera.yaw, participant.yaw);
  assert.equal(camera.pitch, participant.pitch);
  assert.equal(camera.near, 0.06);
  assert.equal(camera.sourceFogScale, 1);
  assert.deepEqual(participant, original);
  assert.throws(() => battleCamera([0, 0, 0], 100, "first", 0));
});

test("spaced formations admit finite cameras through the one-kilometre arena limit", () => {
  const center = [0, 0, 100];
  for (const radius of [12, 100, 317, 708, 867, 1000]) {
    for (const view of ["overview", "orbit", "dolly"] as const) {
      const camera = battleCamera(center, radius, view, 10);
      assert(camera.eye.every(Number.isFinite));
      assert.equal(camera.far, 5000);
    }
  }
  assert.throws(() => battleCamera(center, 1000.01, "overview", 0));
});

test("third person follows actor translation and turns the shoulder with its heading", () => {
  const actor = {
    position: [0, 100, 0] as [number, number, number],
    yaw: 0,
    height: 1.8,
  };
  const a = battleCamera([0, 0, 0], 50, "third", 0, actor);
  assert.ok(a.eye[1] - actor.position[1] > 2.7 && a.eye[1] - actor.position[1] < 2.9);
  assert.ok(a.eye[2] > 5.8);
  assert.ok(a.pitch < -0.2 && a.pitch > -0.25);
  const b = battleCamera([0, 0, 0], 200, "third", 0, {
    ...actor,
    position: [5, 102, -7],
  });
  for (let i = 0; i < 3; i++)
    assert.ok(Math.abs(b.eye[i] - a.eye[i] - [5, 2, -7][i]) < 1e-9);
  const turned = battleCamera([0, 0, 0], 50, "third", 0, {
    ...actor,
    yaw: Math.PI / 2,
  });
  assert.ok(turned.eye[0] < actor.position[0] - 3);
  assert.ok(a.eye[2] > actor.position[2] + 3);
  assert.deepEqual(battleCamera([0, 0, 0], 50, "player", 0, actor), a);
});

test("tight overhead dolly stays human-scale, clears actual terrain and moves smoothly", () => {
  const center = [-10, 0, 100];
  const shots = [0, 10, 20, 30].map((time) =>
    battleCamera(center, 200, "dolly", time),
  );
  for (const shot of shots) {
    assert.ok(shot.eye[1] - mmorpgHeight(shot.eye[0], shot.eye[2]) >= 9);
    assert.ok(Math.abs(shot.eye[0] - center[0]) < 20);
    assert.ok(Math.abs(shot.eye[2] - center[2]) < 20);
    assert.ok(shot.pitch < -0.25 && shot.pitch > -1.3);
    assert.equal(shot.fov, 0.78);
  }
  assert.deepEqual(
    battleCamera(center, 12, "dolly", 10),
    battleCamera(center, 225, "dolly", 10),
  );
  const a = battleCamera(center, 200, "dolly", 10),
    b = battleCamera(center, 200, "dolly", 10 + 1 / 60);
  assert.ok(Math.hypot(...a.eye.map((n, i) => n - b.eye[i])) < 0.05);
  assert.notDeepEqual(shots[0].eye, shots.at(-1)!.eye);
  assert.deepEqual(center, [-10, 0, 100]);
});

test("third person orbits with actual vertical look and keeps the camera above terrain", () => {
  const actor = {
    position: [0, 100, 0] as [number, number, number],
    yaw: 0.4,
    height: 1.8,
    pitch: 0,
  };
  const flat = battleCamera([0, 0, 0], 100, "third", 0, actor);
  const up = battleCamera([0, 0, 0], 100, "third", 0, { ...actor, pitch: 0.5 });
  const down = battleCamera([0, 0, 0], 100, "third", 0, {
    ...actor,
    pitch: -0.5,
  });
  assert.ok(up.pitch > flat.pitch + 0.4);
  assert.ok(down.pitch < flat.pitch - 0.4);
  assert.ok(up.eye[1] < flat.eye[1] && down.eye[1] > flat.eye[1]);
  for (const camera of [flat, up, down]) {
    const distance = Math.hypot(
      camera.eye[0],
      camera.eye[1] - (100 + actor.height * 0.82),
      camera.eye[2],
    );
    assert.ok(Math.abs(distance - Math.hypot(6, 0.48)) < 1e-8);
  }
  const groundActor = {
    ...actor,
    position: [0, mmorpgHeight(0, 0), 0] as [number, number, number],
    pitch: 1.2,
  };
  const grounded = battleCamera([0, 0, 0], 100, "third", 0, groundActor);
  assert.ok(
    grounded.eye[1] >= mmorpgHeight(grounded.eye[0], grounded.eye[2]) + 0.4,
  );
  assert.ok(grounded.pitch > flat.pitch);
  assert.equal(groundActor.pitch, 1.2);
});


test("default visibility keeps the expanded arena corners in wide camera range", () => {
  const center = [0, 0, 0];
  const radius = 1000;
  const maxDistance = validateBattleCrowdOptions({ count: 1_000_000 }).maxDistance;
  for (const view of ["overview", "orbit"] as const) {
    const camera = battleCamera(center, radius, view, 0);
    assert(camera.far !== undefined);
    for (const x of [-radius, radius])
      for (const z of [-radius, radius]) {
        const distance = Math.hypot(x-camera.eye[0], -camera.eye[1], z-camera.eye[2]);
        assert(distance < maxDistance);
        assert(distance < camera.far);
      }
  }
});
