import assert from "node:assert/strict";
import { test } from "node:test";
import {
  fogAmbientBandWeights,
  fogAmbientSH,
  fogAmbientProbeIncident,
  fogAmbientProbeVisibility,
  sourceFogAmbientSkyProfile,
  type FogAmbientProbeField,
} from "../packages/renderer/fog-ambient-volume.ts";
import { sh9Basis } from "../packages/renderer/environment-lighting.ts";
import { localFogPhase } from "../packages/renderer/local-fog.ts";
import type { V3 } from "../packages/renderer/math.ts";
const root4pi = Math.sqrt(4 * Math.PI);
function field(primary: V3 = [1, 2, 3], constant?: V3): FogAmbientProbeField {
  const records = new Float32Array(8 * (constant ? 76 : 40));
  for (let i = 0; i < 8; i++) {
    records.set(
      [(i & 1) * 4, ((i >> 1) & 1) * 4, ((i >> 2) & 1) * 4, 1],
      i * 40,
    );
    records.set([...primary.map((n) => n * root4pi), 0], i * 40 + 4);
    if (constant)
      records.set([...constant.map((n) => n * root4pi), 0], 8 * 40 + i * 36);
  }
  return {
    origin: [0, 0, 0],
    spacing: [4, 4, 4],
    dimensions: [2, 2, 2],
    records,
    constantFog: !!constant,
    moments: () => [64, 4096],
  };
}
function close(a: number[], b: number[], epsilon = 1e-6) {
  a.forEach((n, i) => assert.ok(Math.abs(n - b[i]) < epsilon, `${a} != ${b}`));
}
test("HG convolution preserves constants and matches independent spherical integration of retained bands", () => {
  close(
    fogAmbientBandWeights(0.2),
    [1, 0.3, 0.3, 0.3, 0.16, 0.16, 0.16, 0.16, 0.16],
  );
  const coefficients = new Float64Array(36);
  coefficients.set([root4pi, 2 * root4pi, 3 * root4pi]);
  for (const g of [-0.2, 0, 0.2, 0.5])
    for (const direction of [
      [1, 0, 0],
      [0, 1, 0],
      [0, 0, -1],
    ] as V3[])
      close(fogAmbientSH(coefficients, direction, g), [1, 2, 3]);
  const direction: V3 = [0.6, 0, 0.8];
  // A positive band-limited incident field, independently integrated over solid angle.
  coefficients.fill(0);
  coefficients.set([root4pi, root4pi, root4pi]);
  coefficients[4] = 0.25 * (2 / 3);
  coefficients[8] = -0.2 * (2 / 3);
  coefficients[24] = 0.1 * 0.25;
  for (const g of [0, 0.2]) {
    let numeric = 0;
    const count = 30000;
    for (let i = 0; i < count; i++) {
      const y = 1 - (2 * (i + 0.5)) / count,
        phi = i * 2.399963229728653,
        s = Math.sqrt(1 - y * y),
        w: V3 = [s * Math.cos(phi), y, s * Math.sin(phi)],
        basis = sh9Basis(w);
      const radiance = 1 + 0.25 * basis[1] - 0.2 * basis[2] + 0.1 * basis[6];
      numeric +=
        (radiance *
          localFogPhase(
            w[0] * direction[0] + w[1] * direction[1] + w[2] * direction[2],
            g,
          ) *
          4 *
          Math.PI) /
        count;
    }
    assert.ok(
      Math.abs(fogAmbientSH(coefficients, direction, g)[0] - numeric) < 2e-6,
    );
  }
  assert.throws(() => fogAmbientBandWeights(0.51));
  assert.throws(() => fogAmbientSH(coefficients, [0, 0, 0], 0.2));
});
test("volume interpolation combines solar and constant first, has no normal bias or global fallback, and does not renormalize blocked weights", () => {
  const f = field([-2, 1, 3], [3, 0.5, 2]);
  for (const ratio of [0, 1, 2])
    close(
      fogAmbientProbeIncident(f, [2, 2, 2], [0, 0, 1], 0.2, ratio),
      [-2, 1, 3].map((n, i) => Math.max(0, n * ratio + [3, 0.5, 2][i])),
    );
  close(
    fogAmbientProbeIncident(f, [-0.01, 2, 2], [0, 0, 1], 0.2, 1),
    [0, 0, 0],
  );
  f.moments = () => [0, 0];
  const tiny = fogAmbientProbeIncident(f, [2, 2, 2], [0, 0, 1], 0.2, 1);
  assert.ok(tiny.every((n) => n < 1e-12));
  assert.ok(fogAmbientProbeVisibility(1, 0, 0) < 1e-11);
  for (let i = 0; i < 8; i++) f.records[i * 40 + 3] = 0;
  close(fogAmbientProbeIncident(f, [2, 2, 2], [0, 0, 1], 0.2, 1), [0, 0, 0]);
  // Invalid below-floor probes do not erase the valid upper field.
  const floor = field();
  for (let i = 0; i < 8; i++) if ((i & 2) === 0) floor.records[i * 40 + 3] = 0;
  close(fogAmbientProbeIncident(floor, [2, 1, 2], [0, 1, 0], 0, 1), [1, 2, 3]);
});
test("source metadata admits only the supported shadowed realtime movable sky and finite volume strength", () => {
  const sky: any = {
      class: "SkyLightComponent",
      properties: {
        mobility: "MOVABLE",
        visible: true,
        affects_world: true,
        source_type: "SLS_CAPTURED_SCENE",
        real_time_capture: true,
        cast_volumetric_shadow: true,
        volumetric_scattering_intensity: 1,
      },
    },
    fog = {
      class: "ExponentialHeightFogComponent",
      properties: {
        visible: true,
        enable_volumetric_fog: true,
        volumetric_fog_scattering_distribution: 0.2,
      },
    },
    environment = [{ components: [sky, fog] }];
  assert.deepEqual(sourceFogAmbientSkyProfile(environment), {
    enabled: true,
    scatteringIntensity: 1,
  });
  for (const key of ["real_time_capture", "cast_volumetric_shadow"]) {
    sky.properties[key] = false;
    assert.equal(sourceFogAmbientSkyProfile(environment).enabled, false);
    sky.properties[key] = true;
  }
  sky.properties.volumetric_scattering_intensity = 0;
  assert.deepEqual(sourceFogAmbientSkyProfile(environment), {
    enabled: false,
    scatteringIntensity: 0,
  });
  sky.properties.volumetric_scattering_intensity = NaN;
  assert.equal(sourceFogAmbientSkyProfile(environment).enabled, false);
  assert.equal(sourceFogAmbientSkyProfile([]).enabled, false);
  for (const malformed of [
    null,
    {},
    [null],
    [{ components: [null] }],
    [{ components: [sky, fog, fog] }],
    [{ components: [{ class: "SkyLightComponent" }, fog] }],
  ])
    assert.equal(sourceFogAmbientSkyProfile(malformed).enabled, false);
  sky.properties.volumetric_scattering_intensity = 1;
  for (const key of ["visible", "affects_world", "source_type"]) {
    const prior = sky.properties[key];
    delete sky.properties[key];
    assert.equal(sourceFogAmbientSkyProfile(environment).enabled, false);
    sky.properties[key] = prior;
  }
  fog.properties.visible = false;
  assert.equal(sourceFogAmbientSkyProfile(environment).enabled, false);
});

