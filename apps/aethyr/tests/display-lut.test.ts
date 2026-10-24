import assert from "node:assert/strict";
import { test } from "node:test";
import {
  DisplayLut,
  DISPLAY_LUT_MAX_BYTES,
  decodeDisplayLut,
  displayLutGridInput,
  displayLutParams,
  displayLutShaper,
  sampleDisplayLut,
  validateDisplayLut,
  verifyDisplayLut,
} from "../packages/renderer/display-lut.ts";
import {
  fixtureBytes,
  fixtureDescriptor,
  fixtureHalf,
  fixtureHash,
} from "./display-lut-fixtures.ts";

const close = (
  actual: readonly number[],
  expected: readonly number[],
  epsilon = 1e-6,
) =>
  actual.forEach((v, i) =>
    assert.ok(Math.abs(v - expected[i]) <= epsilon, `${actual} != ${expected}`),
  );

test("v2 admits bounded parametric shapers while v1 retains its exact original domain", () => {
  const d = {
    ...fixtureDescriptor(63),
    schema: "aethyrforge.display-lut.v2" as const,
    shaper: {
      kind: "log2-1p" as const,
      a: 0.002667719095256729,
      max: 43.70524193759099,
    },
  };
  assert.deepEqual(validateDisplayLut(d), d);
  assert.throws(
    () => validateDisplayLut({ ...d, schema: "aethyrforge.display-lut.v1" }),
    /invalid/,
  );
  for (const shaper of [
    { ...d.shaper, a: 2 ** -21 },
    { ...d.shaper, a: 1.01 },
    { ...d.shaper, a: NaN },
    { ...d.shaper, max: 0.99 },
    { ...d.shaper, max: 65505 },
    { ...d.shaper, max: Infinity },
  ])
    assert.throws(() => validateDisplayLut({ ...d, shaper }), /invalid/);
  assert.equal(displayLutGridInput(62, d), d.shaper.max);
  assert.equal(displayLutShaper(d.shaper.max, d), 1);
  assert.equal(displayLutShaper(Infinity, d), 1);
  assert.equal(displayLutShaper(-1, d), 0);
  for (let i = 0; i < d.edge; i++)
    assert.ok(
      Math.abs(displayLutShaper(displayLutGridInput(i, d), d) - i / 62) < 1e-12,
    );
  const params = displayLutParams(d);
  assert.equal(params[1], Math.fround(d.shaper.a));
  assert.equal(params[2], Math.fround(d.shaper.max));
  const values = decodeDisplayLut(
    d,
    fixtureBytes((_c, grid) => grid, d.edge, d),
  );
  for (const color of [
    [0, d.shaper.max, d.shaper.max * 2],
    [0.18, 1, 10],
  ] as const)
    close(
      sampleDisplayLut(d, values, color),
      color.map((v) => displayLutShaper(v, d)),
      0.0005,
    );
});

test("descriptor bounds fix spaces, shaper, orientation and exact byte size", () => {
  const d = fixtureDescriptor();
  assert.deepEqual(validateDisplayLut(d), d);
  assert.notEqual(validateDisplayLut(d).shaper, d.shaper);
  assert.equal(fixtureDescriptor(65).byteLength, DISPLAY_LUT_MAX_BYTES);
  for (const patch of [
    { edge: 32 },
    { edge: 66 },
    { edge: 33.5 },
    { inputSpace: "srgb" },
    { outputSpace: "srgb" },
    { exposure: "internal" },
    { order: "b-fastest" },
    { format: "rgba32float-le" },
    { byteLength: d.byteLength + 1 },
    { shaper: { ...d.shaper, a: 1 } },
    { sha256: "A".repeat(64) },
    { id: "bad id" },
    { url: "file:///private.bin" },
    { url: "javascript:alert(1)" },
  ])
    assert.throws(() => validateDisplayLut({ ...d, ...patch }), /invalid/);
});

test("positive domain and texel centers preserve endpoints without hidden exposure", () => {
  assert.equal(displayLutShaper(0), 0);
  assert.equal(displayLutShaper(-1), 0);
  assert.equal(displayLutShaper(NaN), 0);
  assert.equal(displayLutShaper(-Infinity), 0);
  assert.equal(displayLutShaper(Infinity), 1);
  assert.equal(displayLutShaper(64), 1);
  for (let i = 0; i < 33; i++)
    assert.ok(
      Math.abs(displayLutShaper(displayLutGridInput(i, 33)) - i / 32) < 1e-12,
    );
  assert.equal(displayLutParams()[3], 0);
  assert.equal(displayLutParams(fixtureDescriptor(), false)[3], 0);
});

