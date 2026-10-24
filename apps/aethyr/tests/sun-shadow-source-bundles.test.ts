import assert from "node:assert/strict";
import test from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";
import {
  SourceRenderBundles,
  SOURCE_BUNDLE_PAGE_DRAWS,
  type BundledSourceDraw,
} from "../packages/renderer/source-render-bundles.ts";
import {
  executeSunShadowSourceBundle,
  isSunShadowBundleView,
  sunShadowPartitionIncludes,
  type SunShadowBundleBindings,
  type SunShadowPartition,
} from "../packages/renderer/sun-shadow-source-bundles.ts";

Object.assign(globalThis, { GPUBufferUsage: { INDIRECT: 1, COPY_DST: 2 } });
const named = (id: string) => ({ id });
const id = (value: any) => value?.id ?? value?.label;
const draws = (count = 15): BundledSourceDraw[] =>
  Array.from({ length: count }, (_, index) => ({
    sourceDrawIndex: index,
    indexCount: ((index % 3) + 1) * 3,
    firstIndex: index * 12,
    baseVertex: (index % 5) - 2,
    firstInstance: index * 3,
    count: 3,
    material: index % 5,
    wind: index % 2 === 0,
    rasterOrientation: (["positive", "negative", "degenerate"] as const)[
      index % 3
    ],
  }));
