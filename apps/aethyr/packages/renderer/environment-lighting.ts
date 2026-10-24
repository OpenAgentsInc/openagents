import type { EnvironmentMap } from "../assets/src/scene.ts";
import { cross, dot, inverse, normalize, type V3 } from "./math.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";

/** Independent distant-light preprocessing; this module performs no GPU or network work.
 * SH convolution: https://graphics.stanford.edu/papers/envmap/
 * Split-sum model: https://google.github.io/filament/main/filament.html#lighting/imagebasedlights
 */
export const ENVIRONMENT_LIGHTING_PROFILE = "sh9-ggx-split-sum-v1";
/** Compatibility name for the common scene storage scale; it is not sky-only exposure. */
export const SOURCE_SKY_PREVIEW_SCALE = SCENE_LINEAR_SCALE;
export const SH9_BASIS = [
  "1",
  "y",
  "z",
  "x",
  "xy",
  "yz",
  "3z2-1",
  "xz",
  "x2-y2",
] as const;
const MAX_SOURCE_PIXELS = 4096 * 2048;
const MAX_SAMPLE_EVALUATIONS = 2_000_000;

export interface EnvironmentImage {
  width: number;
  height: number;
  /** Linear RGBA; Uint16 values contain IEEE binary16 bits. Alpha is ignored. */
  pixels: Uint16Array | Float32Array;
}
export interface SpecularLevel {
  width: number;
  height: number;
  /** Perceptual roughness; the GGX alpha parameter is roughness squared. */
  roughness: number;
  pixels: Float32Array;
}
export interface BrdfLut {
  width: number;
  height: number;
  /** RG holds A/B for prefilteredRadiance * (F0 * A + F90 * B); BA is 0/1. */
  pixels: Float32Array;
}

