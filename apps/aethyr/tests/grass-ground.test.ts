import assert from "node:assert/strict";
import test from "node:test";
import type { CompiledMaterial } from "../packages/assets/src/scene.ts";
import {
  grassGroundBase,
  grassGroundMarker,
} from "../packages/renderer/grass-ground.ts";
import { Renderer } from "../packages/renderer/renderer.ts";
import { buildRendererShaders } from "../packages/renderer/shaders.ts";
import { MaterialEditor } from "../packages/renderer/material-editor.ts";

test("grass ground admits only its fixed profile and leaves missing profiles disabled", () => {
  assert.equal(grassGroundMarker({}), 0);
  assert.equal(
    grassGroundMarker({ grassGround: { profile: "quick-grass-v1" } }),
    1,
  );
  for (const grassGround of [
    null,
    true,
    [],
    {},
    { profile: "other" },
    { profile: "quick-grass-v1", distance: 10 },
  ])
    assert.throws(
      () => grassGroundMarker({ grassGround } as CompiledMaterial),
      /supported grass ground/,
    );
});

test("ground stays green through blade LOD and the horizon without a bright olive ring", () => {
  const close = (actual: number[], expected: number[]) =>
    actual.forEach((value, i) =>
      assert.ok(Math.abs(value - expected[i]) < 1e-9),
    );
  close(grassGroundBase(0), [0.0125, 0.05, 0.0025]);
  close(grassGroundBase(25), [0.0125, 0.05, 0.0025]);
  close(grassGroundBase(100), [0.0225, 0.09, 0.0045]);
  close(grassGroundBase(2048), [0.0225, 0.09, 0.0045]);
  const near = grassGroundBase(0);
  for (let distance = 0; distance <= 2048; distance += 0.5) {
    const color = grassGroundBase(distance);
    // Density compensation must not desaturate the ground or brighten it fourfold.
    assert.ok(color[1] <= near[1] * 1.8 + 1e-9);
    assert.ok(Math.abs(color[0] / color[1] - near[0] / near[1]) < 1e-9);
    assert.ok(Math.abs(color[2] / color[1] - near[2] / near[1]) < 1e-9);
    const next = grassGroundBase(distance + 0.5);
    assert.ok(next[1] >= color[1]);
    assert.ok(next[1] - color[1] < 0.0005);
  }
  for (const invalid of [NaN, Infinity, -1])
    assert.throws(() => grassGroundBase(invalid));
  for (const boundary of [25, 50, 100]) {
    const a = grassGroundBase(boundary - 0.001),
      b = grassGroundBase(boundary + 0.001);
    assert.ok(a.every((value, i) => Math.abs(value - b[i]) < 0.0001));
  }
});

test("two-triangle ground evaluates interpolated world distance in fragment shading", () => {
  const { MATERIAL } = buildRendererShaders({});
  const vertex = MATERIAL.slice(
    MATERIAL.indexOf("fn vertexValue("),
    MATERIAL.indexOf("@vertex fn"),
  );
  assert.doesNotMatch(vertex, /quickGrassGroundBase\(/);
  const fragment = MATERIAL.slice(
    MATERIAL.indexOf("fn shade("),
    MATERIAL.indexOf("@fragment fn fs("),
  );
  assert.match(MATERIAL, /let surfaceDistance=distance\(frame.eye.xyz,world\)/);
  assert.match(
    MATERIAL,
    /mix\(0.25,0.45,smoothstep\(25.0,100.0,surfaceDistance\)\)/,
  );
  assert.doesNotMatch(MATERIAL, /vec3\(0.3,0.3,0.1\)/);
  assert.match(
    fragment,
    /if material.creator.y>0.5\s*\{base=quickGrassGroundBase\(v.world\)\*material.base.rgb;\}/,
  );
  assert.ok(
    fragment.indexOf("quickGrassGroundBase(v.world)") <
      fragment.indexOf("let creatorColor="),
  );
  assert.ok(
    fragment.indexOf("quickGrassGroundBase(v.world)") <
      fragment.indexOf("let vis=shadowVisibility"),
  );
  assert.doesNotMatch(fragment, /surfaceEmission\s*=\s*quickGrassGround/);
});

test("actual material packing preserves creator flags and previews preserve the new tail lane", async () => {
  const saved = Object.getOwnPropertyDescriptor(globalThis, "GPUBufferUsage");
  Object.defineProperty(globalThis, "GPUBufferUsage", {
    configurable: true,
    value: { UNIFORM: 1, COPY_DST: 2 },
  });
  try {
    const material: CompiledMaterial = {
      id: "ground",
      name: "Ground",
      baseColorFactor: [1, 1, 1, 1],
      emissiveFactor: [0, 0, 0],
      roughness: 1,
      metallic: 0,
      normalScale: 1,
      uvScale: [1, 1],
      alphaMode: "OPAQUE",
      alphaCutoff: 0.5,
      doubleSided: false,
      packedChannels: "metallic-r-roughness-g-occlusion-b",
      grassGround: { profile: "quick-grass-v1" },
    };
    const uniforms: Float32Array[] = [];
    const renderer = Object.create(Renderer.prototype) as any;
    const texture = { createView: () => ({}) };
    Object.assign(renderer, {
      features: {},
      device: { createBindGroup: () => ({}) },
      layerPools: {
        forMaterial: () =>
          Object.fromEntries(
            ["color", "normal", "data"].map((role) => [
              role,
              { index: () => 0, texture },
            ]),
          ),
      },
      texture: async () => texture,
      sampler: {},
      materialLayout: {},
      materialFeatures: new WeakMap(),
      prepareStockPipelines: async () => {},
      buffer: (data: Float32Array) => {
        uniforms.push(data.slice());
        return {};
      },
    });
    await renderer.makeMaterial(material);
    await renderer.makeMaterial(material, 0, { texture, uniform: () => {} });
    await renderer.makeMaterial({ ...material, grassGround: undefined });
    const [ground, creator, plain] = uniforms;
    assert.deepEqual(Array.from(ground.slice(-4)), [0, 1, 0, 0]);
    assert.deepEqual(Array.from(creator.slice(-4)), [1, 1, 0, 0]);
    assert.deepEqual(Array.from(plain.slice(-4)), [0, 0, 0, 0]);
    assert.equal(ground.byteLength, plain.byteLength);
    assert.deepEqual(ground.slice(0, -4), plain.slice(0, -4));
    const writes: Float32Array[] = [];
    const editor = new MaterialEditor({
      device: {
        queue: {
          writeBuffer: (
            _buffer: GPUBuffer,
            _offset: number,
            data: Float32Array,
          ) => writes.push(data.slice()),
        },
      },
    } as any);
    editor.track(material.id, {} as GPUBuffer, ground);
    editor.applyValues({ ground: { red: 0.5, roughness: 0.8 } });
    editor.applyValues({});
    assert.equal(writes[0][0], 0.5);
    assert.equal(writes[1][0], 1);
    for (const write of writes)
      assert.deepEqual(Array.from(write.slice(-4)), [0, 1, 0, 0]);
    editor.dispose();
  } finally {
    if (saved) Object.defineProperty(globalThis, "GPUBufferUsage", saved);
    else delete (globalThis as any).GPUBufferUsage;
  }
});