function fixture(count = 15) {
  let failUpload = false,
    failFinish = false,
    accounted = 0;
  const encoders: any[] = [];
  const device = {
    queue: {
      writeBuffer(buffer: any, offset: number, words: Uint32Array) {
        if (failUpload) {
          failUpload = false;
          throw Error("injected upload failure");
        }
        assert.equal(offset, 0);
        buffer.words = words.slice();
      },
    },
    createBuffer(descriptor: any) {
      return { ...descriptor, destroy() {} };
    },
    createRenderBundleEncoder(descriptor: any) {
      const calls: any[][] = [],
        record = { descriptor, calls };
      encoders.push(record);
      return {
        ...Object.fromEntries(
          [
            "setPipeline",
            "setBindGroup",
            "setVertexBuffer",
            "setIndexBuffer",
            "drawIndexedIndirect",
          ].map((method) => [
            method,
            (...args: any[]) => calls.push([method, ...args]),
          ]),
        ),
        finish() {
          if (failFinish) {
            failFinish = false;
            throw Error("injected finish failure");
          }
          return record;
        },
      };
    },
  };
  const base = draws(count),
    table = new SourceRenderBundles(
      device as unknown as GPUDevice,
      base,
      (delta) => {
        accounted += delta;
      },
    );
  table.update(base);
  const materials = Array.from({ length: 5 }, (_, index) =>
    named("material-" + index),
  );
  const bindings = {
    enabled: true,
    stockShader: true,
    frameGroup: named("sun-view"),
    pipeline: named("original-shadow-pipeline"),
    materials,
    vertices: named("original-vertices"),
    colors: named("original-colors"),
    uv1: named("original-uv1"),
    zeroWind: named("zero-wind"),
    indices: named("original-indices"),
    include: (draw: BundledSourceDraw) => draw.material < 2,
  } as unknown as SunShadowBundleBindings;
  const source = {
    instances: named("source-instances") as unknown as GPUBuffer,
    wind: named("source-wind") as unknown as GPUBuffer,
    draws: base,
    bundleDraws: base,
    bundles: table,
    shadowPartition: "all" as SunShadowPartition,
  };
  return {
    device,
    table,
    base,
    bindings,
    source,
    encoders,
    account: () => accounted,
    failUpload: () => {
      failUpload = true;
    },
    failFinish: () => {
      failFinish = true;
    },
  };
}
function passRecorder() {
  let pipeline: any,
    group = new Map<number, any>(),
    vertices = new Map<number, any>(),
    index: any;
  const work: any[] = [],
    calls: any[][] = [];
  const pass: any = {
    work,
    calls,
    setPipeline(value: any) {
      pipeline = value;
      calls.push(["pipeline", id(value)]);
    },
    setBindGroup(slot: number, value: any) {
      group.set(slot, value);
      calls.push(["group", slot, id(value)]);
    },
    setVertexBuffer(slot: number, value: any) {
      vertices.set(slot, value);
      calls.push(["vertex", slot, id(value)]);
    },
    setIndexBuffer(value: any, format: string) {
      index = value;
      assert.equal(format, "uint32");
      calls.push(["index", id(value)]);
    },
    drawIndexed(...parameters: number[]) {
      if (!parameters[1]) return;
      assert(pipeline && group.has(0) && group.has(1) && index);
      for (let slot = 0; slot < 5; slot++) assert(vertices.has(slot));
      work.push({
        parameters,
        pipeline: id(pipeline),
        group: [id(group.get(0)), id(group.get(1))],
        vertices: Array.from({ length: 5 }, (_, slot) =>
          id(vertices.get(slot)),
        ),
        indices: id(index),
      });
    },
    executeBundles(bundles: any[]) {
      calls.push(["executeBundles", bundles.length]);
      for (const bundle of bundles)
        for (const [method, ...args] of bundle.calls) {
          if (method === "drawIndexedIndirect") {
            const [buffer, byteOffset] = args,
              offset = byteOffset / 4,
              words = buffer.words;
            pass.drawIndexed(
              words[offset],
              words[offset + 1],
              words[offset + 2],
              words[offset + 3] | 0,
              words[offset + 4],
            );
          } else pass[method](...args);
        }
      pipeline = undefined;
      group = new Map();
      vertices = new Map();
      index = undefined;
    },
  };
  return pass;
}
function direct(
  f: ReturnType<typeof fixture>,
  visible: BundledSourceDraw[],
  partition: SunShadowPartition,
) {
  return visible
    .filter(
      (draw) =>
        sunShadowPartitionIncludes(draw, partition) &&
        f.bindings.include(draw) &&
        draw.count,
    )
    .map((draw) => ({
      parameters: [
        draw.indexCount,
        draw.count,
        draw.firstIndex,
        draw.baseVertex,
        draw.firstInstance,
      ],
      pipeline: "original-shadow-pipeline",
      group: ["sun-view", "material-" + draw.material],
      vertices: [
        "original-vertices",
        "source-instances",
        "original-colors",
        "original-uv1",
        draw.wind ? "source-wind" : "zero-wind",
      ],
      indices: "original-indices",
    }));
}
test("only the three stock sun views use shadow source bundles", () => {
  for (const key of ["nearSun", "farSun", "distantSun"]) {
    assert(isSunShadowBundleView(key, true));
    assert(!isSunShadowBundleView(key, false));
  }
  for (const key of [
    "camera",
    "planar",
    "local:0",
    "local:39",
    "nearSun:extra",
    "",
  ])
    assert(!isSunShadowBundleView(key, true));
});
test("source partitions preserve direct order, geometry, wind and material bindings", () => {
  const f = fixture();
  for (const partition of ["all", "static", "moving"] as const) {
    f.source.shadowPartition = partition;
    const pass = passRecorder();
    assert.equal(
      executeSunShadowSourceBundle(pass, f.source, f.bindings),
      true,
    );
    assert.deepEqual(pass.work, direct(f, f.base, partition));
  }
  assert(
    f.encoders.every(
      ({ descriptor }) =>
        descriptor.depthStencilFormat === "depth32float" &&
        descriptor.sampleCount === 1 &&
        descriptor.colorFormats.length === 0,
    ),
  );
  assert.equal(f.table.snapshot().bundles, 3);
});
test("visibility edits and all-dead/restore reuse recordings without changing indexed commands", () => {
  const f = fixture();
  for (const visible of [
    f.base,
    [f.base[1], { ...f.base[5], count: 1, firstInstance: 3 }],
    [],
    f.base,
  ]) {
    f.table.update(visible);
    f.source.draws = visible;
    f.source.bundleDraws = visible;
    for (const partition of ["all", "static", "moving"] as const) {
      f.source.shadowPartition = partition;
      const pass = passRecorder();
      executeSunShadowSourceBundle(pass, f.source, f.bindings);
      assert.deepEqual(pass.work, direct(f, visible, partition));
    }
  }
  assert.equal(f.table.snapshot().builds, 3);
  assert.equal(f.table.snapshot().reuses, 9);
});
test("bundle execution restores cleared bindings before direct geometry tails", () => {
  const f = fixture(),
    pass = passRecorder();
  executeSunShadowSourceBundle(pass, f.source, f.bindings);
  pass.setVertexBuffer(1, named("authored-models"));
  pass.setVertexBuffer(4, f.bindings.zeroWind);
  pass.setBindGroup(1, named("authored-material"));
  pass.drawIndexed(6, 2, 3, -1, 0);
  assert.deepEqual(pass.work.at(-1), {
    parameters: [6, 2, 3, -1, 0],
    pipeline: "original-shadow-pipeline",
    group: ["sun-view", "authored-material"],
    vertices: [
      "original-vertices",
      "authored-models",
      "original-colors",
      "original-uv1",
      "zero-wind",
    ],
    indices: "original-indices",
  });
});
test("custom shaders, unsupported adapters, stale draw references and non-shadow paths retain direct fallback", () => {
  const f = fixture();
  for (const options of [
    { enabled: false },
    { stockShader: false },
    { frameGroup: undefined },
  ]) {
    const pass = passRecorder();
    assert.equal(
      executeSunShadowSourceBundle(pass, f.source, {
        ...f.bindings,
        ...options,
      }),
      undefined,
    );
    assert.deepEqual(pass.calls, []);
  }
  for (const source of [
    { ...f.source, shadowPartition: undefined },
    { ...f.source, bundles: undefined },
    { ...f.source, bundleDraws: f.source.draws.slice() },
  ]) {
    const pass = passRecorder();
    assert.equal(
      executeSunShadowSourceBundle(pass, source, f.bindings),
      undefined,
    );
    assert.deepEqual(pass.calls, []);
  }
});
test("pipeline and resource replacements rebuild bounded shadow slots", () => {
  const f = fixture();
  executeSunShadowSourceBundle(passRecorder(), f.source, f.bindings);
  for (const field of [
    "pipeline",
    "frameGroup",
    "vertices",
    "indices",
    "zeroWind",
  ] as const) {
    const builds = f.table.snapshot().builds;
    (f.bindings as any)[field] = named("replacement-" + field);
    executeSunShadowSourceBundle(passRecorder(), f.source, f.bindings);
    assert.equal(f.table.snapshot().builds, builds + 1);
    assert.equal(f.table.snapshot().bundles, 1);
  }
});
test("failed uploads and failed recording cannot execute stale source commands", () => {
  const f = fixture();
  executeSunShadowSourceBundle(passRecorder(), f.source, f.bindings);
  const visible = [{ ...f.base[1], count: 1, firstInstance: 0 }];
  f.failUpload();
  assert.throws(() => f.table.update(visible), /upload failure/);
  assert.throws(
    () => executeSunShadowSourceBundle(passRecorder(), f.source, f.bindings),
    /Upload the complete/,
  );
  f.table.update(visible);
  f.source.draws = visible;
  f.source.bundleDraws = visible;
  f.bindings.pipeline = named(
    "replacement-pipeline",
  ) as unknown as GPURenderPipeline;
  f.failFinish();
  assert.throws(
    () => executeSunShadowSourceBundle(passRecorder(), f.source, f.bindings),
    /finish failure/,
  );
  const pass = passRecorder();
  assert.equal(executeSunShadowSourceBundle(pass, f.source, f.bindings), true);
  assert.equal(pass.work.length, 1);
  assert.deepEqual(pass.work[0].parameters, [
    visible[0].indexCount,
    1,
    visible[0].firstIndex,
    visible[0].baseVertex,
    0,
  ]);
  f.table.dispose();
  f.table.dispose();
  assert.equal(f.account(), 0);
  assert.throws(
    () => executeSunShadowSourceBundle(passRecorder(), f.source, f.bindings),
    /closed/,
  );
});
test("an empty partition is handled without submitting a synthetic bundle", () => {
  const f = fixture();
  f.source.shadowPartition = "moving";
  const pass = passRecorder();
  assert.equal(
    executeSunShadowSourceBundle(pass, f.source, {
      ...f.bindings,
      include: () => false,
    }),
    false,
  );
  assert.equal(
    pass.calls.filter((call: any) => call[0] === "executeBundles").length,
    0,
  );
  assert.deepEqual(pass.work, []);
});