function boundedPowerOfTwo(
  value: number,
  min: number,
  max: number,
  name: string,
): void {
  if (
    !Number.isInteger(value) ||
    value < min ||
    value > max ||
    (value & (value - 1)) !== 0
  )
    throw new Error(`${name} must be a power of two from ${min} to ${max}.`);
}
function validateImage(image: EnvironmentImage): void {
  const { width, height, pixels } = image;
  if (
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > 4096 ||
    height > 2048 ||
    width * height > MAX_SOURCE_PIXELS
  )
    throw new Error("Use an environment image up to 4096 by 2048 pixels.");
  if (
    !(pixels instanceof Uint16Array || pixels instanceof Float32Array) ||
    pixels.length !== width * height * 4
  )
    throw new Error("The environment image needs complete linear RGBA data.");
  for (let i = 0; i < pixels.length; i++) {
    if (i % 4 === 3) continue;
    if (pixels instanceof Uint16Array) {
      if (pixels[i] > 0x7bff && pixels[i] !== 0x8000)
        throw new Error("Environment radiance must be finite and nonnegative.");
    } else checkedRadiance(pixels[i]);
  }
}
function checkedRadiance(value: number): number {
  if (!Number.isFinite(value) || value < 0 || value > 65504)
    throw new Error(
      "Environment radiance must be finite and between 0 and 65504.",
    );
  return value;
}
export function halfToFloat(bits: number): number {
  const sign = bits & 0x8000 ? -1 : 1;
  const exponent = (bits >>> 10) & 31;
  const fraction = bits & 1023;
  if (exponent === 31)
    return fraction ? Number.NaN : sign * Number.POSITIVE_INFINITY;
  return exponent === 0
    ? sign * fraction * 2 ** -24
    : sign * (1 + fraction / 1024) * 2 ** (exponent - 15);
}
function roundEven(value: number): number {
  const floor = Math.floor(value),
    delta = value - floor;
  return floor + Number(delta > 0.5 || (delta === 0.5 && floor % 2 === 1));
}
function radianceToHalf(value: number): number {
  checkedRadiance(value);
  if (value === 0) return 0;
  if (value < 2 ** -14) return roundEven(value * 2 ** 24);
  let exponent = Math.floor(Math.log2(value));
  let fraction = roundEven((value / 2 ** exponent - 1) * 1024);
  if (fraction === 1024) {
    exponent++;
    fraction = 0;
  }
  return ((exponent + 15) << 10) | fraction;
}
export function encodeRgba16fLE(pixels: Float32Array): Uint8Array {
  if (pixels.length % 4 !== 0 || pixels.length > MAX_SOURCE_PIXELS * 4)
    throw new Error("Use a bounded RGBA image for half-float encoding.");
  const bytes = new Uint8Array(pixels.length * 2),
    view = new DataView(bytes.buffer);
  pixels.forEach((value, i) =>
    view.setUint16(i * 2, radianceToHalf(value), true),
  );
  return bytes;
}
export function decodeRgba16fLE(
  bytes: Uint8Array,
  width: number,
  height: number,
): EnvironmentImage {
  if (
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > 4096 ||
    height > 2048 ||
    width * height > MAX_SOURCE_PIXELS ||
    bytes.byteLength !== width * height * 8
  )
    throw new Error(
      "The half-float environment image has an unsupported size or incomplete data.",
    );
  const pixels = new Uint16Array(width * height * 4),
    view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  for (let i = 0; i < pixels.length; i++)
    pixels[i] = view.getUint16(i * 2, true);
  // Validate every RGB input once, before sampling or allocating output maps.
  for (let i = 0; i < pixels.length; i++)
    if (i % 4 !== 3) checkedRadiance(halfToFloat(pixels[i]));
  return { width, height, pixels };
}
function readChannel(
  image: EnvironmentImage,
  pixel: number,
  channel: number,
): number {
  const value = image.pixels[pixel * 4 + channel];
  return image.pixels instanceof Uint16Array ? HALF_VALUES[value] : value;
}
const HALF_VALUES = Float32Array.from({ length: 65536 }, (_, bits) =>
  halfToFloat(bits),
);
function sampleUV(image: EnvironmentImage, u: number, v: number): V3 {
  const x = (u - Math.floor(u)) * image.width - 0.5;
  const y = Math.min(image.height - 1, Math.max(0, v * image.height - 0.5));
  const floorX = Math.floor(x),
    floorY = Math.floor(y);
  const x0 = ((floorX % image.width) + image.width) % image.width,
    x1 = (x0 + 1) % image.width;
  const y1 = Math.min(image.height - 1, floorY + 1),
    fx = x - floorX,
    fy = y - floorY;
  const result: V3 = [0, 0, 0];
  for (let channel = 0; channel < 3; channel++) {
    const a = readChannel(image, floorY * image.width + x0, channel),
      b = readChannel(image, floorY * image.width + x1, channel);
    const c = readChannel(image, y1 * image.width + x0, channel),
      d = readChannel(image, y1 * image.width + x1, channel);
    result[channel] = (a + (b - a) * fx) * (1 - fy) + (c + (d - c) * fx) * fy;
  }
  return result;
}
function panoramaDirection(u: number, v: number): V3 {
  const theta = u * 2 * Math.PI,
    phi = v * Math.PI,
    sinPhi = Math.sin(phi);
  return [sinPhi * Math.sin(theta), Math.cos(phi), sinPhi * Math.cos(theta)];
}
function sampleDirection(image: EnvironmentImage, direction: V3): V3 {
  return sampleUV(
    image,
    Math.atan2(direction[0], direction[2]) / (2 * Math.PI),
    Math.acos(Math.min(1, Math.max(-1, direction[1]))) / Math.PI,
  );
}
export function sh9Basis(direction: V3): number[] {
  const [x, y, z] = direction;
  return [
    0.28209479177387814,
    0.4886025119029199 * y,
    0.4886025119029199 * z,
    0.4886025119029199 * x,
    1.0925484305920792 * x * y,
    1.0925484305920792 * y * z,
    0.31539156525252005 * (3 * z * z - 1),
    1.0925484305920792 * x * z,
    0.5462742152960396 * (x * x - y * y),
  ];
}
/** RGB coefficients padded to vec4. Lambert convolution E/pi is already included. */
export function projectDiffuseSH9(
  image: EnvironmentImage,
  sampleWidth = 128,
): Float32Array {
  validateImage(image);
  return projectRadianceSH9(
    (direction) => sampleDirection(image, direction),
    sampleWidth,
  );
}
/** The caller may supply a trusted generated sky instead of an image. */
export function projectRadianceSH9(
  sample: (direction: V3) => V3,
  sampleWidth = 128,
): Float32Array {
  boundedPowerOfTwo(sampleWidth, 16, 256, "SH integration width");
  const height = sampleWidth / 2,
    coefficients = new Float64Array(36);
  for (let y = 0; y < height; y++) {
    const omega =
      ((2 * Math.PI) / sampleWidth) *
      (Math.cos((y * Math.PI) / height) -
        Math.cos(((y + 1) * Math.PI) / height));
    for (let x = 0; x < sampleWidth; x++) {
      const direction = panoramaDirection(
          (x + 0.5) / sampleWidth,
          (y + 0.5) / height,
        ),
        radiance = sample(direction),
        basis = sh9Basis(direction);
      if (!radiance || radiance.length !== 3)
        throw new Error(
          "Sky sampling must return three linear radiance values.",
        );
      radiance.forEach(checkedRadiance);
      for (let i = 0; i < 9; i++) {
        const lambert = i === 0 ? 1 : i < 4 ? 2 / 3 : 1 / 4;
        for (let channel = 0; channel < 3; channel++)
          coefficients[i * 4 + channel] +=
            radiance[channel] * basis[i] * omega * lambert;
      }
    }
  }
  return Float32Array.from(coefficients);
}
export function evaluateDiffuseSH9(
  coefficients: Float32Array,
  direction: V3,
): V3 {
  if (
    coefficients.length !== 36 ||
    !coefficients.every(Number.isFinite) ||
    !direction.every(Number.isFinite) ||
    Math.hypot(...direction) < 1e-8
  )
    throw new Error(
      "Use nine finite RGB coefficients and a nonzero lighting direction.",
    );
  const basis = sh9Basis(normalize(direction)),
    result: V3 = [0, 0, 0];
  for (let i = 0; i < 9; i++)
    for (let channel = 0; channel < 3; channel++)
      result[channel] += coefficients[i * 4 + channel] * basis[i];
  return result.map((value) => Math.max(0, value)) as V3;
}
/** Matches panorama rotation and reflections. Tiny fitted latitude residuals are
 * snapped to the pole-preserving map; a latitude shift is not a rigid transform.
 */
