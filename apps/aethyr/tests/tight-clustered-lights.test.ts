import test from "node:test";
import assert from "node:assert/strict";
import type { SourceLight } from "../packages/assets/src/scene.ts";
import {
  buildClusteredLights,
  buildTightClusteredLights,
  clusterLightIndices,
  CLUSTER_HEADER,
  CLUSTER_OVERFLOW,
  type ClusteredLightInput,
  type ClusteredLights,
} from "../packages/renderer/clustered-lights.ts";
import {
  inverse,
  lookAt,
  multiply,
  normalize,
  orthographic,
  perspective,
  sub,
  type V3,
} from "../packages/renderer/math.ts";

const light = (id: string, position: V3, range: number): SourceLight => ({
  id,
  label: id,
  kind: "point",
  position,
  range,
  direction: [0, -1, 0],
  color: [1, 0.5, 0.25],
  intensity: 100,
  intensityUnits: "candelas",
  castShadows: true,
  enabled: true,
  useTemperature: false,
  temperatureKelvin: 6500,
  raw: {},
});
const input = (
  lights: SourceLight[],
  overrides: Partial<ClusteredLightInput> = {},
): ClusteredLightInput => ({
  viewProjection: perspective(1.2, 1.6, 0.1, 2000),
  eye: [0, 0, 0],
  forward: [0, 0, -1],
  width: 800,
  height: 500,
  lights,
  ...overrides,
});
const seeded = () => {
  let seed = 12999;
  return () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 0x100000000;
  };
};
const unproject = (
  m: ArrayLike<number>,
  x: number,
  y: number,
  z: number,
): V3 => {
  const p = [0, 1, 2, 3].map(
    (r) => m[r] * x + m[r + 4] * y + m[r + 8] * z + m[r + 12],
  );
  return [p[0] / p[3], p[1] / p[3], p[2] / p[3]];
};

// Independent shader-style lookup: use the packed forward and f32 log inputs,
// not the production clusterCellForPoint/depthSlice helpers.
function shaderCell(c: ClusteredLights, point: V3): number | undefined {
  const m = c.viewProjection;
  const q = [0, 1, 2, 3].map(
    (r) =>
      m[r] * point[0] + m[r + 4] * point[1] + m[r + 8] * point[2] + m[r + 12],
  );
  if (
    q[3] <= 0 ||
    q[2] < 0 ||
    q[2] > q[3] ||
    Math.abs(q[0]) > q[3] ||
    Math.abs(q[1]) > q[3]
  )
    return;
  const f = new Float32Array(c.storage.buffer);
  const depth = Math.max(
    c.grid.near,
    Math.fround(
      (point[0] - c.eye[0]) * f[CLUSTER_HEADER.forwardX] +
        (point[1] - c.eye[1]) * f[CLUSTER_HEADER.forwardY] +
        (point[2] - c.eye[2]) * f[CLUSTER_HEADER.forwardZ],
    ),
  );
  const slice = Math.max(
    0,
    Math.min(
      c.grid.depthSlices - 1,
      Math.floor(
        Math.fround(
          Math.log(Math.fround(depth / c.grid.near)) * c.grid.logDepthScale,
        ),
      ),
    ),
  );
  const x = Math.min(
    c.grid.tilesX - 1,
    Math.max(
      0,
      Math.floor(((q[0] / q[3] + 1) * 0.5 * c.grid.width) / c.grid.tileSize),
    ),
  );
  const y = Math.min(
    c.grid.tilesY - 1,
    Math.max(
      0,
      Math.floor(((1 - q[1] / q[3]) * 0.5 * c.grid.height) / c.grid.tileSize),
    ),
  );
  return (slice * c.grid.tilesY + y) * c.grid.tilesX + x;
}

test("tight near-plane sphere lists remove irrelevant tiles while retaining packed light identity and order", () => {
  const source = input(
    [
      { ...light("sun", [0, 0, 0], 1), kind: "directional" },
      light("near-right", [0.3, 0, -0.15], 0.2),
      { ...light("disabled", [0, 0, 0], 1), enabled: false },
      {
        ...light("spot", [-0.3, 0, -0.2], 0.25),
        kind: "spot",
        direction: [0, -1, -1],
        inverseSquaredFalloff: false,
        falloffExponent: 3,
      },
    ],
    {
      shadowIndices: new Map([
        ["near-right", 7],
        ["spot", 2],
      ]),
    },
  );
  const original = buildClusteredLights(source),
    tight = buildTightClusteredLights(source);
  assert.deepEqual(tight.lightFloats, original.lightFloats);
  assert.deepEqual(tight.sourceIndices, original.sourceIndices);
  assert.deepEqual(tight.lightIds, original.lightIds);
  assert.ok(tight.stats.indexCount < original.stats.indexCount * 0.7);
  for (let cell = 0; cell < tight.cells.length / 2; cell++) {
    const actual = [...clusterLightIndices(tight, cell)];
    const broad = clusterLightIndices(original, cell);
    assert.deepEqual(
      actual,
      actual.toSorted((a, b) => a - b),
    );
    assert.ok(actual.every((index) => broad.includes(index)));
  }
});