test("binary16 response preserves signed values and rejects nonfinite or corrupt alpha", async () => {
  const bytes = fixtureBytes(() => [-0.25, 1.5, 65504]);
  const d = { ...fixtureDescriptor(), sha256: await fixtureHash(bytes) };
  const verified = await verifyDisplayLut(d, bytes);
  close(
    sampleDisplayLut(d, verified.values, [0.18, 100, -10]),
    [-0.25, 1.5, 65504],
  );
  await assert.rejects(
    () => verifyDisplayLut({ ...d, sha256: "0".repeat(64) }, bytes),
    /hash/,
  );
  assert.throws(() => decodeDisplayLut(d, bytes.subarray(8)), /byte count/);
  const bad = bytes.slice();
  new DataView(bad.buffer).setUint16(0, 0x7c00, true);
  assert.throws(() => decodeDisplayLut(d, bad), /nonfinite/);
  new DataView(bad.buffer).setUint16(0, fixtureHalf(0), true);
  new DataView(bad.buffer).setUint16(6, fixtureHalf(0), true);
  assert.throws(() => decodeDisplayLut(d, bad), /alpha/);
});

test("trilinear log ramps and cross-channel response use RGB x-fastest orientation", () => {
  const d = fixtureDescriptor();
  const values = decodeDisplayLut(
    d,
    fixtureBytes((_input, [r, g, b]) => [r, 2 * g - 0.5, 4 * b + r]),
  );
  for (const color of [
    [0, 0, 0],
    [64, 0, 0],
    [0, 64, 0],
    [0, 0, 64],
    [0.18, 1.5, 7],
    [Infinity, NaN, -Infinity],
  ] as const) {
    const [r, g, b] = color.map((value) => displayLutShaper(value));
    close(sampleDisplayLut(d, values, color), [r, 2 * g - 0.5, 4 * b + r]);
  }
  const identity = decodeDisplayLut(
    d,
    fixtureBytes((color) => color),
  );
  for (const index of [0, 3, 16, 24, 32]) {
    const value = displayLutGridInput(index, 33);
    close(
      sampleDisplayLut(d, identity, [value, value, value]),
      [value, value, value],
      Math.max(1e-7, value * 0.0005),
    );
  }
});

test("failed partial GPU construction frees the texture; invalid bytes allocate nothing", async () => {
  const previous = Object.getOwnPropertyDescriptor(
    globalThis,
    "GPUTextureUsage",
  );
  Object.defineProperty(globalThis, "GPUTextureUsage", {
    value: { TEXTURE_BINDING: 4, COPY_DST: 2 },
    configurable: true,
  });
  try {
    const bytes = fixtureBytes(() => [0, 0, 0]),
      d = { ...fixtureDescriptor(), sha256: await fixtureHash(bytes) };
    let allocations = 0,
      destroyed = 0;
    const device = {
      limits: { maxTextureDimension3D: 256 },
      pushErrorScope() {},
      popErrorScope: async () => null,
      queue: { writeTexture() {} },
      createSampler: () => ({}),
      createTexture: () => {
        allocations++;
        return {
          createView: () => {
            throw Error("Owned fixture allocation failure");
          },
          destroy: () => destroyed++,
        };
      },
    } as unknown as GPUDevice;
    await assert.rejects(
      () => DisplayLut.create(device, d, bytes),
      /allocation failure/,
    );
    assert.equal(allocations, 1);
    assert.equal(destroyed, 1);
    const gpuFailure = {
      ...device,
      createTexture: () => {
        allocations++;
        return { createView: () => ({}), destroy: () => destroyed++ };
      },
      popErrorScope: async () => ({
        message: "Owned fixture GPU validation failure",
      }),
    } as unknown as GPUDevice;
    await assert.rejects(
      () => DisplayLut.create(gpuFailure, d, bytes),
      /GPU validation failure/,
    );
    assert.equal(allocations, 2);
    assert.equal(destroyed, 2);
    await assert.rejects(
      () => DisplayLut.create(device, { ...d, sha256: "0".repeat(64) }, bytes),
      /hash/,
    );
    assert.equal(allocations, 2);
  } finally {
    if (previous)
      Object.defineProperty(globalThis, "GPUTextureUsage", previous);
    else Reflect.deleteProperty(globalThis, "GPUTextureUsage");
  }
});
