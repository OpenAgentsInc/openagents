import assert from "node:assert/strict";
import test from "node:test";
import {
  Grass,
  GrassPatchPlanner,
  GRASS_LIMITS,
  grassClearingScale,
  validateGrassManifest,
  type GrassManifest,
} from "../packages/renderer/grass.ts";
import { lookAt, multiply, perspective } from "../packages/renderer/math.ts";
import type { CreatureFrame } from "../packages/renderer/creatures.ts";

const manifest = (): GrassManifest => ({
  schema: "aethyrforge.grass.v1",
  profile: "quick-grass-v1",
  enabled: true,
  groundHeight: 0,
  bladeWidth: 0.1,
  bladeHeight: 1.5,
  lodDistance: 15,
  maxDistance: 100,
  windStrength: 1,
  interactionRadius: 2.5,
});
const wideView = new Float32Array([
  1 / 300,
  0,
  0,
  0,
  0,
  1 / 300,
  0,
  0,
  0,
  0,
  1 / 300,
  0,
  0,
  0,
  0.5,
  1,
]);
const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
function frame(): CreatureFrame {
  return {
    mode: "day",
    time: 1,
    viewProjection: wideView,
    eye: [0, 1.7, 0],
    diffuseRotation: identity,
    diffuseSH: new Float32Array(36),
    sunDirection: [0, 1, 0],
    sunIlluminance: [4, 4, 4],
    ambient: [0.2, 0.2, 0.2],
  };
}

test("grass profile bounds constrain geometry and reject unsupported settings", () => {
  assert.equal(validateGrassManifest(manifest()).profile, "quick-grass-v1");
  for (const update of [
    { maxDistance: 1000 },
    { bladeHeight: 100 },
    { bladeWidth: 0 },
    { interactionRadius: NaN },
    { windStrength: Infinity },
    { profile: "other" },
    { enabled: 1 },
  ])
    assert.throws(() =>
      validateGrassManifest({ ...manifest(), ...update } as GrassManifest),
    );
  assert.equal(
    GRASS_LIMITS.nearPatches * GRASS_LIMITS.nearBlades +
      GRASS_LIMITS.farPatches * GRASS_LIMITS.farBlades,
    262144,
  );
  assert.ok(GRASS_LIMITS.uniformBytes + GRASS_LIMITS.patchBytes < 8192);
});

test("patch pool preserves world roots, has unique LOD ownership, and stays bounded", () => {
  const planner = new GrassPatchPlanner(manifest()),
    buffer = planner.data;
  planner.update([0, 1.7, 0], wideView);
  assert.ok(planner.nearPatches > 0 && planner.farPatches > 0);
  assert.ok(planner.nearPatches <= 32 && planner.farPatches <= 320);
  const roots = new Set<string>();
  for (const [start, count] of [
    [0, planner.nearPatches],
    [32, planner.farPatches],
  ]) {
    for (let i = 0; i < count; i++) {
      const at = (start + i) * 4,
        x = buffer[at],
        z = buffer[at + 1],
        key = `${x},${z}`;
      assert.ok(!roots.has(key));
      roots.add(key);
      assert.ok(x % 10 === 0);
      assert.ok(z % 10 === 0);
      assert.ok(
        Math.hypot(
          Math.max(0, Math.abs(x) - 5),
          Math.max(0, Math.abs(z) - 5),
        ) <= 100,
      );
    }
  }
  const first = buffer.slice();
  planner.update([0.2, 1.7, 0.2], wideView);
  assert.equal(planner.data, buffer);
  assert.deepEqual(
    [buffer[0], buffer[1], buffer[4], buffer[5]],
    [first[0], first[1], first[4], first[5]],
  );
  assert.equal(first[2], 1);
  assert.equal(buffer[2], 0);
  planner.update([1010, 1.7, 1010], wideView);
  for (const [start, count] of [
    [0, planner.nearPatches],
    [32, planner.farPatches],
  ])
    for (let i = 0; i < count; i++)
      assert.ok(
        Math.max(
          Math.abs(buffer[(start + i) * 4]),
          Math.abs(buffer[(start + i) * 4 + 1]),
        ) < 1029,
      );
  assert.throws(() => planner.update([NaN, 0, 0], wideView));
});