test("tight planes have no missed sphere influences for perspective, near-crossing, rolled, off-axis, reflected and orthographic views", () => {
  const random = seeded();
  const eye: V3 = [-17, 9, 23],
    target: V3 = [3, 2, -8];
  const forward = normalize(sub(target, eye));
  const projection = perspective(1.2, 1.7, 0.1, 2000);
  const view = lookAt(eye, target, [0.2, 1, 0.1]);
  const offAxis = projection.slice();
  offAxis[8] = -0.35;
  offAxis[9] = 0.2;
  const reflected = new Float32Array([
    -1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
  ]);
  const cameras = [
    { eye: [0, 0, 0] as V3, forward: [0, 0, -1] as V3, matrix: projection },
    { eye, forward, matrix: multiply(projection, view) },
    { eye, forward, matrix: multiply(offAxis, view) },
    { eye, forward, matrix: multiply(reflected, multiply(projection, view)) },
    {
      eye,
      forward,
      matrix: multiply(orthographic(-30, 30, -20, 20, 0.1, 2000), view),
    },
  ];
  let comparisons = 0;
  for (const camera of cameras) {
    const inv = inverse(camera.matrix);
    const points: V3[] = Array.from({ length: 600 }, (_, i) =>
      unproject(
        inv,
        i < 20 ? (i % 2 ? -1 : 1) * 0.999999 : random() * 1.9998 - 0.9999,
        random() * 1.9998 - 0.9999,
        i < 10
          ? [0, 0.000001, 0.01, 0.1, 0.5, 0.9, 0.99, 0.999, 0.9999, 0.99999][i]
          : 1 - 10 ** (-random() * 5),
      ),
    );
    const lights = points.slice(0, 120).map((p, i) => {
      const range = 10 ** (-2 + random() * 4);
      return light(
        `random-${i}`,
        p.map((v) => v + (random() - 0.5) * range) as V3,
        range,
      );
    });
    lights.push(
      light("at-camera", camera.eye, 3),
      light("world", [0, 0, 0], 1e6),
    );
    const tight = buildTightClusteredLights(
      input(lights, {
        viewProjection: camera.matrix,
        eye: camera.eye,
        forward: camera.forward,
        width: 853,
        height: 501,
        near: 1,
        far: 10,
      }),
    );
    assert.equal(tight.stats.globalOverflow, false);
    for (const point of points) {
      const cell = shaderCell(tight, point);
      if (cell === undefined) continue;
      const included = new Set(clusterLightIndices(tight, cell));
      for (let index = 0; index < lights.length; index++) {
        const offset = index * 16;
        if (
          Math.hypot(
            point[0] - tight.lightFloats[offset],
            point[1] - tight.lightFloats[offset + 1],
            point[2] - tight.lightFloats[offset + 2],
          ) < tight.lightFloats[offset + 3]
        ) {
          comparisons++;
          assert.ok(
            included.has(index),
            `View ${cameras.indexOf(camera)} misses ${lights[index].id} at ${point} in cell ${cell}`,
          );
        }
      }
    }
  }
  assert.ok(comparisons > 10000);
});

test("tight planes retain tangent influences at tile and logarithmic boundaries and clamp end slices", () => {
  const base = input([], { width: 257, height: 129, near: 1, far: 10 });
  const template = buildTightClusteredLights(base),
    inv = inverse(template.viewProjection);
  const points: V3[] = [];
  for (const depth of [
    0.100001,
    0.5,
    1,
    ...Array.from(
      { length: 15 },
      (_, slice) =>
        template.grid.near *
        Math.exp((slice + 1) / template.grid.logDepthScale),
    ),
    10,
    100,
  ]) {
    const z =
      (-template.viewProjection[10] * depth + template.viewProjection[14]) /
      depth;
    for (const pixel of [0.0001, 64 - 1e-5, 64, 64 + 1e-5, 128, 256.9999])
      points.push(unproject(inv, (pixel / 257) * 2 - 1, 0, z));
  }
  for (const point of points) {
    const source = light(
      "tangent",
      [point[0] + 0.99999, point[1], point[2]],
      1,
    );
    const clusters = buildTightClusteredLights({ ...base, lights: [source] });
    const cell = shaderCell(clusters, point);
    if (cell !== undefined)
      assert.ok(clusterLightIndices(clusters, cell).includes(0));
  }
});

test("tight builder retains complete index and bounded-work overflow fallbacks", () => {
  const source = input(
    Array.from({ length: 65 }, (_, i) => light(`${i}`, [0, 0, -5], 1e6)),
    { width: 512, height: 512, tileSize: 8, depthSlices: 64 },
  );
  const tight = buildTightClusteredLights(source),
    original = buildClusteredLights(source);
  assert.equal(tight.stats.globalOverflow, true);
  assert.deepEqual(tight.storage, original.storage);
  const exhausted = buildTightClusteredLights(
    input([light("near", [0.3, 0, -0.15], 0.2)], { maxIndices: 0 }),
  );
  assert.ok(exhausted.cells.some((count) => count === CLUSTER_OVERFLOW));
  for (let cell = 0; cell < exhausted.cells.length / 2; cell++)
    if (exhausted.cells[cell * 2 + 1] === CLUSTER_OVERFLOW)
      assert.deepEqual([...clusterLightIndices(exhausted, cell)], [0]);
});

test("tight builder recomputes camera planes without mutating input matrices or lights", () => {
  const source = input([light("near", [0.3, 0, -0.15], 0.2)]);
  const snapshot = structuredClone(source);
  const before = buildTightClusteredLights(source);
  const moved = buildTightClusteredLights({
    ...source,
    viewProjection: multiply(
      perspective(1.2, 1.6, 0.1, 2000),
      lookAt([0, 0, 0], [-1, 0, -1], [0, 1, 0]),
    ),
    forward: normalize([-1, 0, -1]),
  });
  assert.notDeepEqual(before.cells, moved.cells);
  assert.deepEqual(source, snapshot);
});
