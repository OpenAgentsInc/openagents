import assert from "node:assert/strict";
import test from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";
import { createCompiledSourceHost } from "../packages/renderer/compiled-source-host.ts";
import { buildInstanceBounds } from "../packages/renderer/culling.ts";
import {
  lookAt,
  modelMatrix,
  multiply,
  perspective,
} from "../packages/renderer/math.ts";

Object.assign(globalThis, {
  GPUBufferUsage: { VERTEX: 1, COPY_DST: 2, INDIRECT: 4 },
});
class FakeBuffer {
  readonly data: Uint8Array;
  destroyed = false;
  constructor(
    readonly size: number,
    readonly label: string,
  ) {
    this.data = new Uint8Array(size);
  }
  destroy() {
    this.destroyed = true;
  }
}
const matrix = (back = false) =>
  multiply(
    perspective(1.2, 1, 0.1, 100),
    lookAt([0, 0, 0], [0, 0, back ? 1 : -1]),
  );
const bits = (array: Float32Array) => [
  ...new Uint32Array(array.buffer, array.byteOffset, array.length),
];
function fixture(
  t: any,
  options: { indirect?: boolean; capacity?: number; compiled?: boolean } = {},
) {
  const instances = Array.from({ length: 6 }, (_, id) => ({
    mesh: 0,
    model: modelMatrix([((id % 3) - 1) * 2, id * 0.01, id < 3 ? -6 : 6], 0),
  }));
  const scene = {
    instances,
    materials: [
      { alphaMode: "OPAQUE" },
      { alphaMode: "OPAQUE" },
      { alphaMode: "MASK", vegetation: true },
    ],
    meshes: [{ bounds: { min: [-0.1, -0.1, -0.1], max: [0.1, 0.1, 0.1] } }],
  };
  const draws = [
    {
      sourceDrawIndex: 0,
      firstIndex: 0,
      indexCount: 3,
      baseVertex: 0,
      firstInstance: 0,
      count: 2,
      material: 0,
      wind: false,
      rasterOrientation: "positive",
    },
    {
      sourceDrawIndex: 1,
      firstIndex: 3,
      indexCount: 6,
      baseVertex: 4,
      firstInstance: 2,
      count: 2,
      material: 1,
      wind: false,
      rasterOrientation: "positive",
    },
    {
      sourceDrawIndex: 2,
      firstIndex: 0,
      indexCount: 3,
      baseVertex: 0,
      firstInstance: 4,
      count: 2,
      material: 0,
      wind: false,
      rasterOrientation: "positive",
    },
    {
      sourceDrawIndex: 3,
      firstIndex: 9,
      indexCount: 3,
      baseVertex: -2,
      firstInstance: 0,
      count: 6,
      material: 2,
      wind: true,
      rasterOrientation: "negative",
    },
  ];
  const models = Float32Array.from(instances.flatMap((item) => item.model));
  const wind = Float32Array.from({ length: 24 }, (_, index) => index * 0.125);
  const uploads: { buffer: FakeBuffer; bytes: Uint8Array }[] = [];
  let fail = "",
    deviceDestroyed = false;
  const buffers = new Set<FakeBuffer>();
  const device = {
    createBuffer({ size, label }: { size: number; label: string }) {
      const value = new FakeBuffer(size, label);
      buffers.add(value);
      return value;
    },
    queue: {
      writeBuffer(buffer: FakeBuffer, offset: number, array: ArrayBufferView) {
        if (fail && buffer.label.includes(fail)) {
          fail = "";
          throw Error("Mock source upload failure");
        }
        const bytes = new Uint8Array(
          array.buffer,
          array.byteOffset,
          array.byteLength,
        );
        assert(
          offset + bytes.length <= buffer.size,
          "A source upload must fit its actual GPU allocation.",
        );
        buffer.data.set(bytes, offset);
        uploads.push({ buffer, bytes: bytes.slice() });
      },
    },
    destroy() {
      deviceDestroyed = true;
    },
  };
  const sourceInstances = device.createBuffer({
    size: models.byteLength,
    label: "Original instances",
  });
  device.queue.writeBuffer(sourceInstances, 0, models);
  const windBuffer = device.createBuffer({
    size: wind.byteLength,
    label: "Original wind",
  });
  device.queue.writeBuffer(windBuffer, 0, wind);
  const variant = {
    scene,
    models,
    draws,
    bounds: buildInstanceBounds(scene as any),
    wind: { parameters: wind },
    windBuffer,
    instances: sourceInstances,
  };
  const renderer: any = new Renderer();
  Object.assign(renderer, {
    scene,
    variants: new Map([["day", variant]]),
    renderMaterialInfo: scene.materials,
    experiment: {
      backend: "baseline",
      profiling: false,
      compiledSourceBatches: options.compiled !== false,
    },
    features: { createCompiledSourceHost },
    compiledSourceHost:
      options.compiled === false ? undefined : createCompiledSourceHost(),
    cullingEnabled: true,
    activeShaderSource: "stock",
    shaders: { MATERIAL: "stock" },
    device,
    sourceBundlesEnabled: options.indirect !== false,
    instances: sourceInstances,
    draws,
    zeroWind: new FakeBuffer((options.capacity ?? 16) * 16, "Fixed zero wind"),
    gpuBytes: 0,
    preparedBufferUploadBytes: 0,
    releaseBuffer(buffer: FakeBuffer | undefined) {
      if (buffer) {
        buffer.destroy();
        renderer.buffers.delete(buffer);
        renderer.gpuBytes -= buffer.size;
      }
    },
  });
  t.after(() => renderer.dispose());
  return {
    renderer,
    variant,
    scene,
    models,
    wind,
    draws,
    uploads,
    buffers,
    prepare: (back = false, view = "camera") =>
      renderer.prepareSourceSubmission(matrix(back), view, "day"),
    failUpload: (label: string) => {
      fail = label;
    },
    deviceDestroyed: () => deviceDestroyed,
  };
}
function placementUses(f: ReturnType<typeof fixture>, submission: any) {
  const uploaded = new Float32Array(submission.instances.data.buffer);
  const wind = new Float32Array(submission.wind.data.buffer);
  return submission.draws.map((draw: any) => {
    const placements: number[] = [];
    for (
      let instance = draw.firstInstance;
      instance < draw.firstInstance + draw.count;
      instance++
    ) {
      const model = bits(uploaded.subarray(instance * 16, instance * 16 + 16));
      const id = f.scene.instances.findIndex(
        (_item, index) =>
          JSON.stringify(model) ===
          JSON.stringify(bits(f.models.subarray(index * 16, index * 16 + 16))),
      );
      assert(
        id >= 0,
        "Every uploaded matrix must identify a real original placement.",
      );
      assert.deepEqual(
        bits(wind.subarray(instance * 4, instance * 4 + 4)),
        bits(f.wind.subarray(id * 4, id * 4 + 4)),
        "Every primitive use keeps its original wind bits.",
      );
      placements.push(id);
    }
    return {
      firstIndex: draw.firstIndex,
      indexCount: draw.indexCount,
      baseVertex: draw.baseVertex,
      material: draw.material,
      placements,
    };
  });
}
function expectedGrouped(visible: number[], excluded: number[] = []) {
  const allowed = (id: number) =>
    visible.includes(id) && !excluded.includes(id);
  return [
    {
      firstIndex: 0,
      indexCount: 3,
      baseVertex: 0,
      material: 0,
      placements: [0, 1, 4, 5].filter(allowed),
    },
    {
      firstIndex: 3,
      indexCount: 6,
      baseVertex: 4,
      material: 1,
      placements: [2, 3].filter(allowed),
    },
    {
      firstIndex: 9,
      indexCount: 3,
      baseVertex: -2,
      material: 2,
      placements: [0, 1, 2, 3, 4, 5].filter(allowed),
    },
  ].filter((item) => item.placements.length);
}
function expectedOriginal(visible: number[], excluded: number[] = []) {
  const allowed = (id: number) =>
    visible.includes(id) && !excluded.includes(id);
  return [
    {
      firstIndex: 0,
      indexCount: 3,
      baseVertex: 0,
      material: 0,
      placements: [0, 1].filter(allowed),
    },
    {
      firstIndex: 3,
      indexCount: 6,
      baseVertex: 4,
      material: 1,
      placements: [2, 3].filter(allowed),
    },
    {
      firstIndex: 0,
      indexCount: 3,
      baseVertex: 0,
      material: 0,
      placements: [4, 5].filter(allowed),
    },
    {
      firstIndex: 9,
      indexCount: 3,
      baseVertex: -2,
      material: 2,
      placements: [0, 1, 2, 3, 4, 5].filter(allowed),
    },
  ].filter((item) => item.placements.length);
}
function indirect(submission: any) {
  const words = [...new Uint32Array(submission.bundles.indirect.data.buffer)];
  return Array.from({ length: words.length / 5 }, (_, index) =>
    words.slice(index * 5, index * 5 + 5),
  );
}

