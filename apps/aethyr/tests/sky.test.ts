import assert from "node:assert/strict";
import { test } from "node:test";
import type { CompiledScene } from "../packages/assets/src/scene.ts";
import { FOG_HEIGHT_RATE } from "../packages/renderer/fog.ts";
import {
  sourceSkyDome,
  skyDomeDistance,
  skyFogTransmittance,
} from "../packages/renderer/sky.ts";

const close = (a: number, b: number, tolerance = 1e-10) =>
  assert.ok(Math.abs(a - b) <= tolerance, `${a} != ${b}`);
function fixture() {
  const vertices: number[] = [],
    indices: number[] = [],
    latitude = 20,
    longitude = 40;
  for (let y = 0; y <= latitude; y++)
    for (let x = 0; x <= longitude; x++) {
      const theta = (y / latitude) * Math.PI,
        phi = (x / longitude) * Math.PI * 2;
      vertices.push(
        Math.sin(theta) * Math.cos(phi),
        Math.cos(theta),
        Math.sin(theta) * Math.sin(phi),
        ...Array(9).fill(0),
      );
    }
  for (let y = 0; y < latitude; y++)
    for (let x = 0; x < longitude; x++) {
      const a = y * (longitude + 1) + x,
        b = a + longitude + 1;
      indices.push(a, b, a + 1, a + 1, b, b + 1);
    }
  const scene = {
    materials: [{ id: "owned/sky" }, { id: "owned/other" }],
    meshes: [
      {
        bounds: { min: [-1, -1, -1], max: [1, 1, 1] },
        primitives: [
          {
            firstIndex: 0,
            indexCount: indices.length,
            baseVertex: 0,
            material: 0,
            materialSlot: 0,
          },
        ],
      },
    ],
    instances: [
      {
        mesh: 0,
        materialOverrides: [],
        model: [10, 0, 0, 0, 0, 10, 0, 0, 0, 0, 10, 0, 4, 2, -3, 1],
      },
    ],
  } as unknown as CompiledScene;
  return {
    scene,
    geometry: {
      vertices: new Float32Array(vertices),
      indices: new Uint32Array(indices),
    },
  };
}
test("source dome uses actual effective materials, finite placements and near-spherical geometry", () => {
  const { scene, geometry } = fixture();
  assert.deepEqual(sourceSkyDome(scene, "owned/sky", geometry), {
    center: [4, 2, -3],
    radiusMeters: 10,
  });
  assert.equal(sourceSkyDome(scene, "missing", geometry), undefined);
  scene.instances[0].materialOverrides = [1];
  assert.equal(sourceSkyDome(scene, "owned/sky", geometry), undefined);
  assert.deepEqual(sourceSkyDome(scene, "owned/other", geometry), {
    center: [4, 2, -3],
    radiusMeters: 10,
  });
  scene.instances[0].model[0] = -10;
  assert.equal(sourceSkyDome(scene, "owned/other", geometry)?.radiusMeters, 10);
  scene.instances.push({ ...scene.instances[0] });
  assert.throws(
    () => sourceSkyDome(scene, "owned/other", geometry),
    /one source/,
  );
});
test("unsupported source dome shapes, triangle ranges and affine placements fail", () => {
  for (const mutate of [
    (f: ReturnType<typeof fixture>) => (f.scene.instances[0].model[0] = 11),
    (f: ReturnType<typeof fixture>) => (f.scene.instances[0].model[4] = 1),
    (f: ReturnType<typeof fixture>) => (f.scene.instances[0].model[15] = 0),
    (f: ReturnType<typeof fixture>) => (f.geometry.vertices[0] = 9),
    (f: ReturnType<typeof fixture>) => (f.geometry.indices[0] = 999999),
    (f: ReturnType<typeof fixture>) =>
      (f.scene.meshes[0].primitives[0].indexCount = 3.5),
    (f: ReturnType<typeof fixture>) => (f.scene.meshes[0].bounds.max[0] = 4),
  ]) {
    const f = fixture();
    mutate(f);
    assert.throws(() => sourceSkyDome(f.scene, "owned/sky", f.geometry));
  }
  const f = fixture();
  f.geometry.indices.set([0, 20 * 41, 10 * 41]);
  assert.throws(
    () => sourceSkyDome(f.scene, "owned/sky", f.geometry),
    /closer spherical fit/,
  );
});
test("ray intersections handle inside, outside, tangent, reflected direction and misses", () => {
  const dome = {
    center: [0, 0, 0] as [number, number, number],
    radiusMeters: 10,
  };
  close(skyDomeDistance(dome, [0, 0, 0], [0, 0, 1]), 10);
  close(skyDomeDistance(dome, [0, 0, 2], [0, 0, 10]), 8);
  close(skyDomeDistance(dome, [0, 0, 2], [0, 0, -1]), 12);
  close(skyDomeDistance(dome, [0, 0, 20], [0, 0, -1]), 10);
  close(skyDomeDistance(dome, [0, 0, 20], [0, 0, 1]), 0);
  close(skyDomeDistance(dome, [0, 0, 20], [1, 0, 0]), 0);
  close(skyDomeDistance(dome, [10, 0, -20], [0, 0, 1]), 20);
  assert.throws(() => skyDomeDistance(dome, [0, 0, 0], [0, 0, 0]));
  assert.throws(() =>
    skyDomeDistance({ ...dome, radiusMeters: Infinity }, [0, 0, 0], [0, 0, 1]),
  );
});
test("height fog matches analytic limits and partitions without double extinction", () => {
  const rho = 0.002;
  close(skyFogTransmittance(1, 0, 100, rho), Math.exp(-rho * 100));
  close(skyFogTransmittance(1, 0, 100, rho, 60), Math.exp(-rho * 40));
  close(skyFogTransmittance(1, 0, 100, rho, 200), 1);
  close(
    skyFogTransmittance(1, 0.5, 100, rho),
    Math.exp(
      (-rho * -Math.expm1(-FOG_HEIGHT_RATE * 0.5 * 100)) /
        (FOG_HEIGHT_RATE * 0.5),
    ),
  );
  close(
    skyFogTransmittance(1, 1, 100000, rho),
    Math.exp(-rho / FOG_HEIGHT_RATE),
  );
  close(skyFogTransmittance(1, -1, 100000, rho), 0);
  close(
    skyFogTransmittance(1, 0.000000001, 100, rho),
    Math.exp(-rho * 100),
    1e-8,
  );
  close(
    skyFogTransmittance(2, 0.1, 1000, rho),
    skyFogTransmittance(2, 0.1, 60, rho) *
      skyFogTransmittance(2, 0.1, 1000, rho, 60),
  );
  for (const direction of [-1, -0.5, 0, 0.5, 1]) {
    assert.equal(skyFogTransmittance(1, direction, 13540, 0), 1);
    assert.equal(skyFogTransmittance(1, direction, 0, rho), 1);
    const t = skyFogTransmittance(1, direction, 100000, rho);
    assert.ok(Number.isFinite(t) && t >= 0 && t <= 1);
  }
  assert.throws(() => skyFogTransmittance(0, 2, 100, rho));
  assert.throws(() => skyFogTransmittance(0, 1, 100, -1));
  assert.throws(() => skyFogTransmittance(0, 1, 100, rho, -1));
  assert.throws(() => skyFogTransmittance(NaN, 1, 100, rho));
});
