import assert from "node:assert/strict";
import { test } from "node:test";
import type { SourceLight } from "../packages/assets/src/scene.ts";
import {
  buildClusteredLights,
  CLUSTER_HEADER,
  CLUSTER_LIGHT_FLOATS,
  CLUSTER_LIMITS,
  CLUSTER_OVERFLOW,
  clusterCellForPoint,
  clusterLightIndices,
  type ClusteredLightInput,
} from "../packages/renderer/clustered-lights.ts";
import { lightColor, lightIntensity } from "../packages/renderer/lighting.ts";
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
import { SCENE_LINEAR_SCALE } from "../packages/renderer/radiometry.ts";

function light(id: string, position: V3 = [0, 0, -5], range = 1): SourceLight {
  return {
    id,
    label: id,
    kind: "point",
    position,
    direction: [0, -1, 0],
    color: [1, 0.5, 0.25],
    intensity: 100,
    intensityUnits: "candelas",
    range,
    castShadows: true,
    enabled: true,
    temperatureKelvin: 6500,
    useTemperature: false,
    raw: {},
  };
}

function input(
  lights: SourceLight[] = [],
  overrides: Partial<ClusteredLightInput> = {},
): ClusteredLightInput {
  return {
    viewProjection: perspective(Math.PI / 2, 1, 0.1, 2000),
    eye: [0, 0, 0],
    forward: [0, 0, -1],
    width: 512,
    height: 512,
    lights,
    ...overrides,
  };
}

function seeded(seed: number) {
  return () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 4294967296;
  };
}

function unproject(matrix: ArrayLike<number>, clip: V3): V3 {
  const p = [0, 1, 2, 3].map(
    (r) =>
      matrix[r] * clip[0] +
      matrix[r + 4] * clip[1] +
      matrix[r + 8] * clip[2] +
      matrix[r + 12],
  );
  return [p[0] / p[3], p[1] / p[3], p[2] / p[3]];
}

test("packed light records retain source order, source units, spot cones and shadow frame identity", () => {
  const disabled = { ...light("disabled"), enabled: false };
  const sun = { ...light("sun"), kind: "directional" as const };
  const point = light("point", [1.25, -2, -8], 7);
  const spot: SourceLight = {
    ...light("spot", [-3, 4, -6], 12),
    kind: "spot",
    direction: [0, -3, -4],
    intensity: 1000,
    intensityUnits: "lumens",
    innerConeAngleRadians: 0.2,
    outerConeAngleRadians: 0.5,
    temperatureKelvin: 3000,
    useTemperature: true,
  };
  const legacy: SourceLight = {
    ...light("legacy"),
    intensity: 84,
    intensityUnits: "unitless",
    inverseSquaredFalloff: false,
    falloffExponent: 3,
    castShadows: false,
  };
  const source = [sun, point, disabled, spot, legacy];
  const original = structuredClone(source);
  const clusters = buildClusteredLights(
    input(source, {
      shadowIndices: new Map([
        ["point", 7],
        ["spot", 2],
        ["legacy", 4],
      ]),
    }),
  );
  assert.deepEqual(source, original);
  assert.deepEqual(clusters.lightIds, ["point", "spot", "legacy"]);
  assert.deepEqual(Array.from(clusters.sourceIndices), [1, 3, 4]);
  assert.equal(clusters.storage[CLUSTER_HEADER.lightCount], 3);
  assert.equal(
    clusters.storage[CLUSTER_HEADER.lightsOffset],
    CLUSTER_HEADER.words,
  );
  assert.equal(
    clusters.storage[CLUSTER_HEADER.cellsOffset],
    CLUSTER_HEADER.words + 3 * CLUSTER_LIGHT_FLOATS,
  );
  assert.equal(
    clusters.storage[CLUSTER_HEADER.indicesOffset],
    CLUSTER_HEADER.words + 3 * CLUSTER_LIGHT_FLOATS + clusters.cells.length,
  );
  assert.equal(clusters.lightFloats.buffer, clusters.storage.buffer);
  assert.equal(clusters.cells.buffer, clusters.storage.buffer);
  assert.equal(clusters.indices.buffer, clusters.storage.buffer);
  assert.equal(
    new Float32Array(clusters.storage.buffer)[CLUSTER_HEADER.logDepthScale],
    clusters.grid.logDepthScale,
  );
  const camera = buildClusteredLights(input([], { forward: [3, 4, -12] }));
  assert.deepEqual(
    Array.from(
      new Float32Array(camera.storage.buffer).subarray(
        CLUSTER_HEADER.forwardX,
        CLUSTER_HEADER.forwardZ + 1,
      ),
    ),
    normalize([3, 4, -12]).map(Math.fround),
  );
  assert.equal(camera.storage[19], 0);
  for (const [i, sourceLight] of [point, spot, legacy].entries()) {
    const record = clusters.lightFloats.subarray(i * 16, i * 16 + 16);
    assert.deepEqual(Array.from(record.subarray(0, 4)), [
      ...sourceLight.position,
      sourceLight.range,
    ]);
    assert.deepEqual(
      Array.from(record.subarray(4, 7)),
      lightColor(sourceLight).map(Math.fround),
    );
    assert.equal(
      record[7],
      Math.fround(lightIntensity(sourceLight) * SCENE_LINEAR_SCALE),
    );
  }
  assert.deepEqual(
    Array.from(clusters.lightFloats.subarray(8, 16)),
    [0, -1, 0, 0, 1, -1, 7, -1],
  );
  assert.deepEqual(Array.from(clusters.lightFloats.subarray(24, 28)), [
    0,
    Math.fround(-0.6),
    Math.fround(-0.8),
    1,
  ]);
  assert.equal(clusters.lightFloats[28], Math.fround(Math.cos(0.2)));
  assert.equal(clusters.lightFloats[29], Math.fround(Math.cos(0.5)));
  assert.equal(clusters.lightFloats[30], 2);
  assert.equal(clusters.lightFloats[46], -1);
  assert.equal(clusters.lightFloats[47], 3);
  for (let cell = 0; cell < clusters.cells.length / 2; cell++) {
    const indices = Array.from(clusterLightIndices(clusters, cell));
    assert.deepEqual(
      indices,
      indices.toSorted((a, b) => a - b),
    );
  }
});

