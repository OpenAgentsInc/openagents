import assert from "node:assert/strict";
import test from "node:test";
import {
  SourceRenderBundles,
  type SourceBundleBindings,
  type BundledSourceDraw,
} from "../packages/renderer/source-render-bundles.ts";

Object.assign(globalThis, { GPUBufferUsage: { INDIRECT: 1, COPY_DST: 2 } });
const draws: BundledSourceDraw[] = [
  {
    sourceDrawIndex: 0,
    firstIndex: 0,
    indexCount: 3,
    baseVertex: -1,
    firstInstance: 0,
    count: 2,
    material: 0,
  },
];
function fixture() {
  let fail = false,
    accounted = 0,
    uploads = 0;
  const buffers: any[] = [];
  const device: any = {
    queue: {
      writeBuffer(buffer: any, _offset: number, data: Uint32Array) {
        if (fail) {
          fail = false;
          throw Error("upload failed");
        }
        buffer.words = data.slice();
        uploads++;
      },
    },
    createBuffer(d: any) {
      const b = {
        ...d,
        destroyed: false,
        destroy() {
          this.destroyed = true;
        },
      };
      buffers.push(b);
      return b;
    },
    createRenderBundleEncoder() {
      return {
        setBindGroup() {},
        setVertexBuffer() {},
        setIndexBuffer() {},
        setPipeline() {},
        drawIndexedIndirect() {},
        finish() {
          return {};
        },
      };
    },
  };
  const resource = {} as any;
  const bindings: SourceBundleBindings = {
    key: "shadow:moving",
    descriptor: {
      colorFormats: [],
      depthStencilFormat: "depth32float",
      sampleCount: 1,
    },
    frameGroup: resource,
    shader: resource,
    pipelineVersion: 0,
    materials: resource,
    vertices: resource,
    instances: resource,
    colors: resource,
    uv1: resource,
    wind: resource,
    zeroWind: resource,
    indices: resource,
    include: () => true,
    material: () => resource,
    pipeline: () => resource,
  };
  return {
    create: () =>
      new SourceRenderBundles(device, draws, (delta) => {
        accounted += delta;
      }),
    bindings,
    buffers,
    fail: () => {
      fail = true;
    },
    accounted: () => accounted,
    uploads: () => uploads,
  };
}

test("failed indirect upload cannot reuse stale commands and identical retry writes again", () => {
  const f = fixture(),
    table = f.create();
  table.update(draws);
  table.get(f.bindings);
  const edited = [{ ...draws[0], count: 1, firstInstance: 1 }];
  f.fail();
  assert.throws(() => table.update(edited), /upload failed/);
  assert.throws(() => table.get(f.bindings), /Upload the complete/);
  const count = f.uploads();
  table.update(edited);
  assert.equal(f.uploads(), count + 1);
  assert.deepEqual(Array.from(f.buffers[0].words), [3, 1, 0, 0xffffffff, 1]);
  table.get(f.bindings);
  table.dispose();
  assert.equal(f.accounted(), 0);
});

test("failed constructor upload destroys and unaccounts its buffer", () => {
  const f = fixture();
  f.fail();
  assert.throws(f.create, /upload failed/);
  assert.equal(f.accounted(), 0);
  assert.equal(f.buffers.length, 1);
  assert.equal(f.buffers[0].destroyed, true);
});
