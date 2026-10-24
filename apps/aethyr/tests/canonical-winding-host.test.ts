import test from "node:test";
import assert from "node:assert/strict";
import { Renderer } from "../packages/renderer/renderer.ts";
import { prepareCanonicalSourceIndices } from "../packages/renderer/canonical-source-indices.ts";
import { selectedRendererFeatures as playerFeatures } from "../packages/renderer/player-features.ts";
import { selectedRendererFeatures as roomFeatures } from "../packages/renderer/room-features.ts";
import { selectedRendererFeatures as editorFeatures } from "../packages/renderer/features.ts";
import { buildInstanceBounds } from "../packages/renderer/culling.ts";
import {
  PackedFrameCore,
  bindResidentFrameScene,
} from "../packages/renderer/resident-source.ts";
import {
  modelMatrix,
  multiply,
  perspective,
  lookAt,
} from "../packages/renderer/math.ts";

test("canonical preparation belongs to full player/editor features and is absent from Room", () => {
  assert.equal(
    playerFeatures.prepareCanonicalSourceIndices,
    prepareCanonicalSourceIndices,
  );
  assert.equal(
    editorFeatures.prepareCanonicalSourceIndices,
    prepareCanonicalSourceIndices,
  );
  assert.equal(roomFeatures.prepareCanonicalSourceIndices, undefined);
});

test("stock camera/planar use canonical ranges while custom/shadow and restored stock retain exact source tables", () => {
  const saved = (globalThis as any).GPUBufferUsage;
  (globalThis as any).GPUBufferUsage = { VERTEX: 1, COPY_DST: 2, INDIRECT: 4 };
  try {
    const model = modelMatrix([0, 0, -5], 0),
      models = Float32Array.from(model);
    const draws = [
      {
        firstIndex: 0,
        indexCount: 3,
        baseVertex: 0,
        firstInstance: 0,
        count: 1,
        material: 0,
        sourceDrawIndex: 0,
        wind: false,
        rasterOrientation: "negative" as const,
      },
    ];
    const prepared = prepareCanonicalSourceIndices(
      new Uint32Array([0, 1, 2]),
      new Map([["night", draws]]),
      () => 0,
    );
    assert(prepared);
    const bounds = buildInstanceBounds({
        instances: [{ mesh: 0, model }],
        meshes: [{ bounds: { min: [-1, -1, -1], max: [1, 1, 1] } }],
      } as any),
      wind = new Float32Array(4);
    const core = new PackedFrameCore(),
      resident = bindResidentFrameScene(
        core,
        () => ({ bounds, models, draws, wind }),
        0,
      );
    const variant = { draws, bounds, models, wind: { parameters: wind } },
      matrix = multiply(perspective(1, 1, 0.1), lookAt([0, 1, 2], [0, 0, -5]));
    const r: any = {
      variants: new Map([["night", variant]]),
      destroyedPlacements: new Map(),
      cullingEnabled: true,
      geometryVersion: 0,
      buffers: new Set(),
      gpuBytes: 0,
      preparedBufferUploadBytes: 0,
      culledSources: new Map(),
      frameScenes: new Map([["night", resident]]),
      experiment: { framePreparation: {} },
      sourceBundlesEnabled: true,
      device: {
        createBuffer: ({ size }: any) => ({ size, destroy() {} }),
        queue: { writeBuffer() {} },
      },
      releaseBuffer() {},
      canonicalDraws: prepared.variants,
      shaders: { MATERIAL: "stock" },
      activeShaderSource: "stock",
    };
    const prepare = (Renderer.prototype as any).prepareSourceSubmission;
    const canonical = prepare.call(r, matrix, "camera", "night");
    assert.equal(canonical.draws[0].firstIndex, 3);
    assert.equal(canonical.draws[0].rasterOrientation, "positive");
    assert.equal(canonical.draws[0].originalFirstIndex, 0);
    const canonicalBundle = canonical.bundles;
    const planar = prepare.call(r, matrix, "planar", "night");
    assert.equal(planar.draws[0].firstIndex, 3);
    const shadow = prepare.call(r, matrix, "local:0", "night");
    assert.equal(shadow.draws[0].firstIndex, 0);
    assert.equal(shadow.draws[0].rasterOrientation, "negative");
    r.activeShaderSource = "custom";
    const custom = prepare.call(r, matrix, "camera", "night");
    assert.equal(custom.draws[0].firstIndex, 0);
    assert.equal(custom.draws[0].rasterOrientation, "negative");
    assert.notEqual(custom.bundles, canonicalBundle);
    assert.throws(() => canonicalBundle.commandSnapshot(), /closed/);
    r.activeShaderSource = "stock";
    const restored = prepare.call(r, matrix, "camera", "night");
    assert.deepEqual(restored.draws, canonical.draws);
    assert.equal(restored.bundles.commandSnapshot()[2], 3);
    assert.equal(draws[0].firstIndex, 0);
    assert.equal(draws[0].rasterOrientation, "negative");
    r.destroyedPlacements.set("night", new Set([0]));
    r.geometryVersion++;
    const destroyed = prepare.call(r, matrix, "camera", "night");
    assert.deepEqual(destroyed.draws, []);
    assert.equal(destroyed.bundles.commandSnapshot()[1], 0);
    r.destroyedPlacements.clear();
    r.geometryVersion++;
    const recovered = prepare.call(r, matrix, "camera", "night");
    assert.equal(recovered.draws[0].firstIndex, 3);
    assert.equal(recovered.bundles.commandSnapshot()[2], 3);
    r.canonicalDraws = undefined;
    const original = prepare.call(r, matrix, "camera", "night");
    assert.deepEqual(original.draws, shadow.draws);
    assert.equal(original.bundles.commandSnapshot()[2], 0);
    core.dispose();
  } finally {
    (globalThis as any).GPUBufferUsage = saved;
  }
});

test("wire draws use original edge offsets while shaded draws keep canonical triangle offsets", () => {
  const calls: any[] = [],
    indices = {},
    edges = {},
    material = {};
  const r: any = {
    vertices: {},
    instances: {},
    vertexColors: {},
    vertexUV1: {},
    indices,
    meshIndices: edges,
    zeroWind: {},
    renderMaterialInfo: [{ alphaMode: "OPAQUE" }],
    materials: [material],
  };
  const pass: any = Object.fromEntries(
    ["setVertexBuffer", "setIndexBuffer", "setBindGroup", "drawIndexed"].map(
      (name) => [name, (...args: any[]) => calls.push([name, ...args])],
    ),
  );
  const source = {
    instances: {},
    wind: {},
    draws: [
      {
        indexCount: 3,
        firstIndex: 9,
        originalFirstIndex: 2,
        baseVertex: 4,
        count: 2,
        firstInstance: 7,
        material: 0,
        rasterOrientation: "positive",
      },
    ],
  };
  const draw = (Renderer.prototype as any).draw;
  draw.call(r, pass, false, false, source, true);
  assert.deepEqual(
    calls.find((c) => c[0] === "setIndexBuffer"),
    ["setIndexBuffer", edges, "uint32"],
  );
  assert.deepEqual(
    calls.find((c) => c[0] === "drawIndexed"),
    ["drawIndexed", 6, 2, 4, 4, 7],
  );
  calls.length = 0;
  draw.call(r, pass, false, false, source, false);
  assert.deepEqual(
    calls.find((c) => c[0] === "setIndexBuffer"),
    ["setIndexBuffer", indices, "uint32"],
  );
  assert.deepEqual(
    calls.find((c) => c[0] === "drawIndexed"),
    ["drawIndexed", 3, 2, 9, 4, 7],
  );
});