export function environmentLightingTransform(
  description: Pick<
    EnvironmentMap,
    "rotation" | "uSign" | "uOffset" | "vSign" | "vOffset"
  >,
): Float32Array {
  const { rotation, uSign, uOffset, vSign, vOffset } = description;
  if (
    rotation.length !== 16 ||
    !rotation.every(Number.isFinite) ||
    (uSign !== -1 && uSign !== 1) ||
    (vSign !== -1 && vSign !== 1) ||
    !Number.isFinite(uOffset) ||
    !Number.isFinite(vOffset) ||
    Math.abs(vOffset - (vSign === -1 ? 1 : 0)) > 0.001
  )
    throw new Error(
      "Use a finite sky rotation and a complete equirectangular UV mapping.",
    );
  const angle = (uOffset % 1) * 2 * Math.PI,
    c = Math.cos(angle),
    s = Math.sin(angle),
    output = new Float32Array(16);
  for (let column = 0; column < 3; column++) {
    output[column * 4] =
      uSign * c * rotation[column * 4] + s * rotation[column * 4 + 2];
    output[column * 4 + 1] = vSign * rotation[column * 4 + 1];
    output[column * 4 + 2] =
      -uSign * s * rotation[column * 4] + c * rotation[column * 4 + 2];
  }
  output[15] = 1;
  return output;
}

/** Replaces the world-space lower hemisphere for lighting, without changing the sky image.
 * The replacement color is linear radiance before the caller's tint/brightness gain.
 */
export function replaceLowerHemisphere(
  image: EnvironmentImage,
  worldToEnvironment: Float32Array,
  lowerRadiance: V3 = [0, 0, 0],
): EnvironmentImage {
  validateImage(image);
  if (
    worldToEnvironment.length !== 16 ||
    !worldToEnvironment.every(Number.isFinite)
  )
    throw new Error("Use a finite world-to-environment transform.");
  if (lowerRadiance.length !== 3)
    throw new Error("Use three lower-hemisphere radiance values.");
  lowerRadiance.forEach(checkedRadiance);
  const environmentToWorld = inverse(worldToEnvironment);
  const worldUp = normalize([
    environmentToWorld[1],
    environmentToWorld[5],
    environmentToWorld[9],
  ]);
  const pixels =
    image.pixels instanceof Uint16Array
      ? new Uint16Array(image.pixels)
      : new Float32Array(image.pixels);
  const replacement =
    image.pixels instanceof Uint16Array
      ? lowerRadiance.map(radianceToHalf)
      : lowerRadiance;
  for (let y = 0; y < image.height; y++)
    for (let x = 0; x < image.width; x++) {
      const direction = panoramaDirection(
        (x + 0.5) / image.width,
        (y + 0.5) / image.height,
      );
      if (dot(direction, worldUp) < 0)
        pixels.set(replacement, (y * image.width + x) * 4);
    }
  return { ...image, pixels };
}
function radicalInverse(index: number): number {
  let result = 0,
    fraction = 0.5;
  while (index) {
    result += (index & 1) * fraction;
    index >>>= 1;
    fraction *= 0.5;
  }
  return result;
}
function ggxHalfVectors(roughness: number, count: number): V3[] {
  const alpha2 = roughness ** 4;
  return Array.from({ length: count }, (_, i) => {
    const u = radicalInverse(i),
      phi = (2 * Math.PI * (i + 0.5)) / count;
    const cosTheta = Math.sqrt((1 - u) / (1 + (alpha2 - 1) * u)),
      sinTheta = Math.sqrt(Math.max(0, 1 - cosTheta * cosTheta));
    return [Math.cos(phi) * sinTheta, Math.sin(phi) * sinTheta, cosTheta];
  });
}
/** Offline GGX convolution. Deliberately stops at 8x4 to retain broad directional lighting.
 * This bounded preview profile uses bilinear source samples; very small HDR emitters may
 * require more samples or PDF-aware source filtering before production qualification.
 */