test("actual renderer gathers primitive uses into GPU buffers and matching fixed indirect slots", (t) => {
  const f = fixture(t),
    original = structuredClone(f.draws),
    first = f.prepare();
  assert(first.compiled);
  assert.equal(first.compiled.plan.counts.sourceDraws, 4);
  assert.equal(first.compiled.plan.counts.groupedDraws, 3);
  assert.deepEqual(placementUses(f, first), expectedGrouped([0, 1, 2]));
  assert.deepEqual(indirect(first), [
    [3, 2, 0, 0, 0],
    [6, 1, 3, 4, 2],
    [3, 3, 9, 0xfffffffe, 3],
  ]);
  assert.equal(first.instances.size, 12 * 64);
  assert.equal(first.wind.size, 12 * 16);
  const uploads = f.uploads.length;
  assert.equal(f.prepare(), first);
  assert.equal(
    f.uploads.length,
    uploads,
    "Identical camera/generation must reuse already uploaded data.",
  );
  assert.deepEqual(f.draws, original);
});

test("camera and planar views keep separate gather slabs and original source mappings", (t) => {
  const f = fixture(t),
    front = f.prepare(),
    gatherer = front.compiled;
  const back = f.prepare(true);
  assert.equal(back.compiled, gatherer);
  assert.deepEqual(placementUses(f, back), expectedGrouped([3, 4, 5]));
  const cameraBytes = back.instances.data.slice(),
    cameraDraws = structuredClone(back.draws);
  const reflected = f.prepare(false, "planar");
  assert.notEqual(reflected.compiled, gatherer);
  assert.equal(reflected.compiled.plan, gatherer.plan);
  assert.notEqual(reflected.instances, back.instances);
  assert.deepEqual(placementUses(f, reflected), expectedGrouped([0, 1, 2]));
  assert.deepEqual(back.instances.data, cameraBytes);
  assert.deepEqual(back.draws, cameraDraws);
});