test("patches behind the camera are culled while nearby visible grass remains", () => {
  const planner = new GrassPatchPlanner(manifest());
  const view = multiply(
    perspective(Math.PI / 3, 1, 0.1, 200),
    lookAt([0, 1.7, 0], [0, 1.5, -10]),
  );
  planner.update([0, 1.7, 0], view);
  assert.ok(planner.nearPatches > 0 && planner.farPatches > 0);
  for (const [start, count] of [
    [0, planner.nearPatches],
    [32, planner.farPatches],
  ])
    for (let i = 0; i < count; i++)
      assert.ok(planner.data[(start + i) * 4 + 1] < 20);
});

function fakeGPU(failGroups = false) {
  const buffers: { label: string; size: number; destroyed: number }[] = [],
    pipelines: GPURenderPipelineDescriptor[] = [],
    computePipelines: GPUComputePipelineDescriptor[] = [],
    writes: { buffer: unknown; bytes: ArrayBuffer }[] = [],
    draws: number[][] = [],
    dispatches: number[][] = [];
  let computePasses = 0;
  const device = {
    createShaderModule: () => ({}),
    createRenderPipelineAsync: async (
      descriptor: GPURenderPipelineDescriptor,
    ) => {
      pipelines.push(descriptor);
      return { getBindGroupLayout: () => ({}) };
    },
    createComputePipelineAsync: async (
      descriptor: GPUComputePipelineDescriptor,
    ) => {
      computePipelines.push(descriptor);
      return { getBindGroupLayout: () => ({}) };
    },
    createBuffer: ({ label, size }: { label: string; size: number }) => {
      const value = {
        label,
        size,
        destroyed: 0,
        destroy() {
          this.destroyed++;
        },
      };
      buffers.push(value);
      return value;
    },
    createBindGroup: () => {
      if (failGroups) throw Error("Fixture group failure");
      return {};
    },
    queue: {
      writeBuffer: (buffer: unknown, _at: number, data: ArrayBufferView) =>
        writes.push({
          buffer,
          bytes: new Uint8Array(
            data.buffer,
            data.byteOffset,
            data.byteLength,
          ).slice().buffer,
        }),
    },
  } as unknown as GPUDevice;
  const pass = {
    setPipeline() {},
    setBindGroup() {},
    setIndexBuffer() {},
    drawIndexed(...args: number[]) {
      draws.push(args);
    },
  } as unknown as GPURenderPassEncoder;
  const encoder = {
    beginComputePass() {
      computePasses++;
      return {
        setPipeline() {},
        setBindGroup() {},
        dispatchWorkgroups(...args: number[]) {
          dispatches.push(args);
        },
        end() {},
      };
    },
  } as unknown as GPUCommandEncoder;
  return {
    device,
    pass,
    encoder,
    buffers,
    pipelines,
    computePipelines,
    writes,
    draws,
    dispatches,
    get computePasses() {
      return computePasses;
    },
  };
}
async function gpuGlobals(action: () => Promise<void>) {
  const previous = Object.getOwnPropertyDescriptor(
    globalThis,
    "GPUBufferUsage",
  );
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { UNIFORM: 1, COPY_DST: 2, STORAGE: 4, INDEX: 8 },
  });
  try {
    await action();
  } finally {
    if (previous) Object.defineProperty(globalThis, "GPUBufferUsage", previous);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
}

