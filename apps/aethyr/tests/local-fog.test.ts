import assert from "node:assert/strict";
import { test } from "node:test";
import type { SourceLight } from "../packages/assets/src/scene.ts";
import {
  LocalFog,
  localFogOpticalDepth,
  localFogPhase,
  sourceLocalFogProfile,
  type LocalFogProfile,
} from "../packages/renderer/local-fog.ts";

const medium: LocalFogProfile = {
  enabled: true,
  extinctionAtReference: 0.1,
  referenceHeight: 0,
  heightRate: 0,
  albedo: 0.8,
  phaseG: 0.2,
  startDistance: 0,
  maxDistance: 60,
};
test("height extinction integrates constant and reversed elevated paths, caps distance, and normalizes HG phase", () => {
  assert.ok(Math.abs(localFogOpticalDepth(medium, 7, 0.6, 12) - 1.2) < 1e-12);
  assert.equal(localFogOpticalDepth(medium, 7, 0, 100), 6);
  assert.equal(
    localFogOpticalDepth({ ...medium, startDistance: 2 }, 0, 0, 1),
    0,
  );
  const elevated = { ...medium, heightRate: 0.4 };
  const ascent = localFogOpticalDepth(elevated, 0, 0.5, 10);
  const descent = localFogOpticalDepth(elevated, 5, -0.5, 10);
  assert.ok(Math.abs(ascent - descent) < 1e-12);
  assert.ok(Math.abs(ascent - (0.1 * (1 - Math.exp(-2))) / 0.2) < 1e-12);
  assert.equal(localFogPhase(0, 0), 1 / (4 * Math.PI));
  let integral = 0;
  for (let i = 0; i < 10000; i++)
    integral +=
      localFogPhase(-1 + (i + 0.5) / 5000, 0.2) * (2 / 10000) * 2 * Math.PI;
  assert.ok(Math.abs(integral - 1) < 1e-7);
  assert.ok(localFogPhase(1, 0.2) > localFogPhase(-1, 0.2));
  assert.throws(() => localFogOpticalDepth(medium, 0, NaN, 1));
  assert.throws(() => localFogPhase(0, 1));
});

test("recovered fog properties supply source height, volume units, albedo and enable flag independently of analytic fog color", () => {
  const actor = {
    transform: { location_cm: [0, 0, -1000] },
    components: [
      {
        class: "ExponentialHeightFogComponent",
        properties: {
          enable_volumetric_fog: true,
          fog_density: 0.2,
          fog_height_falloff: 0.5,
          volumetric_fog_extinction_scale: 2,
          volumetric_fog_scattering_distribution: 0.3,
          volumetric_fog_distance: 4000,
          volumetric_fog_start_distance: 100,
          volumetric_fog_albedo: "(B=128,G=128,R=128,A=255)",
          fog_inscattering_luminance: "(R=100,G=0,B=0,A=1)",
        },
      },
    ],
  };
  const profile = sourceLocalFogProfile([actor]);
  assert.equal(profile.enabled, true);
  assert.equal(profile.maxDistance, 40);
  assert.equal(profile.startDistance, 1);
  assert.equal(profile.phaseG, 0.3);
  assert.ok(Math.abs(profile.heightRate - 0.05 * Math.LN2) < 1e-10);
  assert.ok(
    Math.abs(
      profile.extinctionAtReference - 0.02 * Math.exp(-profile.heightRate * 11),
    ) < 1e-10,
  );
  assert.ok(Math.abs(profile.albedo - 0.2158605001) < 1e-9);
  assert.equal(sourceLocalFogProfile([]).enabled, false);
  actor.components[0].properties.enable_volumetric_fog = false;
  assert.equal(sourceLocalFogProfile([actor]).enabled, false);
});

