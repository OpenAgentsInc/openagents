import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_SCENE_EFFECTS,
  sameSceneEffects,
  validSceneEffects,
  validateSceneEffects,
} from "../packages/runtime/scene-effects.ts";

test("complete effect candidates are detached and immutable before activation", () => {
  const input = structuredClone(DEFAULT_SCENE_EFFECTS);
  const accepted = validateSceneEffects(input);
  Reflect.set(input.wind.direction, "0", -1);
  input.grass.bladeHeight = 2;
  input.impact.smoke = 0;
  assert.equal(accepted.wind.direction[0], 0.8);
  assert.equal(accepted.grass.bladeHeight, 0.75);
  assert.equal(accepted.impact.smoke, 0.7);
  for (const part of [
    accepted,
    accepted.wind,
    accepted.wind.direction,
    accepted.grass,
    accepted.impact,
  ])
    assert.equal(Object.isFrozen(part), true);
});

test("one invalid field rejects the complete recipe and preserves the working settings", () => {
  const current = validateSceneEffects(DEFAULT_SCENE_EFFECTS);
  for (const input of [
    null,
    [],
    { ...current, schema: "future" },
    { ...current, unknown: 1 },
    { ...current, wind: { ...current.wind, speed: Infinity } },
    { ...current, wind: { ...current.wind, direction: [1, 1] } },
    { ...current, wind: { ...current.wind, seed: 1.5 } },
    { ...current, wind: { ...current.wind, extra: 1 } },
    { ...current, grass: { ...current.grass, bladeHeight: NaN } },
    { ...current, grass: { ...current.grass, bladeWidth: 0.019 } },
    { ...current, grass: { ...current.grass, windStrength: 2.001 } },
    { ...current, grass: { ...current.grass, extra: 1 } },
    { ...current, impact: { ...current.impact, intensity: 4.01 } },
    { ...current, impact: { ...current.impact, smoke: -0.01 } },
    { ...current, impact: { ...current.impact, sparksCount: 24.01 } },
    { ...current, impact: { ...current.impact, sparksCount: 0.5 } },
    { ...current, impact: { ...current.impact, emissionRate: 1e9 } },
  ]) {
    assert.throws(() => validateSceneEffects(input));
    assert.equal(validSceneEffects(input), false);
    assert.deepEqual(current, DEFAULT_SCENE_EFFECTS);
  }
});

test("zero-cost effects and bounded extremes remain valid with independent wind response", () => {
  const value = validateSceneEffects({
    ...DEFAULT_SCENE_EFFECTS,
    wind: {
      ...DEFAULT_SCENE_EFFECTS.wind,
      enabled: false,
      speed: 30,
      seed: 65535,
    },
    grass: { bladeHeight: 0.1, bladeWidth: 0.3, windStrength: 0 },
    impact: { intensity: 0, smoke: 0, sparksCount: 0 },
  });
  assert.equal(value.wind.enabled, false);
  assert.equal(value.grass.windStrength, 0);
  assert.equal(value.impact.sparksCount, 0);
  assert.equal(validSceneEffects(value), true);
  assert.equal(sameSceneEffects(value, structuredClone(value)), true);
  assert.equal(
    sameSceneEffects(value, {
      ...value,
      wind: { ...value.wind, enabled: true },
    }),
    false,
  );
  assert.equal(
    sameSceneEffects(value, {
      ...value,
      impact: { ...value.impact, intensity: 1 },
    }),
    false,
  );
});
