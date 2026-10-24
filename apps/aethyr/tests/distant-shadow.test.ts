import assert from "node:assert/strict";
import test from "node:test";
import {
  fitDirectionalShadow,
  shadowProjectionPoint,
  SUN_SHADOW_FITS,
} from "../packages/renderer/distant-shadow.ts";
import {
  add,
  cross,
  dot,
  lookAt,
  multiply,
  normalize,
  orthographic,
  scale,
  type V3,
} from "../packages/renderer/math.ts";

const eye: V3 = [20, 3, 28];
const forward: V3 = [0.6, 0.1, -0.8];
const sun: V3 = [0.2, 0.4, 0.89];

test("source near/far fits retain their previous light matrices", () => {
  const light = normalize(sun),
    horizontal = normalize([forward[0], 0, forward[2]]);
  const right = normalize(cross([0, 1, 0], light)),
    up = cross(light, right);
  for (const options of [SUN_SHADOW_FITS.near, SUN_SHADOW_FITS.far]) {
    let center = add(eye, scale(horizontal, options.ahead));
    const texel = (options.extent * 2) / options.resolution;
    for (const axis of [right, up]) {
      const along = dot(center, axis);
      center = add(
        center,
        scale(axis, Math.round(along / texel) * texel - along),
      );
    }
    const previous = multiply(
      orthographic(
        -options.extent,
        options.extent,
        -options.extent,
        options.extent,
        1,
        500,
      ),
      lookAt(add(center, scale(light, 200)), center),
    );
    const current = fitDirectionalShadow(eye, forward, light, options).matrix;
    assert.ok(
      Math.max(...previous.map((v, i) => Math.abs(v - current[i]))) < 1e-7,
    );
  }
});

test("three bounded light volumes preserve town texels and cover the backdrop", () => {
  for (const options of Object.values(SUN_SHADOW_FITS)) {
    const fit = fitDirectionalShadow(eye, forward, sun, options);
    assert.ok(fit.matrix.every(Number.isFinite));
    assert.equal(fit.worldTexel, (options.extent * 2) / options.resolution);
    assert.ok(shadowProjectionPoint(fit.matrix, fit.center).inside);
    for (const axis of [fit.right, fit.up]) {
      for (const sign of [-1, 1]) {
        const inside = add(
          fit.center,
          scale(axis, sign * options.extent * 0.999),
        );
        const outside = add(
          fit.center,
          scale(axis, sign * options.extent * 1.001),
        );
        assert.ok(shadowProjectionPoint(fit.matrix, inside).inside);
        assert.equal(shadowProjectionPoint(fit.matrix, outside).inside, false);
      }
    }
    for (const distance of [options.near + 0.1, options.far - 0.1])
      assert.ok(
        shadowProjectionPoint(
          fit.matrix,
          add(fit.lightEye, scale(fit.sun, -distance)),
        ).inside,
      );
    for (const distance of [options.near * 0.5, options.far + 1])
      assert.equal(
        shadowProjectionPoint(
          fit.matrix,
          add(fit.lightEye, scale(fit.sun, -distance)),
        ).inside,
        false,
      );
  }
  assert.equal(SUN_SHADOW_FITS.near.resolution, 4096);
  assert.equal(SUN_SHADOW_FITS.far.extent, 90);
  assert.equal(SUN_SHADOW_FITS.far.resolution, 2048);
  const distant = fitDirectionalShadow(
    eye,
    forward,
    sun,
    SUN_SHADOW_FITS.distant,
  );
  const far = fitDirectionalShadow(eye, forward, sun, SUN_SHADOW_FITS.far);
  const backdrop = add(distant.center, scale(distant.right, 500));
  assert.equal(shadowProjectionPoint(far.matrix, backdrop).inside, false);
  assert.equal(shadowProjectionPoint(distant.matrix, backdrop).inside, true);
});

test("zenith/nadir and vertical views stay finite and texel snapping is stable", () => {
  for (const light of [
    [0, 1, 0],
    [0, -1, 0],
  ] as V3[])
    for (const view of [
      [0, 1, 0],
      [0, -1, 0],
    ] as V3[]) {
      const fit = fitDirectionalShadow(
        eye,
        view,
        light,
        SUN_SHADOW_FITS.distant,
      );
      assert.ok(fit.matrix.every(Number.isFinite));
      assert.ok(shadowProjectionPoint(fit.matrix, fit.center).inside);
    }
  const fit = fitDirectionalShadow(
    [0, 0, 0],
    [0, 0, -1],
    [0, 1, 0],
    SUN_SHADOW_FITS.distant,
  );
  const moved = fitDirectionalShadow(
    [fit.worldTexel * 0.1, 0, 0],
    [0, 0, -1],
    [0, 1, 0],
    SUN_SHADOW_FITS.distant,
  );
  assert.deepEqual(moved.center, fit.center);
  assert.deepEqual(moved.matrix, fit.matrix);
});

test("projection coverage rejects invalid W and the full WebGPU depth range", () => {
  const matrix = new Float32Array([
    1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1,
  ]);
  assert.equal(shadowProjectionPoint(matrix, [0, 0, 0.5]).inside, true);
  for (const point of [
    [1.01, 0, 0.5],
    [0, 1.01, 0.5],
    [0, 0, -0.01],
    [0, 0, 1.01],
  ] as V3[])
    assert.equal(shadowProjectionPoint(matrix, point).inside, false);
  for (const w of [-1, 0, 1e-8]) {
    matrix[15] = w;
    assert.equal(shadowProjectionPoint(matrix, [0, 0, 0.5]).inside, false);
  }
  assert.throws(() => shadowProjectionPoint(matrix, [NaN, 0, 0]), /finite/);
  for (const options of [
    { ...SUN_SHADOW_FITS.distant, extent: 0 },
    { ...SUN_SHADOW_FITS.distant, resolution: 1.5 },
    { ...SUN_SHADOW_FITS.distant, far: 1 },
    { ...SUN_SHADOW_FITS.distant, eyeDistance: 3000 },
  ])
    assert.throws(
      () => fitDirectionalShadow(eye, forward, sun, options),
      /bounds/,
    );
  assert.throws(
    () =>
      fitDirectionalShadow(eye, forward, [0, 0, 0], SUN_SHADOW_FITS.distant),
    /bounds/,
  );
});