test("damage clears every grouped use and indirect slot; restore recovers exact uploaded source bits", (t) => {
  const f = fixture(t),
    first = f.prepare(),
    originalBytes = first.instances.data.slice();
  const gatherer = first.compiled;
  f.renderer.destroyedPlacements.set("day", new Set([1, 2, 4]));
  f.renderer.geometryVersion++;
  const damaged = f.prepare();
  assert.equal(damaged.compiled, gatherer);
  assert.deepEqual(
    placementUses(f, damaged),
    expectedGrouped([0, 1, 2], [1, 2, 4]),
  );
  assert.deepEqual(indirect(damaged), [
    [3, 1, 0, 0, 0],
    [6, 0, 3, 4, 0],
    [3, 1, 9, 0xfffffffe, 1],
  ]);
  f.renderer.destroyedPlacements.set("day", new Set([0, 1, 2, 3, 4, 5]));
  f.renderer.geometryVersion++;
  const empty = f.prepare();
  assert.deepEqual(empty.draws, []);
  assert(indirect(empty).every((row) => row[1] === 0));
  f.renderer.destroyedPlacements.clear();
  f.renderer.geometryVersion++;
  const restored = f.prepare();
  assert.deepEqual(placementUses(f, restored), expectedGrouped([0, 1, 2]));
  assert.deepEqual(restored.instances.data, originalBytes);
});