test("fixed GPU field reuses bindings, dispatches every cell, clears disabled frames, and releases resources", async () => {
  Object.assign(globalThis, {
    GPUBufferUsage: { UNIFORM: 1, COPY_DST: 2 },
    GPUTextureUsage: { STORAGE_BINDING: 1, TEXTURE_BINDING: 2, COPY_SRC: 4 },
  });
  const textures: any[] = [],
    buffers: any[] = [],
    writes: Float32Array[] = [],
    dispatches: number[][] = [];
  let groups = 0;
  const device = {
    createShaderModule: () => ({}),
    createComputePipelineAsync: async () => ({
      getBindGroupLayout: () => ({}),
    }),
    createSampler: () => ({}),
    createBindGroup: () => ({ id: ++groups }),
    createTexture: (description: any) => {
      const result = {
        description,
        destroyed: 0,
        createView: () => ({}),
        destroy() {
          this.destroyed++;
        },
      };
      textures.push(result);
      return result;
    },
    createBuffer: (description: any) => {
      const result = {
        description,
        destroyed: 0,
        destroy() {
          this.destroyed++;
        },
      };
      buffers.push(result);
      return result;
    },
    queue: {
      writeBuffer: (_b: unknown, _o: number, data: Float32Array) =>
        writes.push(data.slice()),
    },
  } as unknown as GPUDevice;
  const { FogAmbientVolume, FOG_AMBIENT_BYTES } =
      await import("../packages/renderer/fog-ambient-volume.ts"),
    owner = await FogAmbientVolume.create(device);
  assert.equal(FOG_AMBIENT_BYTES, 110592);
  assert.equal(owner.bytes, 110624);
  assert.deepEqual(textures[0].description.size, [32, 18, 24]);
  const encoder = {
    beginComputePass: () => ({
      setPipeline() {},
      setBindGroup() {},
      dispatchWorkgroups(...args: number[]) {
        dispatches.push(args);
      },
      end() {},
    }),
  } as unknown as GPUCommandEncoder;
  const input = {
    frameBuffer: {} as GPUBuffer,
    probeRecords: {} as GPUBuffer,
    probeMoments: {} as GPUTextureView,
    enabled: true,
    phaseG: 0.2,
    scatteringIntensity: 1,
    startDistance: 0,
    maxDistance: 60,
  };
  owner.render(encoder, input);
  owner.render(encoder, { ...input, enabled: false });
  assert.equal(groups, 2);
  assert.deepEqual(dispatches, [
    [8, 5, 6],
    [8, 5, 6],
  ]);
  assert.equal(writes[0][4], 1);
  assert.equal(writes[1][4], 0);
  owner.render(encoder, { ...input, probeMoments: {} as GPUTextureView });
  assert.equal(groups, 4);
  assert.throws(() => owner.render(encoder, { ...input, phaseG: 0.8 }));
  assert.throws(() => owner.render(encoder, { ...input, maxDistance: -1 }));
  owner.dispose();
  owner.dispose();
  assert.ok(
    [...textures, ...buffers].every((resource) => resource.destroyed === 1),
  );
  assert.throws(() => owner.render(encoder, input));
});
