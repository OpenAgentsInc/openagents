import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import {
  SpellSwarm,
  SPELL_SWARM_DEFAULT_COUNT,
  SPELL_SWARM_TRIANGLES_PER_BOID,
  SPELL_SWARM_ORIENTATION_BYTES,
  validateSpellSwarmOptions,
} from "../packages/renderer/spell-swarm.ts";

Object.assign(globalThis, {
  GPUBufferUsage: {
    STORAGE: 1,
    COPY_DST: 2,
    COPY_SRC: 4,
    UNIFORM: 8,
    VERTEX: 16,
    INDEX: 32,
  },
  GPUShaderStage: { VERTEX: 1, FRAGMENT: 2, COMPUTE: 4 },
  GPUMapMode: { READ: 1 },
});

const identity = new Float32Array([
  1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
]);

function fakeGpu() {
  const buffers: {
    size: number;
    destroyed: number;
    label: string;
    storage: ArrayBuffer;
  }[] = [];
  const writes: { label: string; data: Uint8Array }[] = [];
  const pipelines: GPURenderPipelineDescriptor[] = [];
  const computePipelines: GPUComputePipelineDescriptor[] = [];
  const shaders: string[] = [];
  const draws: number[][] = [];
  const stages: { label: string; groups: number[]; bindings: unknown[] }[] = [];
  const renderBindings: unknown[] = [];
  const device = {
    features: new Set(),
    createBuffer(descriptor: GPUBufferDescriptor) {
      const buffer = {
        size: descriptor.size,
        label: descriptor.label ?? "",
        destroyed: 0,
        storage: new ArrayBuffer(descriptor.size),
        mapState: "unmapped",
        async mapAsync() {
          this.mapState = "mapped";
        },
        getMappedRange() {
          return this.storage;
        },
        unmap() {
          this.mapState = "unmapped";
        },
        destroy() {
          this.destroyed++;
        },
      };
      buffers.push(buffer);
      return buffer;
    },
    createCommandEncoder: () => ({
      copyBufferToBuffer(
        source: { storage: ArrayBuffer },
        sourceOffset: number,
        target: { storage: ArrayBuffer },
        targetOffset: number,
        size: number,
      ) {
        new Uint8Array(target.storage, targetOffset, size).set(
          new Uint8Array(source.storage, sourceOffset, size),
        );
      },
      finish: () => ({}),
    }),
    createShaderModule: (descriptor: GPUShaderModuleDescriptor) => {
      shaders.push(descriptor.code);
      return { code: descriptor.code };
    },
    createBindGroupLayout: () => ({}),
    createPipelineLayout: () => ({}),
    createComputePipeline: ({ compute }: GPUComputePipelineDescriptor) => ({
      entryPoint: compute.entryPoint,
    }),
    createComputePipelineAsync: async (
      descriptor: GPUComputePipelineDescriptor,
    ) => {
      computePipelines.push(descriptor);
      return { entryPoint: descriptor.compute.entryPoint };
    },
    createRenderPipelineAsync: async (
      descriptor: GPURenderPipelineDescriptor,
    ) => {
      pipelines.push(descriptor);
      return {};
    },
    createBindGroup: (descriptor: GPUBindGroupDescriptor) => descriptor,
    queue: {
      submit() {},
      writeBuffer(
        buffer: { label: string; storage: ArrayBuffer },
        offset: number,
        data: ArrayBuffer | ArrayBufferView,
        dataOffset = 0,
        size?: number,
      ) {
        const bytes = ArrayBuffer.isView(data)
          ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
          : new Uint8Array(data, dataOffset, size ?? data.byteLength);
        new Uint8Array(buffer.storage, offset, bytes.length).set(bytes);
        writes.push({ label: buffer.label, data: bytes.slice() });
      },
    },
  } as unknown as GPUDevice;
  const encoder = {
    beginComputePass({ label }: GPUComputePassDescriptor) {
      const stage = {
        label: String(label),
        groups: [] as number[],
        bindings: [] as unknown[],
      };
      stages.push(stage);
      return {
        setPipeline() {},
        setBindGroup(_index: number, group: unknown) {
          stage.bindings.push(group);
        },
        dispatchWorkgroups(groups: number) {
          stage.groups.push(groups);
        },
        end() {},
      };
    },
  } as unknown as GPUCommandEncoder;
  const pass = {
    setPipeline() {},
    setBindGroup(index: number, group: unknown) {
      renderBindings[index] = group;
    },
    setVertexBuffer() {},
    setIndexBuffer() {},
    drawIndexed(...args: number[]) {
      draws.push(args);
    },
  } as unknown as GPURenderPassEncoder;
  return {
    device,
    encoder,
    pass,
    buffers,
    writes,
    pipelines,
    computePipelines,
    shaders,
    draws,
    stages,
    renderBindings,
  };
}

