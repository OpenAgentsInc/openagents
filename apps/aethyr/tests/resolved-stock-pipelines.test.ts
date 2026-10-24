import assert from "node:assert/strict";
import test from "node:test";
import { Renderer as Candidate } from "../packages/renderer/renderer.ts";
import type { RasterOrientation } from "../packages/renderer/raster-orientation.ts";
import {
  deriveStockDepthPolicy,
  stockRasterPipelineKey,
} from "../packages/renderer/stock-raster-policy.ts";

// Independent resolver preserved from the pre-cache Renderer. Real Renderer
// methods still own command emission, preparation, validation and activation.
function referenceMaterialPipeline(
  this: any,
  group: GPUBindGroup,
  blend: boolean,
  additive: boolean,
  kind: "main" | "planar" | "depth",
  orientation: RasterOrientation = "degenerate",
) {
  if (kind === "depth") {
    if (this.activeShaderSource !== this.shaders.MATERIAL)
      return this.depthPipeline;
    const policy = deriveStockDepthPolicy(
      this.materialFeatures.get(group),
      orientation,
    );
    return policy.fragmentRequired
      ? this.depthPipeline
      : (this.opaqueDepthPipelines.get(
          `${policy.cullMode}:${policy.frontFace}`,
        ) ?? this.depthPipeline);
  }
  const front = kind === "planar" ? "cw" : "ccw";
  const features = this.materialFeatures.get(group);
  if (
    this.activeShaderSource === this.shaders.MATERIAL &&
    features !== undefined
  ) {
    const specialized = this.stockPipelines.get(
      stockRasterPipelineKey(features, blend, additive, front, orientation),
    );
    if (specialized) return specialized;
  }
  return kind === "main"
    ? additive
      ? this.additivePipeline
      : blend
        ? this.blendPipeline
        : this.pipeline
    : additive
      ? this.planarAdditivePipeline
      : blend
        ? this.planarBlendPipeline
        : this.planarPipeline;
}
const Baseline = {
  prototype: Object.assign(Object.create(Candidate.prototype), {
    materialPipeline: referenceMaterialPipeline,
  }),
};