test("empty scenes, offscreen lights, partial edge tiles, and top-left screen lookup have defined layouts", () => {
  const empty = buildClusteredLights(input([], { width: 129, height: 65 }));
  assert.deepEqual([empty.grid.tilesX, empty.grid.tilesY], [3, 2]);
  assert.equal(empty.cells.length, 3 * 2 * 16 * 2);
  assert.equal(empty.indices.length, 0);
  assert.equal(empty.stats.overflowCellCount, 0);
  assert.ok(empty.cells.every((n) => n === 0));
  const clusters = buildClusteredLights(
    input([
      light("visible", [0, 0, -5], 0.01),
      light("behind", [0, 0, 5], 1),
      light("right", [50, 0, -5], 1),
      light("past-far", [0, 0, -5000], 1),
    ]),
  );
  assert.deepEqual(Array.from(new Set(clusters.indices)), [0]);
  const topLeft = clusterCellForPoint(clusters, [-4.9, 4.9, -5])!;
  const bottomRight = clusterCellForPoint(clusters, [4.9, -4.9, -5])!;
  assert.equal(topLeft % 64, 0);
  assert.equal(bottomRight % 64, 63);
  assert.equal(clusterCellForPoint(clusters, [0, 0, 1]), undefined);
  assert.equal(clusterCellForPoint(clusters, [100, 0, -1]), undefined);
});

test("lights crossing the near plane cover every screen tile and visible fragments clamp to depth end slices", () => {
  const clusters = buildClusteredLights(
    input([light("near", [0, 0, -0.1], 0.2)], { far: 10 }),
  );
  const corners: V3[] = [
    [-0.09, 0.09, -0.101],
    [0.09, 0.09, -0.101],
    [-0.09, -0.09, -0.101],
    [0.09, -0.09, -0.101],
  ];
  for (const point of corners)
    assert.ok(
      clusterLightIndices(
        clusters,
        clusterCellForPoint(clusters, point)!,
      ).includes(0),
    );
  // Cluster near/far define logarithmic slices, not additional clipping planes.
  const farLight = buildClusteredLights(
    input([light("far", [0, 0, -100], 1)], { far: 10 }),
  );
  const cell = clusterCellForPoint(farLight, [0, 0, -100])!;
  assert.equal(Math.floor(cell / 64), 15);
  assert.ok(clusterLightIndices(farLight, cell).includes(0));
});