function rendererFixture(f: ReturnType<typeof fixture>, enabled: boolean) {
  const renderer: any = Object.assign(Object.create(Renderer.prototype), {
    device: f.device,
    sourceBundlesEnabled: enabled,
    activeShaderSource: "stock",
    shaders: { MATERIAL: "stock" },
    shadowPipeline: f.bindings.pipeline,
    pipeline: named("main-pipeline"),
    materials: f.bindings.materials,
    vertices: f.bindings.vertices,
    vertexColors: f.bindings.colors,
    vertexUV1: f.bindings.uv1,
    indices: f.bindings.indices,
    zeroWind: f.bindings.zeroWind,
    instances: f.source.instances,
    renderMaterialInfo: [
      { alphaMode: "OPAQUE" },
      { alphaMode: "MASK" },
      { alphaMode: "BLEND" },
      { glass: true },
      { flame: true },
    ],
    authoredInstances: named("authored-models"),
    authoredDraws: [
      {
        material: 0,
        firstIndex: 3,
        indexCount: 6,
        baseVertex: -1,
        firstInstance: 0,
        count: 2,
      },
    ],
    roomCount: 1,
    roomVertices: named("room-vertices"),
    roomIndices: named("room-indices"),
    roomInstances: named("room-models"),
    roomMaterial: named("room-material"),
    roomColors: named("room-colors"),
    roomUV1: named("room-uv1"),
    sourceBundleExecutions: 0,
    stockPipelines: new Map(),
    submittedGeometry: { instances: 0, triangles: 0, drawCalls: 0 },
    gpuBytes: 0,
    buffers: new Set(),
    variants: new Map(),
    destroyedPlacements: new Map(),
    culledSources: new Map(),
    frameScenes: new Map(),
    geometryVersion: 0,
    cullingEnabled: true,
    experiment: {},
    preparedBufferUploadBytes: 0,
  });
  const debris = (
    label: string,
    pass: any,
    materials: any[],
    zeroWind: any,
  ) => {
    pass.setVertexBuffer(0, named(label + "-vertices"));
    pass.setVertexBuffer(1, named(label + "-models"));
    pass.setVertexBuffer(2, named(label + "-colors"));
    pass.setVertexBuffer(3, named(label + "-uv1"));
    pass.setVertexBuffer(4, zeroWind);
    pass.setIndexBuffer(named(label + "-indices"), "uint32");
    pass.setBindGroup(1, materials[0]);
    pass.drawIndexed(6, 2, 0, -1, 0);
  };
  renderer.sourceDebris = {
    drawStanding: (...args: any[]) =>
      debris("standing", args[0], args[1], args[2]),
    drawMoving: (...args: any[]) => debris("moving", args[0], args[1], args[2]),
  };
  return renderer;
}
test("actual Renderer preserves direct authored, room and debris tails after bundled source shadows", () => {
  const f = fixture(),
    baseline = rendererFixture(f, false),
    candidate = rendererFixture(f, true);
  for (const partition of ["all", "static", "moving"] as const) {
    const a = passRecorder(),
      b = passRecorder();
    const source = {
      ...f.source,
      shadowPartition: partition,
      dynamicOnly: partition === "moving",
      debris: partition === "moving" ? "moving" : "standing",
    };
    for (const pass of [a, b]) {
      pass.setPipeline(f.bindings.pipeline);
      pass.setBindGroup(0, f.bindings.frameGroup);
    }
    baseline.draw(
      a,
      false,
      false,
      source,
      false,
      false,
      undefined,
      f.bindings.frameGroup,
    );
    candidate.draw(
      b,
      false,
      false,
      source,
      false,
      false,
      undefined,
      f.bindings.frameGroup,
    );
    assert.deepEqual(b.work, a.work);
    assert.equal(
      b.calls.filter((call: any) => call[0] === "executeBundles").length,
      1,
    );
    assert.equal(
      b.work.filter((draw: any) => draw.vertices[1] === "authored-models")
        .length,
      partition === "moving" ? 0 : 1,
    );
    assert.equal(
      b.work.at(-1).vertices[0],
      partition === "moving" ? "moving-vertices" : "standing-vertices",
    );
  }
  assert.equal(candidate.sourceBundleExecutions, 3);
});
test("actual Renderer prepares bundles for only stock sun keys", () => {
  const identity = new Float32Array([
    1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
  ]);
  for (const [key, stock, expected] of [
    ["nearSun", true, true],
    ["farSun", true, true],
    ["distantSun", true, true],
    ["local:0", true, false],
    ["nearSun", false, false],
  ] as const) {
    const f = fixture(),
      renderer = rendererFixture(f, true),
      count = 45;
    const models = new Float32Array(count * 16),
      spheres = new Float64Array(count * 4),
      aabbs = new Float64Array(count * 6);
    for (let index = 0; index < count; index++) {
      models.set(identity, index * 16);
      models[index * 16 + 14] = 0.5;
      spheres.set([0, 0, 0.5, 0.1], index * 4);
      aabbs.set([-0.1, -0.1, 0.4, 0.1, 0.1, 0.6], index * 6);
    }
    renderer.variants.set("day", {
      models,
      draws: f.base,
      bounds: { count, spheres, aabbs },
      wind: { parameters: new Float32Array(count * 4) },
      windBuffer: f.source.wind,
    });
    if (!stock) renderer.activeShaderSource = "custom";
    const source = renderer.prepareSourceSubmission(identity, key, "day");
    assert.equal(Boolean(source.bundles), expected);
    if (source.bundles) assert.equal(source.instances.size, count * 64);
  }
});