const orientations = ["positive", "negative", "degenerate"] as const;
const kinds = ["main", "planar", "depth"] as const;
const flags = [
  0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 0x10000, 0x20000, 0x30000, 0x40000,
  0x40001, 0x501ff, 0x601ff, 0x701ff,
];
const layout = {};
const named = (name: string) => ({ name, getBindGroupLayout: () => layout });
function pair() {
  const groups = flags.map((_, i) => ({ name: `material-${i}` }));
  const pipelines = new Map<string, any>();
  const pipeline = (key: string) => {
    if (!pipelines.has(key)) pipelines.set(key, named(key));
    return pipelines.get(key);
  };
  const featureMap = new WeakMap(groups.map((group, i) => [group, flags[i]]));
  const stock = new Map<string, any>();
  for (const f of flags)
    for (const blend of [false, true])
      for (const additive of [false, true])
        for (const front of ["ccw", "cw"] as const)
          for (const orientation of orientations) {
            const key = stockRasterPipelineKey(
              f,
              blend,
              additive,
              front,
              orientation,
            );
            stock.set(key, pipeline(key));
          }
  const depth = new Map<string, any>();
  for (const f of flags)
    for (const orientation of orientations) {
      const policy = deriveStockDepthPolicy(f, orientation);
      const key = `${policy.cullMode}:${policy.frontFace}`;
      depth.set(key, pipeline(`depth:${key}`));
    }
  const state = {
    shaders: { MATERIAL: "owned-stock" },
    activeShaderSource: "owned-stock",
    materialFeatures: featureMap,
    stockModule: { code: "owned-stock" },
    pipeline: pipeline("generic-main"),
    blendPipeline: pipeline("generic-blend"),
    additivePipeline: pipeline("generic-additive"),
    planarPipeline: pipeline("generic-planar"),
    planarBlendPipeline: pipeline("generic-planar-blend"),
    planarAdditivePipeline: pipeline("generic-planar-additive"),
    shadowPipeline: pipeline("generic-shadow"),
    depthPipeline: pipeline("generic-depth"),
    materialLayout: layout,
    layout,
    disposed: false,
    desiredShader: 0,
    shaderPending: 0,
    geometryVersion: 0,
    shadowDirty: false,
    stockChoiceRevision: 0,
    vertices: named("vertices"),
    instances: named("instances"),
    vertexColors: named("colors"),
    vertexUV1: named("uv1"),
    indices: named("indices"),
    zeroWind: named("wind"),
    materials: groups,
    renderMaterialInfo: flags.map((_, i) =>
      i % 5 === 1
        ? { alphaMode: "BLEND" }
        : i % 5 === 2
          ? { flame: {} }
          : i % 5 === 3
            ? { glass: {} }
            : {},
    ),
    authoredInstances: named("authored-instances"),
    roomCount: 0,
  };
  const device = {
    pushErrorScope() {},
    async popErrorScope() {
      return null;
    },
    createPipelineLayout() {
      return layout;
    },
    createShaderModule({ code }: any) {
      return {
        code,
        async getCompilationInfo() {
          return {
            messages:
              code === "INVALID"
                ? [{ type: "error", lineNum: 9, message: "bad WGSL" }]
                : [],
          };
        },
      };
    },
    async createRenderPipelineAsync(descriptor: any) {
      if (descriptor.vertex.module.code === "PIPELINE-FAIL")
        throw Error("GPU pipeline failure");
      return pipeline(
        JSON.stringify({
          source: descriptor.vertex.module.code,
          entry: descriptor.vertex.entryPoint,
          fragment: descriptor.fragment?.entryPoint,
          constants: descriptor.fragment?.constants,
          primitive: descriptor.primitive,
        }),
      );
    },
  };
  const make = (klass: any) =>
    Object.assign(Object.create(klass.prototype), state, {
      device,
      shaderQueue: Promise.resolve(),
      stockPipelines: new Map(stock),
      opaqueDepthPipelines: new Map(depth),
      stockPipelineTasks: new Map(),
      stockChoices: new WeakMap(),
      windShadowPlanner: { reset() {} },
      submittedGeometry: { instances: 0, triangles: 0, drawCalls: 0 },
    });
  return {
    baseline: make(Baseline),
    candidate: make(Candidate),
    groups,
    pipeline,
  };
}
function recordPass() {
  const calls: unknown[] = [];
  return {
    calls,
    setPipeline: (...args: unknown[]) => calls.push(["pipeline", ...args]),
    setVertexBuffer: (...args: unknown[]) => calls.push(["vertex", ...args]),
    setIndexBuffer: (...args: unknown[]) => calls.push(["index", ...args]),
    setBindGroup: (...args: unknown[]) => calls.push(["group", ...args]),
    drawIndexed: (...args: unknown[]) => calls.push(["draw", ...args]),
  };
}
function drawRecords() {
  return Array.from({ length: 90 }, (_, i) => ({
    material: (i * 7 + Math.floor(i / 3)) % flags.length,
    rasterOrientation: orientations[Math.floor(i / 2) % 3],
    wind: i % 3 === 0,
    indexCount: ((i % 7) + 1) * 3,
    count: (i % 4) + 1,
    firstIndex: i * 30,
    baseVertex: (i % 9) - 4,
    firstInstance: i * 4,
  }));
}
function assertDraws(value: ReturnType<typeof pair>) {
  for (const kind of kinds)
    for (const blend of [false, true])
      for (const additive of [false, true]) {
        const a = recordPass(),
          b = recordPass();
        const draws = drawRecords();
        for (const renderer of [value.baseline, value.candidate]) {
          renderer.draws = draws;
          renderer.authoredDraws = draws.slice().reverse();
        }
        value.baseline.draw(a, blend, additive, undefined, false, true, kind);
        value.candidate.draw(b, blend, additive, undefined, false, true, kind);
        assert.deepEqual(b.calls, a.calls);
        assert.deepEqual(
          value.candidate.submittedGeometry,
          value.baseline.submittedGeometry,
        );
      }
}