test("source transform changes refresh gathered GPU bytes at the new geometry version", (t) => {
  const f = fixture(t),
    first = f.prepare(),
    gatherer = first.compiled;
  f.models[2 * 16 + 12] = 1.25;
  f.models[2 * 16 + 14] = -8;
  f.scene.instances[2].model = [...f.models.subarray(2 * 16, 2 * 16 + 16)];
  f.variant.bounds = buildInstanceBounds(f.scene as any);
  f.renderer.geometryVersion++;
  const moved = f.prepare();
  assert.equal(moved.compiled, gatherer);
  assert.deepEqual(placementUses(f, moved), expectedGrouped([0, 1, 2]));
  const gpu = new Float32Array(moved.instances.data.buffer);
  assert.equal(gpu[2 * 16 + 12], 1.25);
  assert.equal(gpu[2 * 16 + 14], -8);
});

test("custom shaders retain original source ranges and stock restoration recreates valid grouped draws", (t) => {
  const f = fixture(t),
    first = f.prepare(),
    oldGatherer = first.compiled;
  f.renderer.activeShaderSource = "custom";
  f.renderer.cullingEnabled = false;
  f.renderer.destroyedPlacements.set("day", new Set([1]));
  f.renderer.geometryVersion++;
  const custom = f.prepare();
  assert.equal(custom.compiled, undefined);
  assert.deepEqual(
    placementUses(f, custom),
    expectedOriginal([0, 1, 2, 3, 4, 5], [1]),
  );
  assert.equal(
    oldGatherer.memoryBytes,
    0,
    "Publishing a replacement releases the retired gather slabs.",
  );
  assert.throws(() => oldGatherer.gather({}), /closed/);
  f.renderer.activeShaderSource = "stock";
  f.renderer.cullingEnabled = true;
  f.renderer.destroyedPlacements.clear();
  f.renderer.geometryVersion++;
  const restored = f.prepare();
  assert(restored.compiled);
  assert.notEqual(restored.compiled, oldGatherer);
  assert.deepEqual(placementUses(f, restored), expectedGrouped([0, 1, 2]));
});

test("without indirect support compiled source buffers preserve every primitive use and honest accounting", (t) => {
  const f = fixture(t, { indirect: false }),
    first = f.prepare();
  assert(first.compiled);
  assert.equal(first.bundles, undefined);
  assert.deepEqual(placementUses(f, first), expectedGrouped([0, 1, 2]));
  const reflected = f.prepare(true, "planar");
  assert.deepEqual(placementUses(f, reflected), expectedGrouped([3, 4, 5]));
  const stats = f.renderer.benchmarkInfo().sourceSubmission;
  assert.equal(stats.compiledViews, 2);
  assert.equal(stats.bundles, 0);
  assert.equal(
    stats.compiledMembershipBytes,
    first.compiled.plan.footprint.planBytes,
  );
  assert.equal(
    stats.compiledGatherBytes,
    first.compiled.memoryBytes + reflected.compiled.memoryBytes,
  );
});

test("fixed zero-wind capacity and disabled experiments retain complete original rendering", (t) => {
  for (const options of [{ capacity: 8 }, { compiled: false }]) {
    const f = fixture(t, options),
      value = f.prepare();
    assert.equal(value.compiled, undefined);
    assert.deepEqual(placementUses(f, value), expectedOriginal([0, 1, 2]));
    assert.equal(value.bundles.draws.length, 4);
  }
});