test("actual grass encoder prepares one fixed cache and submits two indexed draws", async () =>
  gpuGlobals(async () => {
    const fake = fakeGPU(),
      grass = await Grass.create(fake.device, manifest());
    assert.equal(fake.buffers.length, 4);
    assert.equal(
      fake.buffers.reduce((n, b) => n + b.size, 0),
      grass.accountedBytes,
    );
    assert.equal(GRASS_LIMITS.bladeBytes, 16 * 1024 * 1024);
    assert.ok(grass.accountedBytes < 17 * 1024 * 1024);
    assert.equal(fake.pipelines.length, 2);
    assert.equal(fake.computePipelines.length, 2);
    for (const pipeline of fake.pipelines) {
      assert.equal(pipeline.depthStencil?.format, "depth24plus");
      assert.equal(pipeline.multisample?.count, 4);
      assert.equal(pipeline.primitive?.cullMode, "none");
    }
    const indexBytes = fake.writes.find(
        (w) => w.buffer === fake.buffers[3],
      )!.bytes,
      indices = new Uint16Array(indexBytes);
    assert.equal(indices.length, 54);
    assert.ok(Math.max(...indices.subarray(0, 36)) < 14);
    assert.ok(Math.max(...indices.subarray(36)) < 8);
    const state = frame();
    assert.throws(() => grass.render(fake.pass, state), /Prepare grass/);
    grass.prepare(fake.encoder, state, { position: [2, 0, 3] });
    const stats = grass.render(fake.pass, state);
    assert.equal(stats.computeDispatches, 2);
    assert.equal(fake.computePasses, 1);
    assert.equal(stats.cacheRebuildBlades, stats.computedBlades);
    assert.equal(stats.drawCalls, 2);
    assert.equal(fake.draws.length, 2);
    assert.deepEqual(fake.draws[0], [36, stats.nearBlades, 0, 0, 0]);
    assert.deepEqual(fake.draws[1], [18, stats.farBlades, 36, 0, 0]);
    assert.equal(stats.blades, stats.nearBlades + stats.farBlades);
    assert.equal(stats.triangles, stats.nearBlades * 12 + stats.farBlades * 6);
    assert.ok(stats.blades <= 262144);
    assert.deepEqual(fake.dispatches, [
      [stats.nearBlades / 64],
      [stats.farBlades / 64],
    ]);
    const uniformWrites = () =>
      fake.writes
        .filter((w) => w.buffer === fake.buffers[0])
        .map((w) => new Float32Array(w.bytes));
    const uniform = uniformWrites()[0];
    assert.equal(uniform.length, 192);
    assert.equal(uniform[35], 1);
    assert.deepEqual([...uniform.subarray(100, 104)], [2, 0, 3, 1]);
    // Rewinding the world clock must refresh wind while preserving static clump roots.
    state.time = 4;
    grass.prepare(fake.encoder, state, { position: [3, 0, 4] });
    grass.render(fake.pass, state);
    assert.equal(stats.cacheRebuildBlades, 0);
    assert.equal(stats.windTime, 4);
    assert.equal(uniformWrites()[1][35], 4);
    state.time = 2;
    grass.prepare(fake.encoder, state, { position: [3, 0, 4] });
    grass.render(fake.pass, state);
    assert.equal(stats.windTime, 2);
    assert.equal(uniformWrites()[2][35], 2);
    assert.equal(fake.buffers.length, 4);
    state.eye = [11, 1.7, 0];
    grass.prepare(fake.encoder, state);
    assert.ok(stats.cacheRebuildBlades > 0);
    grass.render(fake.pass, state);
    const drawCount = fake.draws.length,
      computeCount = fake.computePasses;
    const disabled = { ...state, enabled: false };
    grass.prepare(fake.encoder, disabled);
    grass.render(fake.pass, disabled);
    assert.equal(stats.blades, 0);
    assert.equal(stats.computeDispatches, 0);
    assert.equal(stats.drawCalls, 0);
    assert.equal(fake.draws.length, drawCount);
    assert.equal(fake.computePasses, computeCount);
    assert.throws(() => grass.prepare(fake.encoder, { ...state, time: NaN }));
    grass.dispose();
    grass.dispose();
    assert.deepEqual(
      fake.buffers.map((b) => b.destroyed),
      [1, 1, 1, 1],
    );
    grass.render(fake.pass, state);
    assert.equal(stats.blades, 0);
  }));

test("cache validity includes the zero-origin patch and survives slot/cardinality changes", () => {
  const planner = new GrassPatchPlanner(manifest()),
    cache = new Map<number, string>();
  const validate = () => {
    for (const [start, count] of [
      [0, planner.nearPatches],
      [GRASS_LIMITS.nearPatches, planner.farPatches],
    ])
      for (let i = 0; i < count; i++) {
        const slot = start + i,
          at = slot * 4,
          key = `${planner.data[at]},${planner.data[at + 1]}`;
        if (planner.data[at + 2] > 0.5) cache.set(slot, key);
        else
          assert.equal(
            cache.get(slot),
            key,
            "A cache hit must name the same world patch and LOD pool",
          );
      }
  };
  planner.update([0, 1.7, 0], wideView);
  assert.equal(planner.data[0], 0);
  assert.equal(planner.data[1], 0);
  assert.equal(planner.data[2], 1);
  validate();
  const narrow = multiply(
    perspective(Math.PI / 5, 1, 0.1, 200),
    lookAt([0, 1.7, 0], [0, 1.5, -10]),
  );
  for (const [eye, view] of [
    [[0, 1.7, 0], wideView],
    [[0, 1.7, 0], narrow],
    [[11, 1.7, 0], wideView],
    [[-11, 1.7, 20], narrow],
    [[0, 1.7, 0], wideView],
  ] as const) {
    planner.update(eye, view);
    validate();
  }
});