test("source strengths preserve cluster source order and zero flags; resize bounds resources and disposal", async () => {
  Object.assign(globalThis, {
    GPUBufferUsage: { UNIFORM: 1, STORAGE: 2, COPY_DST: 4 },
    GPUTextureUsage: { TEXTURE_BINDING: 1, RENDER_ATTACHMENT: 2, COPY_SRC: 4 },
  });
  const buffers: Array<{ size: number; destroyed: number }> = [],
    textures: Array<{ size: number[]; destroyed: number }> = [],
    writes: number[][] = [];
  let groupCount = 0;
  const device = {
    createShaderModule: () => ({}),
    createSampler: () => ({}),
    createRenderPipelineAsync: async () => ({ getBindGroupLayout: () => ({}) }),
    createBindGroup: () => ({ id: ++groupCount }),
    createBuffer: ({ size }: { size: number }) => {
      const buffer = {
        size,
        destroyed: 0,
        destroy() {
          this.destroyed++;
        },
      };
      buffers.push(buffer);
      return buffer;
    },
    createTexture: ({ size }: { size: number[] }) => {
      const texture = {
        size,
        destroyed: 0,
        createView: () => ({}),
        destroy() {
          this.destroyed++;
        },
      };
      textures.push(texture);
      return texture;
    },
    queue: {
      writeBuffer: (_buffer: unknown, _offset: number, data: Float32Array) =>
        writes.push(Array.from(data)),
    },
  } as unknown as GPUDevice;
  const fog = await LocalFog.create(device);
  const lights = [
    { raw: { volumetric_scattering_intensity: 150 } },
    { raw: { volumetric_scattering_intensity: 0 } },
    { raw: {} },
  ] as SourceLight[];
  fog.setLightScattering(lights, new Uint32Array([1, 0, 2]), "night-1");
  assert.deepEqual(writes.at(-1), [0, 150, 1, 0]);
  fog.setLightScattering(lights, new Uint32Array([1, 0, 2]), "night-1");
  assert.equal(writes.length, 1);
  assert.throws(() => fog.setLightScattering(lights, [99], "bad"));
  fog.resize(3840, 2160);
  assert.deepEqual(
    textures.slice(1).map((t) => t.size),
    [
      [3840, 2160],
      [640, 360],
    ],
  );
  const view = fog.view;
  fog.resize(3840, 2160);
  assert.equal(fog.view, view);
  fog.resize(100, 200);
  assert.equal(textures[1].destroyed, 1);
  assert.equal(textures[2].destroyed, 1);
  assert.deepEqual(textures.at(-1)?.size, [50, 100]);
  assert.throws(() => fog.resize(Infinity, 1));
  const pass = { setPipeline() {}, setBindGroup() {}, draw() {}, end() {} };
  const encoder = {
    beginRenderPass: () => pass,
  } as unknown as GPUCommandEncoder;
  const renderFrame = {
    width: 100,
    height: 200,
    profile: medium,
    sceneColor: {} as GPUTextureView,
    depthView: {} as GPUTextureView,
    frameBuffer: {} as GPUBuffer,
    clusterBuffer: {} as GPUBuffer,
    localShadowView: {} as GPUTextureView,
    shadowSampler: {} as GPUSampler,
  };
  assert.throws(() =>
    fog.render(encoder, { ...renderFrame, sceneColor: fog.view }),
  );
  fog.render(encoder, renderFrame);
  assert.equal(groupCount, 4);
  fog.render(encoder, renderFrame);
  assert.equal(groupCount, 4);
  fog.render(encoder, { ...renderFrame, profile: { ...medium, phaseG: 0.3 } });
  assert.equal(groupCount, 4);
  renderFrame.frameBuffer = {} as GPUBuffer;
  fog.render(encoder, renderFrame);
  assert.equal(groupCount, 6);
  renderFrame.depthView = {} as GPUTextureView;
  fog.render(encoder, renderFrame);
  assert.equal(groupCount, 8);
  renderFrame.sceneColor = {} as GPUTextureView;
  fog.render(encoder, renderFrame);
  assert.equal(groupCount, 9);
  renderFrame.clusterBuffer = {} as GPUBuffer;
  fog.render(encoder, renderFrame);
  assert.equal(groupCount, 10);
  renderFrame.localShadowView = {} as GPUTextureView;
  fog.render(encoder, renderFrame);
  assert.equal(groupCount, 11);
  const directional = {
    scatteringIntensity: 1,
    nearShadowView: {} as GPUTextureView,
    farShadowView: {} as GPUTextureView,
  };
  fog.render(encoder, { ...renderFrame, directional });
  assert.equal(groupCount, 12);
  fog.render(encoder, {
    ...renderFrame,
    directional: { ...directional, scatteringIntensity: 2.2 },
  });
  assert.equal(groupCount, 12);
  fog.setLightScattering(lights, [0, 1, 2, 0, 1], "larger");
  fog.render(encoder, { ...renderFrame, directional });
  assert.equal(groupCount, 13);
  renderFrame.width = 200;
  renderFrame.height = 100;
  fog.render(encoder, { ...renderFrame, directional });
  assert.equal(groupCount, 14);
  renderFrame.shadowSampler = {} as GPUSampler;
  fog.render(encoder, { ...renderFrame, directional });
  assert.equal(groupCount, 15);
  const distant = { ...directional, distantShadowView: {} as GPUTextureView };
  fog.render(encoder, { ...renderFrame, directional: distant });
  assert.equal(groupCount, 16);
  fog.render(encoder, { ...renderFrame, directional: distant });
  assert.equal(groupCount, 16);
  fog.dispose();
  fog.dispose();
  assert.ok(textures.every((t) => t.destroyed === 1));
  assert.ok(buffers.every((b) => b.destroyed === 1));
  assert.throws(() => fog.resize(1, 1));
  assert.throws(() => fog.setLightScattering([], [], "disposed"));
});