test("a failed compiled indirect upload invalidates the cached signature and identical retry repairs the GPU", (t) => {
  const f = fixture(t),
    first = f.prepare(),
    before = indirect(first);
  f.renderer.destroyedPlacements.set("day", new Set([1]));
  f.renderer.geometryVersion++;
  f.failUpload("Source indexed indirect commands");
  assert.throws(() => f.prepare(), /Mock source upload failure/);
  assert.equal(first.signature, "");
  assert.deepEqual(
    indirect(first),
    before,
    "A failed write leaves the actual GPU indirect bytes unchanged.",
  );
  const repaired = f.prepare();
  assert.equal(repaired.bundles, first.bundles);
  assert.deepEqual(placementUses(f, repaired), expectedGrouped([0, 1, 2], [1]));
  assert.equal(indirect(repaired)[0][1], 1);
});

test("project close releases every compiled view slab and closes the fake graphics device", (t) => {
  const f = fixture(t),
    camera = f.prepare(),
    planar = f.prepare(true, "planar");
  const first = camera.compiled,
    second = planar.compiled;
  f.renderer.dispose();
  assert.equal(first.memoryBytes, 0);
  assert.equal(second.memoryBytes, 0);
  assert.equal(f.renderer.culledSources.size, 0);
  assert.equal(f.deviceDestroyed(), true);
  assert.throws(() => camera.bundles.commandSnapshot(), /closed/);
});

test("versioned in-place source draw metadata changes invalidate the old compiled plan", (t) => {
  const f = fixture(t),
    initial = f.prepare(),
    gatherer = initial.compiled;
  f.draws[2].firstIndex = 12;
  f.renderer.geometryVersion++;
  const changed = f.prepare(true);
  assert.equal(
    changed.compiled,
    undefined,
    "The two formerly identical source primitives no longer merge.",
  );
  const wanted = expectedOriginal([3, 4, 5]);
  wanted.find((value) => value.material === 0)!.firstIndex = 12;
  assert.deepEqual(placementUses(f, changed), wanted);
  assert.equal(gatherer.memoryBytes, 0);
  f.draws[2].firstIndex = 0;
  f.renderer.geometryVersion++;
  assert(
    f.prepare().compiled,
    "Restoring source metadata can rebuild the original compatible plan.",
  );
});

test("versioned in-place material eligibility changes fall back completely and recover", (t) => {
  const f = fixture(t),
    initial = f.prepare(),
    gatherer = initial.compiled;
  f.scene.materials[0].alphaMode = "BLEND";
  f.renderer.geometryVersion++;
  const changed = f.prepare();
  assert.equal(changed.compiled, undefined);
  assert.deepEqual(placementUses(f, changed), expectedOriginal([0, 1, 2]));
  assert.equal(gatherer.memoryBytes, 0);
  f.scene.materials[0].alphaMode = "OPAQUE";
  f.renderer.geometryVersion++;
  const restored = f.prepare();
  assert(restored.compiled);
  assert.deepEqual(placementUses(f, restored), expectedGrouped([0, 1, 2]));
});

test("replacing a source table invalidates a matching camera signature even without a version change", (t) => {
  const f = fixture(t),
    initial = f.prepare(),
    gatherer = initial.compiled;
  f.variant.draws = f.draws.map((draw) => ({
    ...draw,
    indexCount: draw.material === 0 ? 6 : draw.indexCount,
  }));
  const changed = f.prepare();
  assert(changed.compiled);
  assert.notEqual(changed.compiled.plan, gatherer.plan);
  const wanted = expectedGrouped([0, 1, 2]);
  wanted[0].indexCount = 6;
  assert.deepEqual(placementUses(f, changed), wanted);
  assert.equal(indirect(changed)[0][0], 6);
  assert.equal(gatherer.memoryBytes, 0);
});