test("the default spell preserves the lab population and complete mesh", () => {
  const options = validateSpellSwarmOptions();
  assert.equal(options.count, SPELL_SWARM_DEFAULT_COUNT);
  assert.equal(options.count, 100_000);
  assert.equal(SPELL_SWARM_TRIANGLES_PER_BOID, 310);
  assert.equal(options.count * SPELL_SWARM_TRIANGLES_PER_BOID, 31_000_000);
  assert.equal(options.format, "rgba16float");
  assert.equal(options.depthFormat, "depth24plus");
  assert.equal(options.sampleCount, 4);
  assert.equal(options.orientationPreparation, "vertex");
  assert.ok(Object.isFrozen(options));
  assert.ok(Object.isFrozen(options.center));
});

test("spell settings are bounded and snapshot the supplied world center", () => {
  const center: [number, number, number] = [1, 2, 3];
  const config = validateSpellSwarmOptions({ center });
  center[0] = 10;
  assert.deepEqual(config.center, [1, 2, 3]);
  for (const options of [
    { count: 0 },
    { count: 200_001 },
    { count: 1.5 },
    { radius: NaN },
    { radius: 0 },
    { radius: 65 },
    { size: 0 },
    { size: Infinity },
    { size: 2 },
    { seed: -1 },
    { seed: 1.5 },
    { seed: 0x1_0000_0000 },
    { center: [NaN, 0, 0] as const },
    { center: [0, 1_000_001, 0] as const },
    { sampleCount: 2 as 1 },
    { orientationPreparation: "other" as "vertex" },
  ])
    assert.throws(() => validateSpellSwarmOptions(options), RangeError);
});

test("the shared Town pipeline keeps HDR, depth testing, MSAA, and all mesh indices", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, { count: 64 });
  const pipeline = gpu.pipelines[0];
  assert.equal(pipeline.fragment?.targets[0]?.format, "rgba16float");
  assert.equal(pipeline.depthStencil?.format, "depth24plus");
  assert.equal(pipeline.depthStencil?.depthWriteEnabled, true);
  assert.equal(pipeline.multisample?.count, 4);
  assert.equal(pipeline.primitive?.cullMode, "back");
  assert.equal(swarm.render(gpu.pass, identity, [0, 1, 2], 3), true);
  assert.deepEqual(gpu.draws, [[930, 64]]);
  const metrics = swarm.inspect();
  assert.equal(metrics.trianglesPerDraw, 64 * 310);
  assert.equal(metrics.submittedDraws, 1);
  assert.equal(metrics.gpuComputeSample, null);
  assert.equal(metrics.gpuTimingSupported, false);
  assert.equal(
    metrics.accountedGpuBytes,
    gpu.buffers.reduce((sum, b) => sum + b.size, 0),
  );
  swarm.dispose();
});

test("each spell step encodes clear, scatter, update and draws the resulting GPU state", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, { count: 257 });
  swarm.render(gpu.pass, identity, [0, 1, 2], 0);
  const originalState = gpu.renderBindings[1];
  assert.equal(swarm.encodeSimulation(gpu.encoder, 1 / 60, 0), true);
  assert.deepEqual(
    gpu.stages.map((s) => s.label),
    ["clear cells", "aggregate boids", "steer boids"],
  );
  assert.deepEqual(
    gpu.stages.map((s) => s.groups),
    [[256], [3], [3]],
  );
  swarm.render(gpu.pass, identity, [0, 1, 2], 1 / 60);
  assert.notEqual(gpu.renderBindings[1], originalState);
  assert.throws(
    () => swarm.encodeSimulation(gpu.encoder, 1 / 60, 1),
    /Submit the preceding/,
  );
  swarm.afterSubmit();
  swarm.encodeSimulation(gpu.encoder, 1 / 60, 1 / 60);
  swarm.render(gpu.pass, identity, [0, 1, 2], 2 / 60);
  assert.equal(gpu.renderBindings[1], originalState);
  swarm.afterSubmit();
  assert.equal(swarm.inspect().simulationSteps, 2);
  swarm.dispose();
});