test("all stock material/orientation/pass/blend choices and exact direct command streams agree cold and warm", () => {
  const p = pair();
  for (let repeat = 0; repeat < 3; repeat++) {
    for (const group of p.groups)
      for (const kind of kinds)
        for (const blend of [false, true])
          for (const additive of [false, true])
            for (const orientation of orientations)
              assert.equal(
                p.candidate.materialPipeline(
                  group,
                  blend,
                  additive,
                  kind,
                  orientation,
                ),
                p.baseline.materialPipeline(
                  group,
                  blend,
                  additive,
                  kind,
                  orientation,
                ),
              );
    assertDraws(p);
  }
});

test("successful custom activation and exact stock restore invalidate choices; failed and stale shaders preserve them", async () => {
  const p = pair();
  assertDraws(p);
  const group = p.groups[0],
    original = p.candidate.stockChoices.get(group);
  const oldRevision = p.candidate.stockChoiceRevision;
  for (const renderer of [p.baseline, p.candidate]) {
    await assert.rejects(renderer.prepareShader("INVALID"), /Line 9: bad WGSL/);
    await assert.rejects(
      renderer.prepareShader("PIPELINE-FAIL"),
      /GPU pipeline failure/,
    );
  }
  assert.equal(p.candidate.stockChoiceRevision, oldRevision);
  assert.equal(p.candidate.stockChoices.get(group), original);
  assertDraws(p);
  const stale = await p.candidate.prepareShader("ignored", () => false);
  assert.equal(stale, undefined);
  assert.equal(p.candidate.stockChoiceRevision, oldRevision);
  for (const renderer of [p.baseline, p.candidate]) {
    const prepared = await renderer.prepareShader("CUSTOM");
    prepared.activate();
  }
  assert.equal(p.candidate.stockChoiceRevision, oldRevision + 1);
  assertDraws(p);
  for (const renderer of [p.baseline, p.candidate])
    (await renderer.prepareShader("owned-stock")).activate();
  assert.equal(p.candidate.stockChoiceRevision, oldRevision + 2);
  assertDraws(p);
});

test("prepared but unactivated, superseded and no-longer-current shader candidates retain the live cache", async () => {
  const p = pair();
  assertDraws(p);
  const revision = p.candidate.stockChoiceRevision;
  const first = await p.candidate.prepareShader("STAGED");
  assert.equal(first.ready(), true);
  assert.equal(p.candidate.stockChoiceRevision, revision);
  const second = await p.candidate.prepareShader("SUPERSEDING");
  assert.equal(first.ready(), false);
  first.activate();
  assert.equal(p.candidate.stockChoiceRevision, revision);
  let current = true;
  const third = await p.candidate.prepareShader("CANCELLED", () => current);
  assert.equal(second.ready(), false);
  current = false;
  assert.equal(third.ready(), false);
  third.activate();
  assert.equal(p.candidate.stockChoiceRevision, revision);
  assertDraws(p);
  await assert.rejects(
    p.candidate.prepareShader("x".repeat(131073)),
    /Keep shader edits below 128 KiB/,
  );
  assert.equal(p.candidate.stockChoiceRevision, revision);
});