test("actual Renderer executes ordered source pages before unchanged main and shadow direct tails", () => {
  const f = fixture(SOURCE_BUNDLE_PAGE_DRAWS * 2 + 7);
  for (const specialized of [undefined, "main"] as const) {
    const baseline = rendererFixture(f, false),
      candidate = rendererFixture(f, true),
      a = passRecorder(),
      b = passRecorder(),
      source = {
        ...f.source,
        shadowPartition: specialized ? undefined : ("all" as const),
        debris: "standing",
      };
    for (const renderer of [baseline, candidate])
      renderer.materialPipeline = () => f.bindings.pipeline;
    for (const pass of [a, b]) {
      pass.setPipeline(f.bindings.pipeline);
      pass.setBindGroup(0, f.bindings.frameGroup);
    }
    baseline.draw(
      a,
      false,
      false,
      { ...source, bundles: undefined },
      false,
      specialized !== undefined,
      specialized,
      f.bindings.frameGroup,
    );
    candidate.draw(
      b,
      false,
      false,
      source,
      false,
      specialized !== undefined,
      specialized,
      f.bindings.frameGroup,
    );
    assert.deepEqual(b.work, a.work);
    assert.deepEqual(
      b.work.slice(0, -3).map((draw: any) => draw.parameters[2]),
      f.base
        .filter((draw) => f.bindings.include(draw))
        .map((draw) => draw.firstIndex),
    );
    assert.deepEqual(
      b.calls.filter((call: any) => call[0] === "executeBundles"),
      [["executeBundles", 3]],
    );
    assert.equal(b.work.at(-1).vertices[0], "standing-vertices");
    assert.deepEqual(candidate.submittedGeometry, baseline.submittedGeometry);
  }
});