test("inactive spells do no upload, allocation, compute, or draw work", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, { count: 1 });
  const counts = [
    gpu.buffers.length,
    gpu.writes.length,
    gpu.draws.length,
    gpu.stages.length,
  ];
  swarm.setEnabled(false);
  assert.equal(swarm.encodeSimulation(gpu.encoder, 1 / 60, 1), false);
  assert.equal(swarm.render(gpu.pass, identity, [0, 1, 2], 1), false);
  assert.equal(swarm.prepareRender(gpu.encoder), false);
  swarm.afterSubmit();
  assert.deepEqual(
    [
      gpu.buffers.length,
      gpu.writes.length,
      gpu.draws.length,
      gpu.stages.length,
    ],
    counts,
  );
  assert.equal(swarm.inspect().enabled, false);
  swarm.setEnabled(true);
  assert.equal(swarm.render(gpu.pass, identity, [0, 1, 2], 1), true);
  swarm.dispose();
});

test("changing the world center leaves local simulation state intact", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, { count: 1 });
  const stateUploads = gpu.writes.filter((w) =>
    w.label.startsWith("boids-state"),
  ).length;
  swarm.setCenter([100, 20, -30]);
  swarm.render(gpu.pass, identity, [0, 1, 2], 2);
  const uniform = gpu.writes.findLast(
    (w) => w.label === "Spell swarm: camera and world center",
  )!;
  const words = new Float32Array(uniform.data.buffer);
  assert.deepEqual(Array.from(words.subarray(20, 23)), [100, 20, -30]);
  assert.deepEqual(swarm.inspect().center, [100, 20, -30]);
  assert.equal(
    gpu.writes.filter((w) => w.label.startsWith("boids-state")).length,
    stateUploads,
  );
  swarm.dispose();
});

test("zero-time frames do not dispatch and invalid input cannot queue a step", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, { count: 1 });
  assert.equal(swarm.encodeSimulation(gpu.encoder, 0, 1), false);
  for (const dt of [-1, NaN, Infinity, 0.11])
    assert.throws(() => swarm.encodeSimulation(gpu.encoder, dt, 1), RangeError);
  assert.throws(
    () => swarm.encodeSimulation(gpu.encoder, 1 / 60, NaN),
    RangeError,
  );
  assert.throws(
    () => swarm.render(gpu.pass, identity, [0, NaN, 2], 0),
    RangeError,
  );
  assert.equal(gpu.stages.length, 0);
  assert.equal(gpu.draws.length, 0);
  swarm.dispose();
});

test("closing a swarm destroys each owned buffer once and never the shared device", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, { count: 1 });
  swarm.dispose();
  swarm.dispose();
  swarm.afterSubmit();
  assert.ok(gpu.buffers.every((buffer) => buffer.destroyed === 1));
  assert.equal(swarm.inspect().enabled, false);
  assert.equal(swarm.inspect().accountedGpuBytes, 0);
  assert.throws(() => swarm.setEnabled(true), /closed/);
  assert.throws(() => swarm.setCenter([0, 0, 0]), /closed/);
  assert.throws(() => swarm.render(gpu.pass, identity, [0, 0, 0], 0), /closed/);
});

