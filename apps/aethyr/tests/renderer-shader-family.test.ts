import assert from "node:assert/strict";
import test from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((yes) => {
    resolve = yes;
  });
  return { promise, resolve };
}
const fields = [
  "pipeline",
  "blendPipeline",
  "additivePipeline",
  "planarPipeline",
  "planarBlendPipeline",
  "planarAdditivePipeline",
  "shadowPipeline",
  "depthPipeline",
] as const;
function fixture() {
  const renderer: any = new Renderer(),
    descriptors: any[] = [],
    modules: string[] = [],
    original = Object.fromEntries(
      fields.map((name) => [name, { name, getBindGroupLayout: () => ({}) }]),
    );
  const began = deferred();
  let gate: ReturnType<typeof deferred> | undefined,
    scopes = 0,
    popped = 0,
    destroys = 0,
    failPipeline = false,
    resets = 0;
  const device = {
    pushErrorScope() {
      scopes++;
    },
    async popErrorScope() {
      popped++;
      return null;
    },
    createShaderModule({ code }: { code: string }) {
      modules.push(code);
      return {
        code,
        async getCompilationInfo() {
          if (gate) {
            began.resolve();
            await gate.promise;
          }
          return {
            messages: code.startsWith("invalid")
              ? [{ type: "error", lineNum: 1, message: "Invalid test shader." }]
              : [],
          };
        },
      };
    },
    async createRenderPipelineAsync(descriptor: any) {
      if (failPipeline) throw new Error("Pipeline test failure.");
      descriptors.push(descriptor);
      return {
        number: descriptors.length,
        descriptor,
        getBindGroupLayout: () => ({}),
      };
    },
    createPipelineLayout: () => ({}),
    destroy() {
      destroys++;
    },
  };
  Object.assign(renderer, original, {
    device,
    layout: {},
    materialLayout: {},
    geometryVersion: 7,
    shadowDirty: false,
    windShadowPlanner: {
      reset() {
        resets++;
      },
    },
  });
  const opaque = new Map([["back:ccw", { name: "stock opaque" }]]);
  renderer.opaqueDepthPipelines = opaque;
  return {
    renderer,
    original,
    opaque,
    source: renderer.materialShaderSource,
    descriptors,
    modules,
    hold() {
      gate = deferred();
      return { began: began.promise, release: gate.resolve };
    },
    fail() {
      failPipeline = true;
    },
    counts: () => ({ scopes, popped, destroys, resets }),
  };
}
function sameFamily(renderer: any, family: Record<string, unknown>) {
  for (const field of fields)
    assert.equal(renderer[field], family[field], field);
}

test("exact owned stock no-op and restoration reuse every original generic and opaque-depth handle", async () => {
  const f = fixture(),
    r = f.renderer;
  const noOp = await r.prepareShader(f.source);
  assert(noOp.ready());
  noOp.activate();
  sameFamily(r, f.original);
  assert.equal(f.modules.length, 0);
  assert.equal(f.descriptors.length, 0);
  assert.equal(
    await r.replaceShader("custom vertex and fragment source"),
    true,
  );
  assert.notEqual(r.shadowPipeline, f.original.shadowPipeline);
  assert.notEqual(r.depthPipeline, f.original.depthPipeline);
  assert.equal(f.descriptors.length, 8);
  assert.equal(f.descriptors[6].vertex.entryPoint, "vs");
  assert.equal(
    f.descriptors[7].vertex.module.code,
    "custom vertex and fragment source",
  );
  const plan = await r.prepareShader(f.source);
  assert.equal(r.materialShaderSource, "custom vertex and fragment source");
  assert(plan.ready());
  plan.activate();
  sameFamily(r, f.original);
  assert.equal(r.opaqueDepthPipelines, f.opaque);
  assert.equal(r.materialShaderSource, f.source);
  assert.equal(f.descriptors.length, 8);
  assert.equal(f.modules.length, 1);
  assert.equal(f.counts().scopes, f.counts().popped);
});