test("new stock pipeline insertions invalidate a previously cached generic fallback; failed insertion does not", async () => {
  const p = pair(),
    group = p.groups[0];
  for (const renderer of [p.baseline, p.candidate])
    renderer.stockPipelines.clear();
  assert.equal(
    p.candidate.materialPipeline(group, false, false, "main", "positive"),
    p.candidate.pipeline,
  );
  for (const renderer of [p.baseline, p.candidate])
    await renderer.prepareStockPipelines(0, false, false);
  assert.equal(p.candidate.stockChoiceRevision, 6);
  assert.notEqual(
    p.candidate.materialPipeline(group, false, false, "main", "positive"),
    p.candidate.pipeline,
  );
  assertDraws(p);
  const revision = p.candidate.stockChoiceRevision;
  p.candidate.stockModule = { code: "PIPELINE-FAIL" };
  await assert.rejects(
    p.candidate.prepareStockPipelines(1, false, false),
    /GPU pipeline failure/,
  );
  assert.equal(p.candidate.stockChoiceRevision, revision);
});

test("new or changed material bindings do not inherit another group's choices", () => {
  const p = pair();
  const oldGroup = p.groups[0],
    newGroup = {};
  p.candidate.materialPipeline(oldGroup, false, false, "main", "positive");
  for (const renderer of [p.baseline, p.candidate])
    renderer.materialFeatures.set(newGroup, 0x20000);
  assert.equal(
    p.candidate.materialPipeline(newGroup, false, false, "main", "positive"),
    p.baseline.materialPipeline(newGroup, false, false, "main", "positive"),
  );
  for (const renderer of [p.baseline, p.candidate])
    renderer.materialFeatures.set(oldGroup, 0x10000);
  assert.equal(
    p.candidate.materialPipeline(oldGroup, false, false, "depth", "positive"),
    p.baseline.materialPipeline(oldGroup, false, false, "depth", "positive"),
  );
  assert.notEqual(
    p.candidate.stockChoices.get(newGroup),
    p.candidate.stockChoices.get(oldGroup),
  );
});

test("unknown policy, missing features, compile and validation errors retain baseline semantics", () => {
  const p = pair();
  const invoke = (renderer: any, args: unknown[]) => {
    try {
      return { value: renderer.materialPipeline(...args) };
    } catch (error) {
      return { error: String(error) };
    }
  };
  const g = p.groups[0];
  for (const args of [
    [g, false, false, "main", "bad"],
    [g, 1, false, "main", "positive"],
    [g, false, null, "main", "positive"],
    [g, false, false, "unknown", "positive"],
    [{}, false, false, "depth", "negative"],
    [null, false, false, "main", "positive"],
    [g, false, false, "main"],
  ])
    assert.deepEqual(invoke(p.candidate, args), invoke(p.baseline, args));
  for (const features of [-1, NaN, Infinity, 0xffffffff, "invalid"])
    for (const renderer of [p.baseline, p.candidate]) {
      renderer.materialFeatures.set(g, features);
      assert.throws(
        () => renderer.materialPipeline(g, false, false, "main", "positive"),
        /Choose complete stock material and source raster settings/,
      );
    }
});

test("each live material has at most36 slots independent of camera count; warm choices avoid resolver work", () => {
  const p = pair();
  let resolutions = 0;
  const resolve = p.candidate.resolveMaterialPipeline;
  p.candidate.resolveMaterialPipeline = function (...args: any[]) {
    resolutions++;
    return resolve.apply(this, args);
  };
  for (let camera = 0; camera < 500; camera++)
    for (const kind of kinds)
      for (const blend of [false, true])
        for (const additive of [false, true])
          for (const orientation of orientations)
            p.candidate.materialPipeline(
              p.groups[0],
              blend,
              additive,
              kind,
              orientation,
            );
  assert.equal(resolutions, 36);
  const choice = p.candidate.stockChoices.get(p.groups[0]);
  assert.equal(choice.pipelines.length, 36);
  assert.equal(Object.keys(choice.pipelines).length, 36);
  p.candidate.stockChoiceRevision++;
  p.candidate.materialPipeline(p.groups[0], false, false, "main", "positive");
  assert.equal(p.candidate.stockChoices.get(p.groups[0]), choice);
  assert.equal(Object.keys(choice.pipelines).length, 1);
});
