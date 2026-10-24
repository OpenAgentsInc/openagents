import assert from "node:assert/strict";
import test from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";
import {
  PackedFrameCore,
  bindResidentFrameScene,
  createResidentSourcePreparation,
} from "../packages/renderer/resident-source.ts";
import { PackedFrameCore as CompatibleCore } from "../experiments/c-town/packed-frame.ts";
import { bindResidentFrameScene as compatibleBind } from "../experiments/c-town/resident-frame.ts";
import {
  buildInstanceBounds,
  compactInstanceDraws,
  frustumVisibility,
} from "../packages/renderer/culling.ts";
import {
  modelMatrix,
  multiply,
  perspective,
  lookAt,
} from "../packages/renderer/math.ts";

function source() {
  const instances = Array.from({ length: 67 }, (_, index) => ({
    mesh: 0,
    model: modelMatrix(
      [((index % 9) - 4) * 4, (index % 3) * 2, -Math.floor(index / 9) * 4 - 2],
      index * 0.2,
    ),
  }));
  const models = new Float32Array(
    instances.flatMap((instance) => instance.model),
  );
  const scene = {
    meshes: [{ bounds: { min: [-0.5, -1, -0.2], max: [0.5, 1, 0.2] } }],
    instances,
  } as Parameters<typeof buildInstanceBounds>[0];
  return {
    bounds: buildInstanceBounds(scene),
    models,
    draws: [
      { firstInstance: 0, count: 32 },
      { firstInstance: 0, count: 32 },
      { firstInstance: 32, count: 35 },
    ],
    wind: Float32Array.from({ length: 67 * 4 }, (_, index) => index * 0.125),
  };
}
const camera = (step = 0) =>
  multiply(
    perspective(0.8, 1.7, 0.1, 1000),
    lookAt([Math.sin(step) * 10, 3, Math.cos(step) * 10], [0, 2, -16]),
  );

test("production and experimental imports share one exact culling implementation", () => {
  assert.equal(CompatibleCore, PackedFrameCore);
  assert.equal(compatibleBind, bindResidentFrameScene);
  const core = new PackedFrameCore(),
    data = source();
  const handle = core.prepareScene(
    data.bounds,
    data.models,
    data.draws,
    data.wind,
  );
  const sourceModels = data.models.slice(),
    sourceWind = data.wind.slice();
  for (let frame = 0; frame < 60; frame++) {
    const matrix = camera(frame * 0.1);
    const visible = frustumVisibility(data.bounds, matrix);
    const expected = compactInstanceDraws(data.models, data.draws, visible);
    const actual = core.cull(handle, matrix);
    assert.deepEqual(actual.visibility, visible);
    assert.deepEqual(actual.models, expected.models);
    const ids = [...visible.keys()].filter((index) => visible[index]);
    assert.deepEqual([...actual.sourceIndices], ids);
    assert.deepEqual(
      data.draws.flatMap((draw, index) =>
        actual.drawRanges[index * 2 + 1]
          ? [
              {
                firstInstance: actual.drawRanges[index * 2],
                count: actual.drawRanges[index * 2 + 1],
              },
            ]
          : [],
      ),
      expected.draws,
    );
    for (const [destination, index] of ids.entries())
      assert.deepEqual(
        actual.wind.subarray(destination * 4, destination * 4 + 4),
        data.wind.subarray(index * 4, index * 4 + 4),
      );
    // Damage fallback may change this borrowed visibility without poisoning the source.
    actual.visibility.fill(0);
  }
  assert.deepEqual(data.models, sourceModels);
  assert.deepEqual(data.wind, sourceWind);
  core.dispose();
});

test("moving cameras reuse resident buffers without validating or reading source transforms again", () => {
  const data = source(),
    core = createResidentSourcePreparation();
  let validations = 0,
    reads = 0;
  const every = data.models.every.bind(data.models);
  Object.defineProperty(data.models, "every", {
    value(callback: typeof Number.isFinite) {
      validations++;
      return every(callback);
    },
  });
  const resident = core.prepare(() => {
    reads++;
    return data;
  }, 4);
  const bytes = core.memoryBytes;
  assert.equal(reads, 1);
  assert.equal(validations, 1);
  const first = resident.cull(camera());
  for (let frame = 1; frame < 100; frame++) {
    const result = resident.cull(camera(frame * 0.05));
    assert.equal(result.models.buffer, first.models.buffer);
    assert.equal(result.wind.buffer, first.wind.buffer);
    assert.equal(result.visibility.buffer, first.visibility.buffer);
    assert.equal(result.sourceIndices.buffer, first.sourceIndices.buffer);
    assert.equal(result.drawRanges.buffer, first.drawRanges.buffer);
  }
  assert.equal(core.memoryBytes, bytes);
  assert.equal(reads, 1);
  assert.equal(validations, 1);
  resident.update();
  resident.version = 5;
  assert.equal(reads, 2);
  assert.equal(validations, 2);
  assert.equal(resident.version, 5);
  core.dispose();
});

