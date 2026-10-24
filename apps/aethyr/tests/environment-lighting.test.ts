import assert from "node:assert/strict";
import { test } from "node:test";
import {
  decodeRgba16fLE,
  encodeRgba16fLE,
  environmentLightingTransform,
  evaluateDiffuseSH9,
  halfToFloat,
  integrateBrdfLut,
  prefilterSpecular,
  projectDiffuseSH9,
  projectRadianceSH9,
  replaceLowerHemisphere,
  type EnvironmentImage,
} from "../packages/renderer/environment-lighting.ts";
import { modelMatrix, normalize, type V3 } from "../packages/renderer/math.ts";

function image(color: V3, width = 32, height = 16): EnvironmentImage {
  const pixels = new Float32Array(width * height * 4);
  for (let i = 0; i < width * height; i++) pixels.set([...color, 1], i * 4);
  return { width, height, pixels };
}
function close(actual: number, expected: number, tolerance = 0.001): void {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `${actual} differs from ${expected}`,
  );
}

test("half-float files have explicit little-endian encoding and reject invalid HDR radiance", () => {
  const pixels = new Float32Array([0, 1, 0.5, 65504, 2 ** -24, 2 ** -14, 2, 1]);
  const encoded = encodeRgba16fLE(pixels);
  assert.deepEqual([...encoded.slice(0, 8)], [0, 0, 0, 60, 0, 56, 255, 123]);
  const decoded = decodeRgba16fLE(encoded, 2, 1);
  for (let i = 0; i < pixels.length; i++)
    close(halfToFloat(decoded.pixels[i]), pixels[i], 0);
  assert.equal(halfToFloat(0x8000), -0);
  assert.throws(() => encodeRgba16fLE(new Float32Array([0, -1, 1, 1])));
  assert.throws(() => encodeRgba16fLE(new Float32Array([0, Number.NaN, 1, 1])));
  assert.throws(() =>
    decodeRgba16fLE(new Uint8Array([0, 124, 0, 0, 0, 0, 0, 60]), 1, 1),
  );
  assert.throws(() => decodeRgba16fLE(encoded, 2, 2));
});

test("constant sky SH evaluates to constant Lambert E/pi in every direction", () => {
  const color: V3 = [0.2, 1, 4],
    coefficients = projectDiffuseSH9(image(color));
  assert.equal(coefficients.length, 36);
  for (let i = 0; i < 9; i++) assert.equal(coefficients[i * 4 + 3], 0);
  for (const normal of [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [1, 1, 1],
  ] as V3[]) {
    const result = evaluateDiffuseSH9(coefficients, normal);
    result.forEach((value, channel) => close(value, color[channel], 0.001));
  }
  assert.deepEqual([...projectDiffuseSH9(image([0, 0, 0]))], Array(36).fill(0));
});

test("directional first-band radiance obeys analytic Lambert convolution and supports generated skies", () => {
  const coefficients = projectRadianceSH9(([x, y, z]) => [
    1 + 0.5 * x,
    1 + 0.5 * y,
    1 + 0.5 * z,
  ]);
  for (const normal of [
    [1, 0, 0],
    [0, 1, 0],
    [0, 0, 1],
    [-1, -1, -1],
  ] as V3[]) {
    const n = normalize(normal),
      result = evaluateDiffuseSH9(coefficients, n);
    result.forEach((value, channel) => close(value, 1 + n[channel] / 3, 0.001));
  }
});

test("lighting direction transform follows the displayed panorama rotation, flips, and longitude offset", () => {
  const rotation = modelMatrix([0, 0, 0], 0.72);
  for (const uSign of [-1, 1])
    for (const vSign of [-1, 1]) {
      const uOffset = 0.23,
        vOffset = vSign === -1 ? 1 : 0;
      const matrix = environmentLightingTransform({
        rotation,
        uSign,
        uOffset,
        vSign,
        vOffset,
      });
      for (const direction of [
        [1, 0, 0],
        [0.1, 1, 0.2],
        [-0.3, -0.4, 0.8],
      ] as V3[]) {
        const n = normalize(direction);
        const local = [0, 1, 2].map(
          (row) =>
            rotation[row] * n[0] +
            rotation[4 + row] * n[1] +
            rotation[8 + row] * n[2],
        );
        const theta =
          ((Math.atan2(local[0], local[2]) * uSign) / (2 * Math.PI) + uOffset) *
          2 *
          Math.PI;
        const phi =
          Math.acos(Math.min(1, Math.max(-1, local[1]))) * vSign +
          vOffset * Math.PI;
        const expected = [
          Math.sin(phi) * Math.sin(theta),
          Math.cos(phi),
          Math.sin(phi) * Math.cos(theta),
        ];
        const actual = [0, 1, 2].map(
          (row) =>
            matrix[row] * n[0] +
            matrix[4 + row] * n[1] +
            matrix[8 + row] * n[2],
        );
        actual.forEach((value, i) => close(value, expected[i], 1e-6));
      }
    }
  assert.throws(() =>
    environmentLightingTransform({
      rotation,
      uSign: 0,
      uOffset: 0,
      vSign: 1,
      vOffset: 0,
    }),
  );
  assert.throws(() =>
    environmentLightingTransform({
      rotation,
      uSign: 1,
      uOffset: 0,
      vSign: 1,
      vOffset: Number.NaN,
    }),
  );
});

