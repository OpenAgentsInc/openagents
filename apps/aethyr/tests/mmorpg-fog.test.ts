import assert from "node:assert/strict";
import test from "node:test";
import {
  mmorpgFogAmount,
  MMORPG_FOG_COLOR,
  MMORPG_FOG_WGSL,
} from "../packages/renderer/mmorpg-fog.ts";
import { MATERIAL } from "../packages/renderer/shaders.ts";
import { GRASS_WGSL } from "../packages/renderer/grass-shader.ts";
import { characterShader } from "../packages/renderer/character-renderer.ts";

test("scaled fog matches the original squared-distance height integral", () => {
  for (const y of [0, 3, 80])
    for (const distance of [1, 10, 50, 100, 250])
      for (const vertical of [-0.9, -0.2, 0.1, 0.8]) {
        const delta = [
          distance * Math.sqrt(1 - vertical ** 2),
          distance * vertical,
          0,
        ];
        const expected = Math.max(
          0,
          Math.min(
            1,
            (0.05 *
              Math.exp(-y * 6 * 0.00002) *
              (1 - Math.exp(-((distance * 6) ** 2) * vertical * 0.00002))) /
              vertical,
          ),
        );
        assert(Math.abs(mmorpgFogAmount(y, delta) - expected) < 1e-9);
      }
  assert.deepEqual(MMORPG_FOG_COLOR, [93 / 255, 103 / 255, 158 / 255]);
});

test("horizontal rays, zero distance and source-scale distant rays remain finite", () => {
  assert.equal(mmorpgFogAmount(0, [0, 0, 0]), 0);
  assert(
    Math.abs(mmorpgFogAmount(3, [100, 0, 0]) - 0.36 * Math.exp(-3 * 0.00012)) <
      1e-12,
  );
  for (const dy of [-1e-6, 0, 1e-6])
    assert(
      Math.abs(
        mmorpgFogAmount(3, [100, dy, 0]) - mmorpgFogAmount(3, [100, 0, 0]),
      ) < 2e-8,
    );
  assert.equal(mmorpgFogAmount(3, [10000, -10000, 0]), 1);
  assert.throws(() => mmorpgFogAmount(NaN, [0, 0, 0]));
  assert.throws(() => mmorpgFogAmount(0, [0, Infinity, 0]));
});

test("stock, character and grass share source fog only behind the MMO profile", () => {
  assert(MATERIAL.includes("fn mmoFog("));
  assert(
    MATERIAL.includes(
      "if frame.skyLayers.w>.5{color=mix(color,mmoFogColor*0.01,mmoFog(",
    ),
  );
  assert(
    characterShader().includes(
      "if frame.reserved1.w>.5{return vec4(mix(raw,mmoFogColor,mmoFog(",
    ),
  );
  assert(GRASS_WGSL.includes(MMORPG_FOG_WGSL));
  assert(
    GRASS_WGSL.includes(
      "if frame.fog.z>.5{return vec4(mix(raw,mmoFogColor,mmoFog(",
    ),
  );
  assert(new TextEncoder().encode(MATERIAL).length < 65536);
});