test("Town owns timing queries even when the shared device supports timestamps", async () => {
  const gpu = fakeGpu();
  (gpu.device.features as unknown as Set<string>).add("timestamp-query");
  const swarm = await SpellSwarm.create(gpu.device, { count: 1 });
  swarm.encodeSimulation(gpu.encoder, 1 / 60, 0);
  swarm.afterSubmit();
  assert.equal(swarm.inspect().gpuComputeSample, null);
  assert.equal(swarm.inspect().gpuTimingSupported, false);
  assert.equal(
    swarm.inspect().accountedGpuBytes,
    gpu.buffers.reduce((sum, b) => sum + b.size, 0),
  );
  swarm.dispose();
});

test("an explicit probe hashes complete GPU state and reports eight world positions", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, {
    count: 12,
    center: [10, 20, -30],
  });
  const state = gpu.buffers.find((buffer) => buffer.label === "boids-state-0")!;
  const words = new Float32Array(state.storage);
  const buffersBefore = gpu.buffers.length;
  const probe = await swarm.probe();
  assert.equal(probe.count, 12);
  assert.equal(probe.finite, true);
  assert.equal(probe.simulationSteps, 0);
  assert.equal(probe.positions.length, 8);
  assert.deepEqual(probe.positions[0], [
    words[0] + 10,
    words[1] + 20,
    words[2] - 30,
  ]);
  assert.equal(
    probe.stateSha256,
    createHash("sha256").update(new Uint8Array(state.storage)).digest("hex"),
  );
  assert.ok(
    probe.maxDistanceFromCenter > 0 && probe.maxDistanceFromCenter <= 12.000001,
  );
  assert.equal(gpu.buffers.length, buffersBefore + 1);
  assert.equal(gpu.buffers.at(-1)?.destroyed, 1);
  assert.equal(gpu.stages.length, 0);
  assert.equal(gpu.draws.length, 0);
  swarm.encodeSimulation(gpu.encoder, 1 / 60, 0);
  swarm.afterSubmit();
  const next = await swarm.probe();
  assert.equal(next.simulationSteps, 1);
  assert.equal(next.finite, true);
  swarm.dispose();
  await assert.rejects(swarm.probe(), /closed/);
});

test("a probe checks every state word and hashes even a nonfinite tail velocity", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, { count: 12 });
  const state = gpu.buffers.find((buffer) => buffer.label === "boids-state-0")!;
  const words = new Float32Array(state.storage);
  words[words.length - 2] = NaN;
  const probe = await swarm.probe();
  assert.equal(probe.finite, false);
  assert.equal(
    probe.stateSha256,
    createHash("sha256").update(new Uint8Array(state.storage)).digest("hex"),
  );
  swarm.dispose();
});

function boundBuffer(group: unknown, binding: number): GPUBuffer {
  return (
    group as { entries: { binding: number; resource: { buffer: GPUBuffer } }[] }
  ).entries.find((entry) => entry.binding === binding)!.resource.buffer;
}

test("the vertex baseline adds no orientation buffers, pipelines, or compute passes", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, { count: 257 });
  const before = gpu.buffers.length;
  assert.equal(swarm.prepareRender(gpu.encoder), false);
  assert.equal(gpu.stages.length, 0);
  assert.equal(gpu.computePipelines.length, 0);
  assert.equal(gpu.buffers.length, before);
  assert.equal(swarm.inspect().orientationPreparation, "vertex");
  assert.equal(swarm.inspect().orientationBytes, 0);
  assert.equal(swarm.inspect().orientationPasses, 0);
  swarm.render(gpu.pass, identity, [0, 1, 2], 0);
  assert.deepEqual(gpu.draws, [[930, 257]]);
  swarm.dispose();
});