test("source edits require explicit refresh and rejected refresh preserves the accepted resident data", () => {
  const data = source(),
    core = new PackedFrameCore();
  const resident = bindResidentFrameScene(core, () => data, 0);
  const before = resident.cull(camera()).models.slice();
  data.models[12] += 1000;
  data.bounds.spheres[0] += 1000;
  data.bounds.aabbs[0] += 1000;
  data.bounds.aabbs[3] += 1000;
  assert.deepEqual(resident.cull(camera()).models, before);
  resident.update();
  assert.deepEqual(
    resident.cull(camera()).visibility,
    frustumVisibility(data.bounds, camera()),
  );
  const accepted = resident.cull(camera()).models.slice();
  data.models[3] = 0.5;
  assert.throws(() => resident.update(), /affine/);
  assert.deepEqual(resident.cull(camera()).models, accepted);
  data.models[3] = 0;
  data.draws.pop();
  assert.throws(() => resident.update(), /complete resident/);
  assert.deepEqual(resident.cull(camera()).models, accepted);
  core.dispose();
});

test("scene/core disposal releases owned storage and invalidates borrowed handles", () => {
  const data = source(),
    core = new PackedFrameCore();
  const baseline = core.memoryBytes;
  const handle = core.prepareScene(
    data.bounds,
    data.models,
    data.draws,
    data.wind,
  );
  const resident = bindResidentFrameScene(core, () => data, 0);
  const two = core.memoryBytes;
  assert.ok(two > baseline);
  resident.dispose();
  resident.dispose();
  assert.ok(core.memoryBytes > baseline && core.memoryBytes < two);
  assert.throws(() => resident.cull(camera()), /closed/);
  assert.throws(() => resident.update(), /closed/);
  core.releaseScene(handle);
  core.releaseScene(handle);
  assert.equal(core.memoryBytes, baseline);
  assert.equal(handle.models.length, 0);
  assert.throws(() => core.cull(handle, camera()), /prepared by this/);
  assert.throws(
    () =>
      core.prepareScene({ ...data.bounds, count: -1 }, data.models, data.draws),
    /bounded/,
  );
  assert.equal(core.memoryBytes, baseline);
  const invalid = data.models.slice();
  invalid[3] = 1;
  assert.throws(
    () => core.prepareScene(data.bounds, invalid, data.draws, data.wind),
    /affine/,
  );
  assert.equal(core.memoryBytes, baseline);
  core.dispose();
  core.dispose();
  assert.equal(core.memoryBytes, 0);
  assert.throws(
    () => core.prepareScene(data.bounds, data.models, data.draws),
    /closed/,
  );
});

test("destroyed placements use pooled stable compaction across cameras, damage, and restore", () => {
  const data = source(),
    core = createResidentSourcePreparation();
  let reads = 0;
  const resident = core.prepare(() => {
    reads++;
    return data;
  }, 0);
  assert.equal(typeof resident.cullMasked, "function");
  const originalModels = data.models.slice(),
    originalWind = data.wind.slice(),
    bytes = core.memoryBytes,
    first = resident.cull(camera());
  const buffers = [
    first.visibility.buffer,
    first.models.buffer,
    first.wind.buffer,
    first.sourceIndices.buffer,
    first.drawRanges.buffer,
  ];
  for (let frame = 0; frame < 60; frame++) {
    const matrix = camera(frame * 0.11),
      dead = new Set(
        frame < 20
          ? [0, 3, 17, 32, 66]
          : frame < 40
            ? Array.from({ length: data.bounds.count }, (_, i) => i)
            : [],
      ),
      visible = frustumVisibility(data.bounds, matrix);
    for (const index of dead) visible[index] = 0;
    const expected = compactInstanceDraws(data.models, data.draws, visible),
      actual = resident.cullMasked!(matrix, dead),
      ids = [...visible.keys()].filter((index) => visible[index]);
    assert.deepEqual(actual.visibility, visible);
    assert.deepEqual(actual.models, expected.models);
    assert.deepEqual([...actual.sourceIndices], ids);
    assert.deepEqual(
      [...actual.drawRanges],
      data.draws.flatMap((draw) => {
        let first = 0,
          count = 0;
        for (let index = 0; index < draw.firstInstance; index++)
          first += visible[index];
        for (
          let index = draw.firstInstance;
          index < draw.firstInstance + draw.count;
          index++
        )
          count += visible[index];
        return [first, count];
      }),
    );
    assert.deepEqual(
      [...actual.wind],
      ids.flatMap((index) => [...data.wind.subarray(index * 4, index * 4 + 4)]),
    );
    [
      actual.visibility.buffer,
      actual.models.buffer,
      actual.wind.buffer,
      actual.sourceIndices.buffer,
      actual.drawRanges.buffer,
    ].forEach((buffer, index) => {
      assert.equal(buffer, buffers[index]);
    });
  }
  assert.equal(reads, 1);
  assert.equal(core.memoryBytes, bytes);
  assert.deepEqual(data.models, originalModels);
  assert.deepEqual(data.wind, originalWind);
  const accepted = resident.cullMasked!(
    camera(),
    new Set([0, 1]),
  ).models.slice();
  assert.throws(
    () => resident.cullMasked!(new Float32Array(16), new Set()),
    /finite view/,
  );
  assert.deepEqual(first.models.subarray(0, accepted.length), accepted);
  resident.dispose();
  assert.throws(() => resident.cullMasked!(camera(), new Set()), /closed/);
  core.dispose();
});