export function prefilterSpecular(
  image: EnvironmentImage,
  baseWidth = 256,
  sampleCount = 128,
): SpecularLevel[] {
  validateImage(image);
  boundedPowerOfTwo(baseWidth, 16, 256, "Specular width");
  boundedPowerOfTwo(sampleCount, 16, 512, "Specular sample count");
  const levels = Math.log2(baseWidth / 8) + 1;
  let evaluations = (baseWidth * baseWidth) / 2;
  for (let level = 1; level < levels; level++)
    evaluations += ((baseWidth >> level) ** 2 / 2) * sampleCount;
  if (evaluations > MAX_SAMPLE_EVALUATIONS)
    throw new Error(
      "The specular filter exceeds its two-million-sample budget.",
    );
  const output: SpecularLevel[] = [];
  for (let level = 0; level < levels; level++) {
    const width = baseWidth >> level,
      height = width / 2,
      roughness = level / (levels - 1),
      pixels = new Float32Array(width * height * 4);
    const samples = level
      ? ggxHalfVectors(roughness, sampleCount)
          .map(
            (h) =>
              [2 * h[2] * h[0], 2 * h[2] * h[1], 2 * h[2] * h[2] - 1] as V3,
          )
          .filter((l) => l[2] > 0)
      : [];
    const weight = samples.reduce((sum, l) => sum + l[2], 0);
    for (let y = 0; y < height; y++)
      for (let x = 0; x < width; x++) {
        const u = (x + 0.5) / width,
          v = (y + 0.5) / height,
          index = (y * width + x) * 4;
        let color: V3;
        if (!level) color = sampleUV(image, u, v);
        else {
          const n = panoramaDirection(u, v),
            tangent = normalize(
              cross(Math.abs(n[2]) < 0.999 ? [0, 0, 1] : [1, 0, 0], n),
            ),
            bitangent = cross(n, tangent);
          color = [0, 0, 0];
          for (const l of samples) {
            const direction = [0, 1, 2].map(
              (axis) =>
                tangent[axis] * l[0] + bitangent[axis] * l[1] + n[axis] * l[2],
            ) as V3;
            const sample = sampleDirection(image, direction);
            for (let channel = 0; channel < 3; channel++)
              color[channel] += (sample[channel] * l[2]) / weight;
          }
        }
        pixels.set([...color, 1], index);
      }
    output.push({ width, height, roughness, pixels });
  }
  return output;
}
/** Height-correlated Smith GGX with independent Schlick A/B basis coefficients.
 * Runtime reflection weights are F0*A + F90*B; authoring F90 does not change this table.
 * Coordinates are x=NoV and y=perceptual roughness, sampled at texel centers.
 */
export function integrateBrdfLut(size = 64, sampleCount = 128): BrdfLut {
  boundedPowerOfTwo(size, 16, 128, "BRDF LUT size");
  boundedPowerOfTwo(sampleCount, 16, 512, "BRDF sample count");
  if (size * size * sampleCount > MAX_SAMPLE_EVALUATIONS)
    throw new Error("The BRDF LUT exceeds its two-million-sample budget.");
  const pixels = new Float32Array(size * size * 4);
  for (let y = 0; y < size; y++) {
    const roughness = (y + 0.5) / size,
      alpha2 = roughness ** 4,
      samples = ggxHalfVectors(roughness, sampleCount);
    for (let x = 0; x < size; x++) {
      const noV = (x + 0.5) / size,
        view: V3 = [Math.sqrt(1 - noV * noV), 0, noV];
      let a = 0,
        b = 0;
      for (const h of samples) {
        const voH = Math.max(0, dot(view, h)),
          noL = 2 * voH * h[2] - noV;
        if (noL <= 0 || voH <= 0) continue;
        const visibility =
          0.5 /
          Math.max(
            noL * Math.sqrt(noV * noV * (1 - alpha2) + alpha2) +
              noV * Math.sqrt(noL * noL * (1 - alpha2) + alpha2),
            1e-12,
          );
        const weight = (4 * visibility * noL * voH) / Math.max(h[2], 1e-12),
          fresnel = (1 - voH) ** 5;
        a += (1 - fresnel) * weight;
        b += fresnel * weight;
      }
      pixels.set([a / sampleCount, b / sampleCount, 0, 1], (y * size + x) * 4);
    }
  }
  return { width: size, height: size, pixels };
}
