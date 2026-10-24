import assert from "node:assert/strict";
import { test } from "node:test";
import {
  selectedCameraVignette,
  cameraVignetteIntensity,
  vignetteWeight,
} from "../packages/renderer/vignette.ts";

test("camera vignette selection preserves explicit intensity and blend independently", () => {
  for (const blendWeight of [0, 0.5, 1]) {
    const camera = {
      vignetteOverride: {
        intensity: 0.55,
        blendWeight,
        shape: "cosine-fourth" as const,
      },
    };
    const selected = selectedCameraVignette(camera)!;
    assert.deepEqual(selected, camera.vignetteOverride);
    assert.notEqual(selected, camera.vignetteOverride);
    selected.intensity = 0;
    assert.equal(camera.vignetteOverride.intensity, 0.55);
  }
});

test("fallback cameras have no inferred vignette and malformed source overrides fail", () => {
  assert.equal(selectedCameraVignette(undefined), undefined);
  assert.equal(selectedCameraVignette({}), undefined);
  for (const intensity of [NaN, Infinity, -0.01, 1.01])
    assert.throws(
      () =>
        selectedCameraVignette({
          vignetteOverride: {
            intensity,
            blendWeight: 1,
            shape: "cosine-fourth",
          },
        }),
      /between zero and one/,
    );
});

test("explicit camera lens selection does not infer an inherited or partial override", () => {
  assert.equal(cameraVignetteIntensity(undefined), 0);
  for (const blendWeight of [0, 0.5, 1])
    assert.equal(
      cameraVignetteIntensity({
        intensity: 0.55,
        blendWeight,
        shape: "cosine-fourth",
      }),
      blendWeight === 1 ? 0.55 : 0,
    );
});

test("lens falloff preserves the center, diagonal corners, and reciprocal aspect rotation", () => {
  for (const aspect of [9 / 16, 1, 16 / 9, 32 / 9])
    for (const intensity of [0, 0.25, 0.5, 0.55, 1]) {
      assert.equal(vignetteWeight([0.5, 0.5], aspect, intensity), 1);
      assert.ok(
        Math.abs(
          vignetteWeight([0, 0], aspect, intensity) -
            1 / (1 + 2 * intensity ** 2) ** 2,
        ) < 1e-14,
      );
      for (const uv of [
        [0.2, 0.3],
        [0.75, 0.1],
        [0, 1],
      ] as const)
        assert.ok(
          Math.abs(
            vignetteWeight(uv, aspect, intensity) -
              vignetteWeight([uv[1], uv[0]], 1 / aspect, intensity),
          ) < 1e-14,
        );
    }
  for (const bad of [NaN, Infinity, 0, -1])
    assert.throws(() => vignetteWeight([0.5, 0.5], bad, 0.5));
  for (const bad of [NaN, Infinity, -0.1, 1.1])
    assert.throws(() => vignetteWeight([0.5, 0.5], 1, bad));
});
