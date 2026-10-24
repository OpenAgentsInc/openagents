import assert from "node:assert/strict";
import test from "node:test";
import { Renderer } from "../packages/renderer/renderer.ts";
import { DEFAULT_SCENE_EFFECTS } from "../packages/runtime/scene-effects.ts";

test("actual renderer parameter updates preserve allocations, event identity, and last working values", () => {
  const renderer = Object.create(Renderer.prototype) as any;
  const appearances: unknown[] = [];
  Object.assign(renderer, {
    gpuBytes: 123456,
    previewImpactId: 0,
    grass: { setAppearance: (settings: unknown) => appearances.push(settings) },
  });
  renderer.setSceneEffects(DEFAULT_SCENE_EFFECTS);
  renderer.previewImpact([6, 0, 14], 2);
  const pool = renderer.previewImpacts;
  pool.snapshot(2.5);
  const before = { ...pool.stats };
  const next = structuredClone(DEFAULT_SCENE_EFFECTS);
  next.wind.direction = [-0.8, -0.6];
  next.grass.bladeHeight = 1.2;
  next.impact.smoke = 0.2;
  renderer.setSceneEffects(next);
  assert.equal(renderer.previewImpacts, pool);
  assert.deepEqual(pool.stats, before);
  assert.equal(pool.recipe.smoke, 0.2);
  assert.equal(renderer.gpuBytes, 123456);
  assert.equal(renderer.sceneEffectsSettings.grass.bladeHeight, 1.2);
  const accepted = renderer.sceneEffectsSettings;
  for (const bad of [
    { ...next, wind: { ...next.wind, speed: NaN } },
    { ...next, grass: { ...next.grass, bladeHeight: 9 } },
    { ...next, impact: { ...next.impact, sparksCount: 25 } },
  ])
    assert.throws(() => renderer.setSceneEffects(bad));
  assert.equal(renderer.sceneEffectsSettings, accepted);
  assert.equal(appearances.length, 2);
  assert.equal(pool.recipe.smoke, 0.2);
  assert.deepEqual(pool.stats, before);
  assert.throws(() => renderer.previewImpact([NaN, 0, 0], 2.6));
  assert.equal(pool.stats.active, 1);
  assert.equal(renderer.previewImpactId, 1);
});
