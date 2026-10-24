import assert from "node:assert/strict";
import { test } from "node:test";
import {
  SCENE_LINEAR_SCALE,
  OWNED_EXPOSURE,
  GENERATED_SKY_RADIANCE_SCALE,
} from "../packages/renderer/radiometry.ts";

test("shared radiance normalization cancels at camera exposure without changing emitter ratios", () => {
  const source = [25 / Math.PI, 3, 0.5, 7.5];
  for (const mode of ["day", "night"] as const) {
    const stored = source.map((value) => value * SCENE_LINEAR_SCALE);
    const exposed = stored.map(
      (value) => (value * OWNED_EXPOSURE[mode]) / SCENE_LINEAR_SCALE,
    );
    source.forEach((value, index) =>
      assert.ok(Math.abs(exposed[index] - value * 0.125) < 1e-12),
    );
    assert.ok(Math.abs(stored[2] / stored[1] - source[2] / source[1]) < 1e-12);
  }
  assert.equal(GENERATED_SKY_RADIANCE_SCALE, 8);
});
