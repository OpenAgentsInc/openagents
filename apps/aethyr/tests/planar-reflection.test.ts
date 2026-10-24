import assert from "node:assert/strict";
import { test } from "node:test";
import {
  PlanarReflection,
  planarCamera,
  type PlanarReflectionRequest,
} from "../packages/renderer/planar-reflection.ts";
import {
  lookAt,
  modelMatrix,
  multiply,
  perspective,
  transformPoint,
} from "../packages/renderer/math.ts";

const eye: [number, number, number] = [3, 4, 8];
const viewProjection = multiply(
  perspective(1, 16 / 9, 0.1, 100),
  lookAt(eye, [0, 1, 0]),
);
const project = (m: ArrayLike<number>, p: number[]) =>
  [0, 1, 2, 3].map(
    (row) =>
      m[row] * p[0] + m[row + 4] * p[1] + m[row + 8] * p[2] + m[row + 12],
  );

test("mirrored projection preserves the water plane and reflects actual local geometry", () => {
  const camera = planarCamera(viewProjection, eye, 1.5);
  assert.deepEqual(camera.eye, [3, -1, 8]);
  assert.equal(camera.frontFace, "cw");
  for (const point of [
    [0, 1.5, 0],
    [2, 1.5, -3],
    [-4, 1.5, 1],
  ]) {
    const a = project(viewProjection, point),
      b = project(camera.viewProjection, point);
    a.forEach((n, i) => assert.ok(Math.abs(n - b[i]) < 2e-6));
  }
  const above = [1, 3, -2],
    mirrored = [1, 0, -2];
  project(viewProjection, mirrored).forEach((n, i) =>
    assert.ok(Math.abs(n - project(camera.viewProjection, above)[i]) < 2e-6),
  );
  const clip = (y: number) => camera.clipPlane[1] * y + camera.clipPlane[3];
  assert.ok(clip(1.52) > 0);
  assert.ok(clip(1.49) < 0);
});

test("mirrored inverse projection recovers world position and includes a handedness flip", () => {
  const camera = planarCamera(modelMatrix([0, 0, 0]), [0, 2, 3], 0.5, 0);
  assert.deepEqual(
    Array.from(camera.viewProjection),
    [1, 0, 0, 0, 0, -1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1],
  );
  const p: [number, number, number] = [2, 3, 4];
  const clip = transformPoint(camera.viewProjection, p);
  assert.deepEqual(transformPoint(camera.inverseViewProjection, clip), p);
});

test("invalid camera, singular projection and unbounded clip offsets are rejected", () => {
  assert.throws(() => planarCamera(new Float32Array(16), eye, 0), /singular/);
  assert.throws(() => planarCamera([NaN], eye, 0), /finite camera/);
  assert.throws(() => planarCamera(viewProjection, [1, 2], 0), /finite camera/);
  assert.throws(
    () => planarCamera(viewProjection, eye, Infinity),
    /finite camera/,
  );
  for (const bias of [-0.01, 0.2, NaN])
    assert.throws(
      () => planarCamera(viewProjection, eye, 0, bias),
      /clip bias/,
    );
});

function mock() {
  Object.assign(globalThis, {
    GPUTextureUsage: { RENDER_ATTACHMENT: 16, TEXTURE_BINDING: 4 },
  });
  const textures: Array<{
    descriptor: GPUTextureDescriptor;
    destroyed: boolean;
  }> = [];
  const passes: Array<{ descriptor: GPURenderPassDescriptor; ended: boolean }> =
    [];
  const device = {
    limits: { maxTextureDimension2D: 8192 },
    createTexture(descriptor: GPUTextureDescriptor) {
      const entry = { descriptor, destroyed: false };
      textures.push(entry);
      return {
        createView() {
          return { entry };
        },
        destroy() {
          entry.destroyed = true;
        },
      };
    },
  } as unknown as GPUDevice;
  const encoder = {
    beginRenderPass(descriptor: GPURenderPassDescriptor) {
      const entry = { descriptor, ended: false };
      passes.push(entry);
      return {
        end() {
          entry.ended = true;
        },
      };
    },
  } as unknown as GPUCommandEncoder;
  const request: PlanarReflectionRequest = {
    viewProjection,
    eye,
    planeY: 0,
    width: 1280,
    height: 720,
    sceneRevision: 1,
    lightingRevision: 1,
    environmentRevision: "owned-sky",
  };
  return { textures, passes, device, encoder, request };
}