test("custom variants preserve color blending, winding and independent depth/shadow entry points", async () => {
  const f = fixture();
  await f.renderer.replaceShader("custom checked descriptors");
  for (let index = 0; index < 6; index++) {
    const descriptor = f.descriptors[index],
      variant = index % 3;
    assert.equal(descriptor.vertex.entryPoint, "vs");
    assert.equal(descriptor.fragment.entryPoint, "fs");
    assert.equal(descriptor.primitive.frontFace, index < 3 ? "ccw" : "cw");
    assert.equal(descriptor.multisample.count, 4);
    assert.equal(descriptor.fragment.targets[0].format, "rgba16float");
    assert.equal(
      descriptor.fragment.targets[0].blend?.color.dstFactor,
      variant === 0 ? undefined : variant === 1 ? "one-minus-src-alpha" : "one",
    );
  }
  const depth = f.descriptors[6],
    shadow = f.descriptors[7];
  assert.equal(depth.vertex.entryPoint, "vs");
  assert.equal(depth.fragment.entryPoint, "depthFS");
  assert.equal(depth.depthStencil.format, "depth24plus");
  assert.equal(depth.multisample.count, 4);
  assert.equal(shadow.vertex.entryPoint, "shadowVS");
  assert.equal(shadow.fragment.entryPoint, "shadowFS");
  assert.equal(shadow.depthStencil.format, "depth32float");
  assert.equal(shadow.depthStencil.depthBias, 2);
  assert.equal(shadow.depthStencil.depthBiasSlopeScale, 2);
  assert.equal(f.renderer.depthPipeline.descriptor, depth);
  assert.equal(f.renderer.shadowPipeline.descriptor, shadow);
});

test("custom sources keep their own newly compiled families and never replace the retained stock family", async () => {
  const f = fixture(),
    r = f.renderer;
  await r.replaceShader("custom-A");
  const first = r.shadowPipeline,
    firstDepth = r.depthPipeline;
  await r.replaceShader("custom-A");
  assert.notEqual(r.shadowPipeline, first);
  assert.notEqual(r.depthPipeline, firstDepth);
  assert.equal(f.descriptors.length, 16);
  await r.replaceShader("custom-B");
  assert.equal(f.descriptors.length, 24);
  assert.equal(r.stockPipelineFamily.pipelines[0], f.original.pipeline);
  assert.equal(r.stockPipelineFamily.pipelines[6], f.original.shadowPipeline);
  assert.equal(r.stockPipelineFamily.pipelines.length, 8);
  assert.equal(Object.keys(r.stockPipelineFamily).length, 2);
  await r.replaceShader(f.source);
  sameFamily(r, f.original);
  assert.equal(f.descriptors.length, 24);
});

test("failed validation and pipeline creation leave the last working shader and stock handles untouched", async () => {
  const f = fixture(),
    r = f.renderer;
  await r.replaceShader("working custom");
  const current = Object.fromEntries(fields.map((field) => [field, r[field]])),
    version = r.geometryVersion;
  await assert.rejects(
    r.prepareShader("invalid shader"),
    /Invalid test shader/,
  );
  sameFamily(r, current);
  assert.equal(r.materialShaderSource, "working custom");
  assert.equal(r.geometryVersion, version);
  f.fail();
  await assert.rejects(
    r.prepareShader("failing pipeline"),
    /Pipeline test failure/,
  );
  sameFamily(r, current);
  assert.equal(r.geometryVersion, version);
  await r.replaceShader(f.source);
  sameFamily(r, f.original);
  assert.equal(f.counts().scopes, f.counts().popped);
});

test("cached stock candidates cannot activate after cancellation, supersession or disposal", async () => {
  const f = fixture(),
    r = f.renderer;
  await r.replaceShader("custom active");
  const active = r.pipeline,
    version = r.geometryVersion;
  let allowed = true;
  const cancelled = await r.prepareShader(f.source, () => allowed);
  allowed = false;
  assert.equal(cancelled.ready(), false);
  cancelled.activate();
  assert.equal(r.pipeline, active);
  assert.equal(r.geometryVersion, version);
  const stale = await r.prepareShader(f.source);
  await r.prepareShader("new pending custom");
  assert.equal(stale.ready(), false);
  stale.activate();
  assert.equal(r.pipeline, active);
  assert.equal(r.geometryVersion, version);
  const disposed = await r.prepareShader(f.source);
  r.dispose();
  assert.equal(r.stockPipelineFamily, undefined);
  assert.equal(disposed.ready(), false);
  disposed.activate();
  assert.equal(r.pipeline, active);
  assert.equal(f.counts().destroys, 1);
});

test("the existing three-request bound and serialized stale compilation behavior remain unchanged", async () => {
  const f = fixture(),
    r = f.renderer,
    hold = f.hold();
  const a = r.prepareShader("queued-A");
  await hold.began;
  const b = r.prepareShader("queued-B"),
    c = r.prepareShader(f.source);
  await assert.rejects(r.prepareShader("overflow"), /current shader edits/);
  hold.release();
  assert.equal(await a, undefined);
  assert.equal(await b, undefined);
  const stock = await c;
  assert(stock.ready());
  stock.activate();
  sameFamily(r, f.original);
  assert.equal(f.descriptors.length, 8);
  assert.equal(r.shaderPending, 0);
});