test("GGX prefilter preserves constant HDR energy, finite pixels, and broad directional final levels", () => {
  const levels = prefilterSpecular(image([2, 0.5, 1]), 32, 32);
  assert.deepEqual(
    levels.map(({ width, height, roughness }) => [width, height, roughness]),
    [
      [32, 16, 0],
      [16, 8, 0.5],
      [8, 4, 1],
    ],
  );
  for (const level of levels)
    for (let i = 0; i < level.pixels.length; i += 4) {
      close(level.pixels[i], 2, 1e-6);
      close(level.pixels[i + 1], 0.5, 1e-6);
      close(level.pixels[i + 2], 1, 1e-6);
      assert.equal(level.pixels[i + 3], 1);
    }
  const source = image([0.1, 0.1, 0.1]);
  for (let y = 0; y < source.height; y++)
    for (let x = 0; x < source.width; x++) {
      const theta = ((x + 0.5) / source.width) * Math.PI * 2,
        phi = ((y + 0.5) / source.height) * Math.PI;
      const radiance =
        0.1 + 10 * Math.max(0, Math.sin(phi) * Math.cos(theta)) ** 32;
      source.pixels.set(
        [radiance, radiance, radiance, 1],
        (y * source.width + x) * 4,
      );
    }
  const filtered = prefilterSpecular(source, 32, 128),
    maxima = filtered.map((level) =>
      Math.max(...level.pixels.filter((_, i) => i % 4 === 0)),
    );
  assert.ok(
    maxima.at(-1)! < maxima[0] * 0.4,
    "Roughness should spread the bright lobe rather than box-downsample it.",
  );
  assert.deepEqual(
    prefilterSpecular(source, 16, 32),
    prefilterSpecular(source, 16, 32),
  );
});

test("split-sum LUT is deterministic, finite, and approaches mirror Fresnel for smooth surfaces", () => {
  const lut = integrateBrdfLut(16, 256);
  assert.equal(lut.pixels.length, 16 * 16 * 4);
  assert.ok(lut.pixels.every((value) => Number.isFinite(value) && value >= 0));
  for (let i = 0; i < lut.pixels.length; i += 4) {
    assert.ok(lut.pixels[i] + lut.pixels[i + 1] <= 1.04);
    assert.equal(lut.pixels[i + 2], 0);
    assert.equal(lut.pixels[i + 3], 1);
  }
  const noV = 15.5 / 16,
    index = 15 * 4,
    fresnel = (1 - noV) ** 5;
  close(lut.pixels[index], 1 - fresnel, 0.005);
  close(lut.pixels[index + 1], fresnel, 0.005);
  assert.deepEqual(integrateBrdfLut(16, 16), integrateBrdfLut(16, 16));
});

test("invalid radiance and oversized integration jobs fail before allocating filtered outputs", () => {
  const malformed = image([1, 1, 1]);
  malformed.pixels[100] = Number.NaN;
  assert.throws(() => projectDiffuseSH9(malformed));
  assert.throws(() => prefilterSpecular(malformed, 16, 16));
  assert.throws(() => projectDiffuseSH9(image([1, 1, 1]), 512));
  assert.throws(() => projectRadianceSH9(() => [-1, 0, 0]));
  assert.throws(() => prefilterSpecular(image([1, 1, 1]), 256, 512));
  assert.throws(() => prefilterSpecular(image([1, 1, 1]), 512, 16));
  assert.throws(() => integrateBrdfLut(128, 512));
  assert.throws(() => evaluateDiffuseSH9(new Float32Array(36), [0, 0, 0]));
});

test("lower-hemisphere lighting replacement follows world up and leaves the visible sky intact", () => {
  const source = image([1, 1, 1], 64, 32),
    original = source.pixels.slice();
  const identity = new Float32Array(modelMatrix([0, 0, 0]));
  const masked = replaceLowerHemisphere(source, identity);
  assert.deepEqual(source.pixels, original);
  const coefficients = projectDiffuseSH9(masked);
  close(evaluateDiffuseSH9(coefficients, [0, 1, 0])[0], 1, 0.003);
  close(evaluateDiffuseSH9(coefficients, [0, -1, 0])[0], 0, 0.003);
  close(evaluateDiffuseSH9(coefficients, [1, 0, 0])[0], 0.5, 0.003);
  const rotated = new Float32Array([
    0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
  ]);
  const rotatedCoefficients = projectDiffuseSH9(
    replaceLowerHemisphere(source, rotated),
  );
  close(evaluateDiffuseSH9(rotatedCoefficients, [-1, 0, 0])[0], 1, 0.003);
  close(evaluateDiffuseSH9(rotatedCoefficients, [1, 0, 0])[0], 0, 0.003);
  const half = decodeRgba16fLE(
    encodeRgba16fLE(source.pixels as Float32Array),
    source.width,
    source.height,
  );
  const colored = replaceLowerHemisphere(half, identity, [0.25, 0.5, 0.75]);
  const bottom = (source.width * source.height - 1) * 4;
  close(halfToFloat(colored.pixels[bottom]), 0.25, 0);
  close(halfToFloat(colored.pixels[bottom + 2]), 0.75, 0);
});