test("offscreen targets remain linear HDR, capped at half resolution and one primary plane", () => {
  const { device, encoder, request, textures, passes } = mock();
  const planar = new PlanarReflection(device);
  const first = planar.render(
    encoder,
    { ...request, width: 3840, height: 2160 },
    (_pass, context) => {
      assert.equal(context.width, 1024);
      assert.equal(context.height, 576);
      assert.equal(context.frontFace, "cw");
    },
  )!;
  assert.equal(first.width, 1024);
  assert.equal(first.height, 576);
  assert.equal(textures.length, 3);
  assert.equal(textures[0].descriptor.format, "rgba16float");
  assert.equal(textures[0].descriptor.sampleCount, 4);
  assert.equal(textures[1].descriptor.format, "rgba16float");
  assert.equal(textures[2].descriptor.format, "depth24plus");
  assert.equal(passes[0].ended, true);
  first.markSubmitted();
  assert.equal(
    planar.render(encoder, { ...request, width: 3840, height: 2160 }, () =>
      assert.fail("Cached contents must not draw"),
    )!.updated,
    false,
  );
  const second = planar.render(encoder, request, () => {})!;
  assert.equal(second.width, 640);
  assert.equal(second.height, 360);
  assert.ok(textures.slice(0, 3).every((t) => t.destroyed));
  planar.dispose();
  assert.ok(textures.every((t) => t.destroyed));
  assert.throws(() => planar.render(encoder, request, () => {}), /closed/);
});

test("only the latest submitted encoding seals a cache entry; every scene dependency invalidates it", () => {
  const { device, encoder, request, passes } = mock();
  const planar = new PlanarReflection(device);
  const a = planar.render(encoder, request, () => {})!;
  const b = planar.render(encoder, request, () => {})!;
  a.markSubmitted();
  const c = planar.render(encoder, request, () => {})!;
  b.markSubmitted();
  c.markSubmitted();
  assert.equal(
    planar.render(encoder, request, () => assert.fail())!.updated,
    false,
  );
  assert.equal(passes.length, 3);
  for (const update of [
    { sceneRevision: 2 },
    { lightingRevision: 2 },
    { environmentRevision: "changed" },
    { planeY: 0.1 },
    { clipBias: 0.02 },
    { eye: [3, 4.1, 8] },
    { viewProjection: multiply(viewProjection, modelMatrix([0.1, 0, 0])) },
  ]) {
    const changed = planar.render(
      encoder,
      { ...request, ...update },
      () => {},
    )!;
    assert.equal(changed.updated, true);
    changed.markSubmitted();
    const baseline = planar.render(encoder, request, () => {})!;
    baseline.markSubmitted();
  }
  planar.invalidate();
  assert.equal(planar.render(encoder, request, () => {})!.updated, true);
  for (let i = 0; i < 2; i++)
    assert.equal(
      planar.render(encoder, { ...request, cache: false }, () => {})!.updated,
      true,
    );
  planar.dispose();
});

test("callback failure ends the pass without caching, while below-plane cameras allocate nothing", () => {
  const { device, encoder, request, passes, textures } = mock();
  const planar = new PlanarReflection(device);
  assert.equal(
    planar.render(encoder, { ...request, eye: [0, -0.1, 3] }, () =>
      assert.fail(),
    ),
    undefined,
  );
  assert.equal(textures.length, 0);
  assert.throws(
    () =>
      planar.render(encoder, request, () => {
        throw new Error("owned draw failure");
      }),
    /draw failure/,
  );
  assert.equal(passes[0].ended, true);
  assert.equal(planar.render(encoder, request, () => {})!.updated, true);
  assert.throws(
    () => planar.render(encoder, { ...request, width: 0 }, () => {}),
    /dimensions/,
  );
  assert.throws(
    () =>
      planar.render(encoder, { ...request, sceneRevision: Infinity }, () => {}),
    /revisions/,
  );
  assert.throws(
    () => new PlanarReflection(device, { resolutionScale: 1 }),
    /half resolution/,
  );
  assert.throws(
    () => new PlanarReflection(device, { maxDimension: 2048 }),
    /1024/,
  );
  planar.dispose();
});