test("resident adapters expose masked culling only when their backend implements it", () => {
  const data = source(),
    core = new PackedFrameCore();
  const legacy = {
    prepareScene: core.prepareScene.bind(core),
    updateScene: core.updateScene.bind(core),
    cull: core.cull.bind(core),
    releaseScene: core.releaseScene.bind(core),
  };
  const resident = bindResidentFrameScene(legacy, () => data, 0);
  assert.equal(resident.cullMasked, undefined);
  assert.deepEqual(
    resident.cull(camera()).visibility,
    frustumVisibility(data.bounds, camera()),
  );
  resident.dispose();
  core.dispose();
});

test("renderer uses pooled damage compaction and retains legacy backend fallback", () => {
  for (const legacy of [false, true]) {
    const data = source(),
      core = new PackedFrameCore();
    let maskedCalls = 0,
      sourceReads = 0;
    const backend = legacy
      ? {
          prepareScene: core.prepareScene.bind(core),
          updateScene: core.updateScene.bind(core),
          cull: core.cull.bind(core),
          releaseScene: core.releaseScene.bind(core),
        }
      : {
          prepareScene: core.prepareScene.bind(core),
          updateScene: core.updateScene.bind(core),
          cull: core.cull.bind(core),
          cullMasked(
            handle: Parameters<typeof core.cull>[0],
            matrix: ArrayLike<number>,
            mask: ReadonlySet<number>,
          ) {
            maskedCalls++;
            return core.cullMasked(handle, matrix, mask);
          },
          releaseScene: core.releaseScene.bind(core),
        };
    const resident = bindResidentFrameScene(backend, () => data, 0),
      dead = new Set([0, 17, 32]),
      uploads: { models?: Float32Array; wind?: Float32Array } = {};
    const variant = {
      bounds: data.bounds,
      draws: data.draws,
      wind: { parameters: data.wind },
      get models() {
        sourceReads++;
        return data.models;
      },
    };
    const renderer = Object.assign(Object.create(Renderer.prototype), {
      variants: new Map([["day", variant]]),
      destroyedPlacements: new Map([["day", dead]]),
      cullingEnabled: true,
      geometryVersion: 0,
      buffers: new Set(),
      gpuBytes: 0,
      preparedBufferUploadBytes: 0,
      culledSources: new Map(),
      frameScenes: new Map([["day", resident]]),
      experiment: { framePreparation: {} },
      device: {
        createBuffer({ size, label }: { size: number; label: string }) {
          return {
            size,
            kind: label.startsWith("Visible instances") ? "models" : "wind",
          };
        },
        queue: {
          writeBuffer(
            buffer: { kind: string },
            _offset: number,
            values: Float32Array,
          ) {
            uploads[buffer.kind as "models" | "wind"] = values.slice();
          },
        },
      },
      buffer(values: Float32Array, _usage: number, label: string) {
        const kind = label.startsWith("Visible instances") ? "models" : "wind";
        uploads[kind] = values.slice();
        return { size: Math.max(4, values.byteLength), kind };
      },
      releaseBuffer() {},
    }) as any;
    const savedUsage = Object.getOwnPropertyDescriptor(
      globalThis,
      "GPUBufferUsage",
    );
    Object.defineProperty(globalThis, "GPUBufferUsage", {
      configurable: true,
      value: { VERTEX: 1, COPY_DST: 2 },
    });
    try {
      for (let frame = 0; frame < 12; frame++) {
        const matrix = camera(frame * 0.1),
          visible = frustumVisibility(data.bounds, matrix);
        for (const index of dead) visible[index] = 0;
        const expected = compactInstanceDraws(data.models, data.draws, visible),
          actual = renderer.prepareSourceSubmission(matrix, "camera", "day");
        assert.deepEqual(uploads.models, expected.models);
        assert.deepEqual(actual.draws, expected.draws);
        assert.deepEqual(
          [...uploads.wind!],
          [...visible.keys()]
            .filter((i) => visible[i])
            .flatMap((i) => [...data.wind.subarray(i * 4, i * 4 + 4)]),
        );
      }
      assert.equal(maskedCalls, legacy ? 0 : 12);
      assert.equal(sourceReads, legacy ? 12 : 0);
    } finally {
      if (savedUsage)
        Object.defineProperty(globalThis, "GPUBufferUsage", savedUsage);
      else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
      core.dispose();
    }
  }
});
