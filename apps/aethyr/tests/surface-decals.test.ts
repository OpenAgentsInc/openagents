import assert from "node:assert/strict";
import test from "node:test";
import {
  accumulateSurfaceDecal,
  buildSurfaceDecalDraws,
  packSurfaceDecalDraws,
  packSurfaceDecalMaterials,
  validateSurfaceDecals,
  type SurfaceDecalInstance,
  type SurfaceDecalManifest,
  type SurfaceDecalMaterial,
} from "../packages/renderer/surface-decals.ts";

const material = (): SurfaceDecalMaterial => ({
  id: "owned-material",
  colorOpacityUrl: "/private-assets/owned-color.bin",
  normalUrl: "/private-assets/owned-normal.bin",
  mraUrl: "/private-assets/owned-mra.bin",
  uvTransform: [2, 2, 1, 1],
  normalFlatness: -2,
  normalConstant: [128, 128, 255],
  roughnessIntensity: 1.2,
  sourceSpecularAmount: 1,
  metallic: 0,
});
const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
function volume(id = "owned-volume", x = 0, z = 0.5): SurfaceDecalInstance {
  return {
    id,
    material: 0,
    sourceOrder: 0,
    sortOrder: 0,
    worldFromDecal: [0.2, 0, 0, 0, 0, 0.2, 0, 0, 0, 0, 0.1, 0, x, 0, z, 1],
    worldToDecal: [5, 0, 0, 0, 0, 5, 0, 0, 0, 0, 10, 0, -x * 5, 0, -z * 10, 1],
    bounds: { min: [x - 0.2, -0.2, z - 0.1], max: [x + 0.2, 0.2, z + 0.1] },
    fadeScreenSize: 0.01,
  };
}
const manifest = (): SurfaceDecalManifest => ({
  schema: "aethyrforge.surface-decals.v1",
  materials: [material()],
  instances: [volume()],
});

test("source-over keeps residual transmittance and source draw order", () => {
  const first = accumulateSurfaceDecal([0, 0, 0, 1], [1, 0, 0], 0.25);
  const second = accumulateSurfaceDecal(first, [0, 0, 1], 0.5);
  assert.deepEqual(second, [0.125, 0, 0.5, 0.375]);
  const composed = second
    .slice(0, 3)
    .map((v, i) => v + [0.2, 0.4, 0.6][i] * second[3]);
  assert.deepEqual(composed, [0.2, 0.15000000000000002, 0.725]);
  const reversed = accumulateSurfaceDecal(
    accumulateSurfaceDecal([0, 0, 0, 1], [0, 0, 1], 0.5),
    [1, 0, 0],
    0.25,
  );
  assert.notDeepEqual(reversed, second);
});

test("material storage preserves normal extrapolation and converts specular amount exactly once", () => {
  const values = packSurfaceDecalMaterials(
    [material()],
    (url, role) => ({ color: 3, normal: 5, data: 7 })[role],
  );
  assert.equal(values.byteLength, 64);
  assert.deepEqual([...values.slice(0, 4)], [2, 2, 1, 1]);
  assert.equal(values[4], -2);
  assert.ok(Math.abs(values[5] - 1.2) < 1e-6);
  assert.ok(Math.abs(values[6] - 0.08) < 1e-7);
  assert.deepEqual([...values.slice(8, 16)], [128, 128, 255, 1, 3, 5, 7, 0]);
  const plain = material();
  delete plain.normalUrl;
  delete plain.mraUrl;
  assert.deepEqual(
    [...packSurfaceDecalMaterials([plain], () => 2).slice(8, 16)],
    [128, 128, 255, 0, 2, 0, 0, 0],
  );
});

test("camera culling bounds raster work without reordering overlapping materials", () => {
  const a = volume("a"),
    b = volume("b"),
    c = volume("c"),
    off = volume("off", 3);
  a.sourceOrder = 2;
  a.material = 1;
  b.sourceOrder = 0;
  c.sourceOrder = 1;
  c.material = 1;
  const draws = buildSurfaceDecalDraws([a, off, c, b], identity, 100, 100);
  assert.deepEqual(
    draws.map((d) => d.instance.id),
    ["b", "c", "a"],
  );
  assert.deepEqual(draws[0].scissor, [39, 39, 22, 22]);
  const packed = packSurfaceDecalDraws(draws, 100, 100);
  assert.equal(packed.byteLength, 3 * 160);
  assert.deepEqual([packed[36], packed[76], packed[116]], [0, 1, 1]);
});

test("near-plane intersections stay conservative and fully clipped volumes disappear", () => {
  const crossing = volume("near", 0, 0.05);
  const behind = volume("behind", 0, -0.5);
  const far = volume("far", 0, 1.5);
  crossing.sourceOrder = 0;
  behind.sourceOrder = 1;
  far.sourceOrder = 2;
  const draws = buildSurfaceDecalDraws(
    [crossing, behind, far],
    identity,
    80,
    60,
  );
  assert.deepEqual(
    draws.map((d) => d.instance.id),
    ["near"],
  );
  assert.deepEqual(draws[0].scissor, [0, 0, 80, 60]);
});

test("source volumes require finite mutually inverse transforms and real bounds", () => {
  validateSurfaceDecals(manifest());
  const corrupt = manifest();
  corrupt.instances[0].worldToDecal = identity;
  assert.throws(() => validateSurfaceDecals(corrupt), /inverses/);
  const bounds = manifest();
  bounds.instances[0].bounds.min = [0, 0, 0];
  assert.throws(() => validateSurfaceDecals(bounds), /contain/);
  const invalid = manifest();
  invalid.materials[0].normalFlatness = Number.NaN;
  assert.throws(() => validateSurfaceDecals(invalid), /parameters/);
  const shear = manifest();
  shear.instances[0].worldFromDecal = [...shear.instances[0].worldFromDecal];
  shear.instances[0].worldToDecal = [...shear.instances[0].worldToDecal];
  (shear.instances[0].worldFromDecal as number[])[4] = 0.1;
  (shear.instances[0].worldToDecal as number[])[4] = -2.5;
  shear.instances[0].bounds.min = [-0.3, -0.2, 0.4];
  shear.instances[0].bounds.max = [0.3, 0.2, 0.6];
  assert.throws(() => validateSurfaceDecals(shear), /without shear/);
});

test("private map boundaries and unique source orders are checked before upload", () => {
  const escaped = manifest();
  escaped.materials[0].colorOpacityUrl = "/private-assets/../secret";
  assert.throws(() => validateSurfaceDecals(escaped), /maps/);
  const duplicate = manifest();
  duplicate.instances = [volume("a"), volume("b")];
  assert.throws(() => validateSurfaceDecals(duplicate), /unique order/);
});
