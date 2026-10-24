import assert from "node:assert/strict";
import { test } from "node:test";
import type { SceneLook } from "../packages/assets/src/scene.ts";
import {
  sharpenDisplayPixel,
  sourceSharpenStrength,
} from "../packages/renderer/display-sharpen.ts";

test("after-tone sharpening preserves constant colors and adds a signed neutral high-pass", () => {
  const center = [0.25, 0.5, 0.75];
  assert.deepEqual(
    sharpenDisplayPixel(center, [center, center, center, center], 1),
    center,
  );
  assert.deepEqual(
    sharpenDisplayPixel(
      center,
      [
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
        [0, 0, 0],
      ],
      0,
    ),
    center,
  );
  const raised = sharpenDisplayPixel(
    [0.3, 0.4, 0.5],
    Array(4).fill([0.2, 0.3, 0.4]),
    1,
  );
  raised.forEach((v, i) => assert.ok(Math.abs(v - [0.4, 0.5, 0.6][i]) < 1e-12));
  assert.deepEqual(
    sharpenDisplayPixel([1, 1, 1], Array(4).fill([0, 0, 0]), 1),
    [1, 1, 1],
  );
  assert.throws(() => sharpenDisplayPixel(center, Array(3).fill(center), 1));
});
test("only an enabled unbound source sharpening material activates the filter", () => {
  const volume = {
    class: "PostProcessVolume",
    properties: {
      enabled: true,
      unbound: true,
      blend_weight: 1,
      settings:
        "WeightedBlendables=(Array=((Weight=0.500000,Object=\"/Script/Engine.Material'/Game/kit/MM_Sharp.MM_Sharp'\"))))",
    },
  };
  const look = {
    scenarios: { day: { environment: [volume] }, night: { environment: [] } },
  } as unknown as SceneLook;
  assert.equal(sourceSharpenStrength(look, "day"), 0.5);
  assert.equal(sourceSharpenStrength(look, "night"), 0);
  volume.properties.enabled = false;
  assert.equal(sourceSharpenStrength(look, "day"), 0);
  assert.equal(sourceSharpenStrength(undefined, "day"), 0);
});