test("per-cell index exhaustion falls back to every light without truncating source identity", () => {
  const lights = Array.from({ length: 70 }, (_, i) =>
    light(`light-${i}`, [0, 0, -5], 100),
  );
  const clusters = buildClusteredLights(
    input(lights, { width: 128, height: 128, depthSlices: 1, maxIndices: 70 }),
  );
  assert.equal(clusters.indices.length, 70);
  assert.equal(clusters.stats.overflowCellCount, 3);
  assert.equal(clusters.cells[1], 70);
  assert.equal(clusters.cells[3], CLUSTER_OVERFLOW);
  for (let cell = 0; cell < 4; cell++)
    assert.deepEqual(
      Array.from(clusterLightIndices(clusters, cell)),
      Array.from({ length: 70 }, (_, i) => i),
    );
  const zero = buildClusteredLights(input(lights, { maxIndices: 0 }));
  assert.equal(zero.indices.length, 0);
  assert.ok(zero.cells.some((n) => n === CLUSTER_OVERFLOW));
  assert.ok(
    clusterLightIndices(zero, clusterCellForPoint(zero, [0, 0, -5])!).includes(
      69,
    ),
  );
});

test("all 488 source lights remain available at tile and logarithmic depth boundaries", () => {
  const lights = Array.from({ length: 488 }, (_, i) =>
    light(`source-${i}`, [0, 0, -5], 100),
  );
  const clusters = buildClusteredLights(input(lights));
  assert.equal(clusters.lightIds.length, 488);
  assert.equal(clusters.stats.overflowCellCount, 0);
  assert.equal(clusters.stats.maxCellLightCount, 488);
  const inv = inverse(clusters.viewProjection);
  for (let slice = 0; slice <= clusters.grid.depthSlices; slice++) {
    const depth =
      clusters.grid.near * Math.exp(slice / clusters.grid.logDepthScale);
    if (depth > 90) continue;
    const clipZ =
      (-clusters.viewProjection[10] * depth + clusters.viewProjection[14]) /
      depth;
    for (const sign of [-1, 1]) {
      const point = unproject(inv, [sign * 1e-7, sign * 1e-7, clipZ]);
      const cell = clusterCellForPoint(clusters, point);
      if (cell !== undefined)
        assert.equal(clusterLightIndices(clusters, cell).length, 488);
    }
  }
});

test("excessive CPU memberships use a bounded whole-grid fallback", () => {
  const clusters = buildClusteredLights(
    input(
      Array.from({ length: 65 }, (_, i) =>
        light(`large-${i}`, [0, 0, -5], 1e6),
      ),
      { tileSize: 8, depthSlices: 64 },
    ),
  );
  assert.ok(clusters.stats.candidateMemberships > CLUSTER_LIMITS.memberships);
  assert.equal(clusters.stats.globalOverflow, true);
  assert.equal(clusters.indices.length, 0);
  assert.equal(clusters.stats.overflowCellCount, clusters.cells.length / 2);
  assert.equal(clusters.stats.maxCellLightCount, 65);
  assert.ok(clusters.storage.byteLength < 3 * 1024 * 1024);
  assert.ok(clusterLightIndices(clusters, 0).includes(64));
});

