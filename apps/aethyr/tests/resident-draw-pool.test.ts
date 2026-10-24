import assert from "node:assert/strict";
import test from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";
import {
  PackedFrameCore,
  bindResidentFrameScene,
} from "../packages/renderer/resident-source.ts";
import {
  buildInstanceBounds,
  compactInstanceDraws,
  frustumVisibility,
} from "../packages/renderer/culling.ts";
import {
  lookAt,
  modelMatrix,
  multiply,
  perspective,
} from "../packages/renderer/math.ts";

const camera = (step = 0) =>
  multiply(
    perspective(0.8, 1.7, 0.1, 1000),
    lookAt([Math.sin(step) * 10, 3, Math.cos(step) * 10], [0, 2, -16]),
  );
function fixture(bundled = true) {
  const instances = Array.from({ length: 96 }, (_, i) => ({
    mesh: 0,
    model: modelMatrix(
      [((i % 12) - 6) * 3, i % 3, -Math.floor(i / 12) * 4],
      i * 0.05,
    ),
  }));
  const models = Float32Array.from(instances.flatMap((i) => i.model));
  const bounds = buildInstanceBounds({
    instances,
    meshes: [{ bounds: { min: [-0.5, -1, -0.2], max: [0.5, 1, 0.2] } }],
  } as any);
  const draws = Object.freeze(
    Array.from({ length: 48 }, (_, i) =>
      Object.freeze({
        sourceDrawIndex: i,
        firstInstance: (i % 12) * 8,
        count: 8,
        indexCount: 12 + (i % 3) * 3,
        firstIndex: i * 18,
        baseVertex: i * 6,
        material: i % 4,
        wind: i % 2 === 0,
        rasterOrientation: i % 3 === 0 ? "negative" : "positive",
      }),
    ),
  );
  const wind = Float32Array.from({ length: 96 * 4 }, (_, i) => i * 0.125);
  const data = { models, bounds, draws, wind };
  const core = new PackedFrameCore(),
    residents = new Map<string, any>();
  let sourceReads = 0;
  const variant: any = {
    bounds,
    draws,
    wind: { parameters: wind },
    get models() {
      sourceReads++;
      return models;
    },
  };
  for (const mode of ["day", "night"])
    residents.set(
      mode,
      bindResidentFrameScene(
        core,
        () => ({ ...data, draws: variant.draws }),
        0,
      ),
    );
  const uploads = new Map<any, any>();
  const device: any = {
    createBuffer({ size, label }: any) {
      return { size, label, destroy() {} };
    },
    queue: {
      writeBuffer(buffer: any, _offset: number, values: any) {
        uploads.set(buffer, values.slice());
      },
    },
  };
  const renderer: any = Object.assign(Object.create(Renderer.prototype), {
    variants: new Map([
      ["day", variant],
      ["night", variant],
    ]),
    destroyedPlacements: new Map(),
    cullingEnabled: true,
    geometryVersion: 0,
    buffers: new Set(),
    gpuBytes: 0,
    preparedBufferUploadBytes: 0,
    culledSources: new Map(),
    frameScenes: residents,
    experiment: { framePreparation: {} },
    device,
    sourceBundlesEnabled: bundled,
    releaseBuffer() {},
  });
  return {
    renderer,
    data,
    variant,
    uploads,
    core,
    get sourceReads() {
      return sourceReads;
    },
  };
}
function expected(
  f: ReturnType<typeof fixture>,
  matrix: Float32Array,
  mode = "day",
) {
  const visible = frustumVisibility(f.data.bounds, matrix);
  for (const i of f.renderer.destroyedPlacements.get(mode) ?? [])
    visible[i] = 0;
  return compactInstanceDraws(f.data.models, f.variant.draws, visible);
}
function verify(f: ReturnType<typeof fixture>, result: any, want: any) {
  assert.deepEqual(result.draws, want.draws);
  if (want.models.length)
    assert.deepEqual(f.uploads.get(result.instances), want.models);
  assert.equal(result.bundleDraws, result.draws);
  if (result.bundles) {
    const words = result.bundles.commandSnapshot();
    for (let i = 0; i < f.variant.draws.length; i++) {
      const draw = want.draws.find((d: any) => d.sourceDrawIndex === i);
      assert.equal(words[i * 5 + 1], draw?.count ?? 0);
      assert.equal(words[i * 5 + 4], draw?.firstInstance ?? 0);
    }
  }
}
test("resident draw records reuse per view without changing culling, order, base descriptors or indirect commands", () => {
  const saved = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { VERTEX: 1, COPY_DST: 2, INDIRECT: 4 },
  });
  try {
    for (const bundled of [false, true]) {
      const f = fixture(bundled),
        original = structuredClone(f.data.draws);
      const a = camera(),
        first = f.renderer.prepareSourceSubmission(a, "camera", "day"),
        array = first.draws,
        pool = first.drawPool;
      verify(f, first, expected(f, a));
      const records = new Map(
        first.draws.map((d: any) => [d.sourceDrawIndex, d]),
      );
      const beforeReflection = structuredClone(first.draws);
      const b = camera(1),
        reflection = f.renderer.prepareSourceSubmission(b, "planar", "day");
      verify(f, reflection, expected(f, b));
      assert.notEqual(reflection.drawPool, pool);
      assert.notEqual(reflection.draws, array);
      assert.deepEqual(
        first.draws,
        beforeReflection,
        "second resident cull must not change the first view's retained draw scalars",
      );
      for (let i = 1; i <= 30; i++) {
        const matrix = camera(i * 0.05),
          next = f.renderer.prepareSourceSubmission(matrix, "camera", "day");
        verify(f, next, expected(f, matrix));
        assert.equal(next.draws, array);
        assert.equal(next.drawPool, pool);
        for (const draw of next.draws)
          if (records.has(draw.sourceDrawIndex))
            assert.equal(draw, records.get(draw.sourceDrawIndex));
        assert.ok(pool.records.filter(Boolean).length <= f.data.draws.length);
      }
      const night = f.renderer.prepareSourceSubmission(
        camera(0.2),
        "camera",
        "night",
      );
      assert.notEqual(night.drawPool, pool);
      const shadow = f.renderer.prepareSourceSubmission(
        camera(0.3),
        "local:0",
        "day",
      );
      assert.equal(shadow.drawPool, undefined);
      const filtered = {
        ...first,
        draws: first.draws.filter((d: any) => d.wind),
      };
      assert.notEqual(
        filtered.bundleDraws,
        filtered.draws,
        "wind-only path must continue to miss the full bundle identity guard",
      );
      assert.deepEqual(f.data.draws, original);
      assert.equal(f.sourceReads, 0);
      f.core.dispose();
    }
  } finally {
    if (saved) Object.defineProperty(globalThis, "GPUBufferUsage", saved);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});