test("fixed grass clearings are bounded and leave outside roots unchanged", async () => {
  const clearings = [{ center: [-10, 9] as const, radius: 6 }];
  validateGrassManifest({ ...manifest(), clearings });
  assert.equal(grassClearingScale(-10, 9, clearings), 0);
  assert.equal(grassClearingScale(-5, 9, clearings), 0);
  assert.equal(grassClearingScale(-4, 9, clearings), 1);
  assert.equal(grassClearingScale(100, 100, clearings), 1);
  assert.equal(grassClearingScale(0, 0), 1);
  assert.ok(Math.abs(grassClearingScale(-4.375, 9, clearings) - 0.5) < 1e-12);
  for (const invalid of [
    [{ center: [0, 0], radius: 13 }],
    [{ center: [0, Infinity], radius: 6 }],
    [{ center: [0], radius: 6 }],
    Array.from({ length: 17 }, () => ({ center: [0, 0], radius: 1 })),
  ])
    assert.throws(() =>
      validateGrassManifest({
        ...manifest(),
        clearings: invalid,
      } as unknown as GrassManifest),
    );
  await gpuGlobals(async () => {
    const fake = fakeGPU(),
      grass = await Grass.create(fake.device, { ...manifest(), clearings });
    const state = frame();
    grass.prepare(fake.encoder, state);
    const uniform = new Float32Array(
      fake.writes.find((w) => w.buffer === fake.buffers[0])!.bytes,
    );
    assert.equal(uniform[124], 1);
    assert.deepEqual([...uniform.subarray(128, 132)], [-10, 9, 6, 0]);
    assert.ok(uniform.subarray(132).every((value) => value === 0));
    grass.dispose();
  });
});

test("grass creation destroys every allocated cache buffer when binding fails", async () =>
  gpuGlobals(async () => {
    const fake = fakeGPU(true);
    await assert.rejects(
      () => Grass.create(fake.device, manifest()),
      /Fixture group failure/,
    );
    assert.equal(fake.buffers.length, 4);
    assert.deepEqual(
      fake.buffers.map((b) => b.destroyed),
      [1, 1, 1, 1],
    );
  }));

test("live grass changes and shared wind update existing uniform storage atomically", async () =>
  gpuGlobals(async () => {
    const { DEFAULT_WIND_FIELD } =
      await import("../packages/renderer/shared-wind.ts");
    const fake = fakeGPU(),
      grass = await Grass.create(fake.device, manifest());
    const state = frame();
    grass.prepare(fake.encoder, state);
    grass.render(fake.pass, state);
    const pool = grass.planner.data;
    const previous = grass.manifest;
    assert.throws(() =>
      grass.setAppearance({
        bladeHeight: 3,
        bladeWidth: 0.05,
        windStrength: 1,
      }),
    );
    assert.equal(grass.manifest, previous);
    grass.setAppearance({
      bladeHeight: 0.75,
      bladeWidth: 0.05,
      windStrength: 0.5,
    });
    assert.equal(grass.manifest.bladeHeight, 0.75);
    assert.equal(grass.planner.manifest, grass.manifest);
    assert.equal(grass.planner.data, pool);
    state.sharedWind = { ...DEFAULT_WIND_FIELD, enabled: false };
    grass.prepare(fake.encoder, state);
    grass.render(fake.pass, state);
    const writes = fake.writes.filter(
      (entry) => entry.buffer === fake.buffers[0],
    );
    const legacy = new Float32Array(writes[0].bytes),
      current = new Float32Array(writes[1].bytes);
    assert.deepEqual([...legacy.subarray(112, 124)], new Array(12).fill(0));
    assert.equal(current[105], Math.fround(0.05));
    assert.equal(current[106], 0.75);
    assert.equal(current[107], 0.5);
    assert.equal(current[115], 0);
    assert.equal(current[120], 1);
    assert.equal(grass.stats.cacheRebuildBlades, 0);
    assert.equal(fake.buffers.length, 4);
    grass.dispose();
    assert.throws(() =>
      grass.setAppearance({
        bladeHeight: 0.5,
        bladeWidth: 0.05,
        windStrength: 1,
      }),
    );
  }));