test("sphere coverage matches brute force for perspective, reflected, off-axis, rolled and orthographic cameras", () => {
  const random = seeded(0xa37e21);
  const eye: V3 = [17, 9, 23],
    target: V3 = [-3, 2, -8];
  const forward = normalize(sub(target, eye));
  const view = lookAt(eye, target, [0.2, 1, 0.1]);
  const projection = perspective(1.2, 1.7, 0.1, 2000);
  const reflected = new Float32Array([
    -1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
  ]);
  const offAxis = projection.slice();
  offAxis[8] = 0.35;
  offAxis[9] = -0.2;
  const cameras = [
    {
      eye: [0, 0, 0] as V3,
      forward: [0, 0, -1] as V3,
      matrix: perspective(1.4, 1.7),
    },
    { eye, forward, matrix: multiply(projection, view) },
    { eye, forward, matrix: multiply(reflected, multiply(projection, view)) },
    { eye, forward, matrix: multiply(offAxis, view) },
    {
      eye,
      forward,
      matrix: multiply(orthographic(-30, 30, -20, 20, 0.1, 2000), view),
    },
  ];
  let comparisons = 0;
  for (const camera of cameras) {
    const inverseVP = inverse(camera.matrix);
    const points: V3[] = Array.from({ length: 800 }, (_, i) =>
      unproject(inverseVP, [
        random() * 1.9998 - 0.9999,
        random() * 1.9998 - 0.9999,
        i < 8
          ? [0.0001, 0.01, 0.1, 0.5, 0.9, 0.99, 0.999, 0.99999][i]
          : 1 - 10 ** (-random() * 5),
      ]),
    );
    const lights = points.slice(0, 120).map((p, i) => {
      const range = 10 ** (-2 + random() * 5);
      return light(
        `random-${i}`,
        p.map((v) => v + (random() - 0.5) * range) as V3,
        range,
      );
    });
    lights.push(
      light("near-crossing", camera.eye, 3),
      light("whole-world", [0, 0, 0], 1e6),
    );
    const clusters = buildClusteredLights(
      input(lights, {
        viewProjection: camera.matrix,
        eye: camera.eye,
        forward: camera.forward,
        width: 853,
        height: 501,
        maxIndices: 800000,
      }),
    );
    assert.equal(clusters.stats.globalOverflow, false);
    for (const p of points) {
      const cell = clusterCellForPoint(clusters, p);
      if (cell === undefined) continue;
      const candidates = new Set(clusterLightIndices(clusters, cell));
      for (let i = 0; i < lights.length; i++) {
        const offset = i * 16;
        const distance = Math.hypot(
          p[0] - clusters.lightFloats[offset],
          p[1] - clusters.lightFloats[offset + 1],
          p[2] - clusters.lightFloats[offset + 2],
        );
        if (distance < clusters.lightFloats[offset + 3]) {
          comparisons++;
          assert.ok(
            candidates.has(i),
            `Camera ${cameras.indexOf(camera)}, light ${i}, cell ${cell} missed distance ${distance}.`,
          );
        }
      }
    }
  }
  assert.ok(comparisons > 10000);
});

test("finite validation and resource caps reject malformed inputs before allocation", () => {
  const invalidInputs: Array<Partial<ClusteredLightInput>> = [
    { width: 0 },
    { width: Infinity },
    { height: 1.5 },
    { width: 16385 },
    { tileSize: 0 },
    { depthSlices: 0 },
    { depthSlices: 65 },
    { width: 16384, height: 16384, tileSize: 1 },
    { near: 0 },
    { near: NaN },
    { near: 1, far: 1 },
    { far: Infinity },
    { maxIndices: -1 },
    { maxIndices: CLUSTER_LIMITS.indices + 1 },
    { eye: [NaN, 0, 0] },
    { eye: [1e100, 0, 0] },
    { forward: [0, 0, 0] },
    { viewProjection: new Float32Array(16) },
    { viewProjection: [NaN, ...new Array(15).fill(1)] },
    { viewProjection: new Float32Array(15) },
    {
      lights: Array.from({ length: CLUSTER_LIMITS.lights + 1 }, (_, i) =>
        light(`${i}`),
      ),
    },
  ];
  for (const invalid of invalidInputs)
    assert.throws(() => buildClusteredLights(input([], invalid)));
  const invalidLights: Array<Partial<SourceLight>> = [
    { id: "" },
    { range: 0 },
    { range: -1 },
    { range: Infinity },
    { intensity: -1 },
    { intensity: Infinity },
    { intensityUnits: "unknown" },
    { position: [0, NaN, 0] },
    { position: [1e100, 0, 0] },
    { color: [-1, 0, 0] },
    { color: [1, Infinity, 0] },
    { color: [1, 1] },
    { kind: "spot", direction: [0, 0, 0] },
    { kind: "spot", outerConeAngleRadians: 0 },
    { kind: "spot", innerConeAngleRadians: 0.5, outerConeAngleRadians: 0.2 },
    { inverseSquaredFalloff: false, falloffExponent: -1 },
  ];
  for (const invalid of invalidLights)
    assert.throws(() =>
      buildClusteredLights(input([{ ...light("invalid"), ...invalid }])),
    );
  assert.throws(() =>
    buildClusteredLights(input([light("duplicate"), light("duplicate")])),
  );
  assert.throws(() =>
    buildClusteredLights(
      input([light("shadow")], { shadowIndices: new Map([["shadow", 0.5]]) }),
    ),
  );
  const clusters = buildClusteredLights(input());
  assert.throws(() => clusterLightIndices(clusters, -1));
  assert.throws(() => clusterLightIndices(clusters, clusters.cells.length / 2));
  assert.throws(() => clusterCellForPoint(clusters, [NaN, 0, 0]));
});