test("pool invalidation, masked culling and legacy masked fallback preserve every accepted draw", () => {
  const saved = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { VERTEX: 1, COPY_DST: 2, INDIRECT: 4 },
  });
  try {
    const f = fixture(),
      r = f.renderer;
    const first = r.prepareSourceSubmission(camera(), "camera", "day"),
      firstPool = first.drawPool;
    r.destroyedPlacements.set("day", new Set([0, 2, 8, 17, 32, 80]));
    const matrix = camera(0.1),
      damaged = r.prepareSourceSubmission(matrix, "camera", "day");
    verify(f, damaged, expected(f, matrix));
    assert.equal(damaged.drawPool, firstPool);
    r.destroyedPlacements.set(
      "day",
      new Set(Array.from({ length: 96 }, (_, i) => i)),
    );
    const empty = r.prepareSourceSubmission(camera(0.2), "camera", "day");
    verify(f, empty, expected(f, camera(0.2)));
    assert.equal(empty.draws.length, 0);
    r.destroyedPlacements.clear();
    r.geometryVersion++;
    const restored = r.prepareSourceSubmission(camera(0.3), "camera", "day");
    verify(f, restored, expected(f, camera(0.3)));
    assert.notEqual(restored.drawPool, firstPool);
    const nextPool = restored.drawPool;
    f.variant.draws = Object.freeze(
      f.data.draws.map((d: any) =>
        Object.freeze({ ...d, material: d.material + 4 }),
      ),
    );
    const replaced = r.prepareSourceSubmission(camera(0.4), "camera", "day");
    verify(f, replaced, expected(f, camera(0.4)));
    assert.notEqual(replaced.drawPool, nextPool);
    const modern = r.frameScenes.get("day");
    r.frameScenes.set("day", {
      version: modern.version,
      cull: modern.cull,
      update: modern.update,
    });
    r.destroyedPlacements.set("day", new Set([0, 1, 2]));
    const legacy = r.prepareSourceSubmission(camera(0.5), "camera", "day");
    verify(f, legacy, expected(f, camera(0.5)));
    assert.equal(legacy.drawPool, undefined);
    assert.equal(f.sourceReads, 1);
    const previousPool = replaced.drawPool;
    r.culledSources.clear();
    r.destroyedPlacements.clear();
    r.frameScenes.set("day", modern);
    const fresh = r.prepareSourceSubmission(camera(0.6), "camera", "day");
    verify(f, fresh, expected(f, camera(0.6)));
    assert.notEqual(fresh.drawPool, previousPool);
    f.core.dispose();
  } finally {
    if (saved) Object.defineProperty(globalThis, "GPUBufferUsage", saved);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});