test("GPU orientation uses one fixed 48-byte record per boid and preserves every triangle", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, {
    count: 257,
    orientationPreparation: "compute",
  });
  const orientation = gpu.buffers.find(
    (buffer) => buffer.label === "Spell swarm: prepared orientation and seed",
  )!;
  assert.equal(orientation.size, 257 * SPELL_SWARM_ORIENTATION_BYTES);
  assert.equal(SPELL_SWARM_ORIENTATION_BYTES, 48);
  assert.equal(100_000 * SPELL_SWARM_ORIENTATION_BYTES, 4_800_000);
  assert.equal(gpu.computePipelines.length, 1);
  assert.equal(
    gpu.computePipelines[0].compute.entryPoint,
    "prepareOrientation",
  );
  assert.equal(swarm.inspect().orientationPreparation, "compute");
  assert.equal(swarm.inspect().orientationBytes, 257 * 48);
  assert.equal(
    swarm.inspect().accountedGpuBytes,
    gpu.buffers.reduce((sum, b) => sum + b.size, 0),
  );
  assert.throws(
    () => swarm.render(gpu.pass, identity, [0, 1, 2], 0),
    /Prepare spell orientation/,
  );
  assert.equal(swarm.prepareRender(gpu.encoder), true);
  assert.equal(swarm.render(gpu.pass, identity, [0, 1, 2], 0), true);
  assert.deepEqual(gpu.draws, [[930, 257]]);
  assert.equal(boundBuffer(gpu.renderBindings[1], 1), orientation);
  swarm.dispose();
  assert.ok(gpu.buffers.every((buffer) => buffer.destroyed === 1));
  assert.equal(swarm.inspect().orientationBytes, 0);
});

test("orientation preparation follows actual simulation ping-pong state bindings", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, {
    count: 257,
    orientationPreparation: "compute",
  });
  swarm.prepareRender(gpu.encoder);
  swarm.render(gpu.pass, identity, [0, 1, 2], 0);
  const initial = boundBuffer(gpu.stages[0].bindings[0], 0);
  assert.equal(initial, boundBuffer(gpu.renderBindings[1], 0));
  swarm.encodeSimulation(gpu.encoder, 1 / 60, 1 / 60);
  assert.throws(
    () => swarm.render(gpu.pass, identity, [0, 1, 2], 1 / 60),
    /Prepare spell orientation/,
  );
  swarm.prepareRender(gpu.encoder);
  swarm.render(gpu.pass, identity, [0, 1, 2], 1 / 60);
  const afterStep = boundBuffer(gpu.stages[4].bindings[0], 0);
  assert.notEqual(afterStep, initial);
  assert.equal(afterStep, boundBuffer(gpu.renderBindings[1], 0));
  assert.equal(afterStep, boundBuffer(gpu.stages[1].bindings[0], 1));
  swarm.afterSubmit();
  swarm.encodeSimulation(gpu.encoder, 1 / 60, 2 / 60);
  swarm.prepareRender(gpu.encoder);
  swarm.render(gpu.pass, identity, [0, 1, 2], 2 / 60);
  assert.equal(boundBuffer(gpu.stages[8].bindings[0], 0), initial);
  assert.equal(boundBuffer(gpu.renderBindings[1], 0), initial);
  assert.deepEqual(
    gpu.stages.map((stage) => stage.groups),
    [[3], [256], [3], [3], [3], [256], [3], [3], [3]],
  );
  swarm.afterSubmit();
  swarm.dispose();
});

test("still captures prepare initial and repeated orientation without advancing simulation", async () => {
  const gpu = fakeGpu();
  const swarm = await SpellSwarm.create(gpu.device, {
    count: 12,
    orientationPreparation: "compute",
  });
  for (let capture = 0; capture < 2; capture++) {
    swarm.prepareRender(gpu.encoder);
    swarm.render(gpu.pass, identity, [0, 1, 2], capture);
  }
  assert.equal(swarm.inspect().simulationSteps, 0);
  assert.equal(swarm.inspect().orientationPasses, 2);
  assert.deepEqual(
    gpu.stages.map((stage) => stage.label),
    ["prepare spell orientation", "prepare spell orientation"],
  );
  assert.equal(
    boundBuffer(gpu.stages[0].bindings[0], 0),
    boundBuffer(gpu.stages[1].bindings[0], 0),
  );
  const counts = [
    gpu.buffers.length,
    gpu.writes.length,
    gpu.stages.length,
    gpu.draws.length,
  ];
  swarm.setEnabled(false);
  assert.equal(swarm.prepareRender(gpu.encoder), false);
  assert.equal(swarm.render(gpu.pass, identity, [0, 1, 2], 3), false);
  assert.deepEqual(
    [
      gpu.buffers.length,
      gpu.writes.length,
      gpu.stages.length,
      gpu.draws.length,
    ],
    counts,
  );
  swarm.dispose();
});