test("failed uploads cannot make a reused pool hit the old camera signature", () => {
  const saved = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { VERTEX: 1, COPY_DST: 2, INDIRECT: 4 },
  });
  try {
    const f = fixture(),
      a = camera(),
      b = camera(Math.PI);
    const first = f.renderer.prepareSourceSubmission(a, "camera", "day");
    const oldSignature = first.signature;
    assert.notDeepEqual(expected(f, a).draws, expected(f, b).draws);
    const write = f.renderer.device.queue.writeBuffer;
    f.renderer.device.queue.writeBuffer = () => {
      throw Error("Mock upload failure");
    };
    assert.throws(
      () => f.renderer.prepareSourceSubmission(b, "camera", "day"),
      /Mock upload failure/,
    );
    assert.notEqual(first.signature, oldSignature);
    f.renderer.device.queue.writeBuffer = write;
    const recovered = f.renderer.prepareSourceSubmission(a, "camera", "day");
    verify(f, recovered, expected(f, a));
    assert.equal(recovered.signature, oldSignature);
    f.core.dispose();
  } finally {
    if (saved) Object.defineProperty(globalThis, "GPUBufferUsage", saved);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});

test("buffer growth failures retain usable cached handles independently of the draw pool", () => {
  const saved = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { VERTEX: 1, COPY_DST: 2, INDIRECT: 4 },
  });
  try {
    for (const failure of [
      "models-allocation",
      "wind-allocation",
      "models-upload",
      "wind-upload",
    ]) {
      const f = fixture(false),
        r = f.renderer;
      const views = Array.from({ length: 24 }, (_, i) =>
        camera((i * Math.PI) / 12),
      )
        .map((matrix) => ({
          matrix,
          bytes: expected(f, matrix).models.byteLength,
        }))
        .sort((a, b) => a.bytes - b.bytes);
      const small = views[0].matrix,
        large = views.at(-1)!.matrix;
      assert.ok(views[0].bytes > 0 && views.at(-1)!.bytes > views[0].bytes);
      const first = r.prepareSourceSubmission(small, "local:0", "day");
      assert.equal(first.drawPool, undefined);
      const old = { models: first.instances, wind: first.wind };
      const create = r.device.createBuffer,
        write = r.device.queue.writeBuffer;
      const kind = (label: string) =>
        label.startsWith("Visible instances") ? "models" : "wind";
      let failed = false;
      r.releaseBuffer = (buffer: any) => {
        if (buffer) buffer.destroyed = true;
      };
      r.device.createBuffer = (options: any) => {
        const field = kind(options.label);
        if (
          !failed &&
          failure === field + "-allocation" &&
          options.size > old[field].size
        ) {
          failed = true;
          throw Error("Mock growth allocation failure");
        }
        return create(options);
      };
      r.device.queue.writeBuffer = (
        buffer: any,
        offset: number,
        values: any,
      ) => {
        assert.notEqual(
          buffer.destroyed,
          true,
          "Retry must never upload into the released buffer",
        );
        const field = kind(buffer.label);
        if (
          !failed &&
          failure === field + "-upload" &&
          buffer.size > old[field].size
        ) {
          failed = true;
          throw Error("Mock growth upload failure");
        }
        write(buffer, offset, values);
      };
      assert.throws(
        () => r.prepareSourceSubmission(large, "local:0", "day"),
        /Mock growth/,
      );
      assert.equal(failed, true);
      assert.equal(first.signature, "");
      if (failure.endsWith("allocation")) {
        const field = failure.startsWith("models") ? "models" : "wind";
        assert.notEqual(
          old[field].destroyed,
          true,
          "Allocation must succeed before releasing its old buffer",
        );
      }
      for (const [field, buffer] of Object.entries({
        models: first.instances,
        wind: first.wind,
      }))
        if (old[field as "models" | "wind"].destroyed)
          assert.notEqual(
            buffer,
            old[field as "models" | "wind"],
            "The cache must publish the replacement handle before a later failure",
          );
      const recovered = r.prepareSourceSubmission(small, "local:0", "day");
      verify(f, recovered, expected(f, small));
      assert.notEqual(recovered.instances.destroyed, true);
      assert.notEqual(recovered.wind.destroyed, true);
      f.core.dispose();
    }
  } finally {
    if (saved) Object.defineProperty(globalThis, "GPUBufferUsage", saved);
    else Reflect.deleteProperty(globalThis, "GPUBufferUsage");
  }
});
