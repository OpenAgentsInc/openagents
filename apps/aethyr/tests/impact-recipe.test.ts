import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_METEOR_IMPACT_RECIPE,
  IMPACT_RECIPE_LIMITS,
  ImpactEventPool,
  sampleImpactRecipe,
  validateImpactEvent,
  validateImpactRecipe,
  type ImpactEvent,
  type ImpactRecipeDraw,
} from "../packages/renderer/impact-recipe.ts";
import {
  GAMEPLAY_VISUAL_LIMITS,
  GameplayEffectSampler,
  gameplayLights,
  validateGameplayVisualState,
} from "../packages/renderer/gameplay-effects.ts";
import type { V3 } from "../packages/renderer/math.ts";

const event = (id = 1, startTime = 10): ImpactEvent => ({
  id,
  startTime,
  position: [2, 0, -4],
  normal: [0, 1, 0],
  radius: 2,
  intensity: 1,
  seed: 12345,
});
const recipe = () => structuredClone(DEFAULT_METEOR_IMPACT_RECIPE);

function collect(age: number, wind?: (position: V3, time: number) => V3) {
  const samples: { kind: string; values: unknown[] }[] = [];
  const draw: ImpactRecipeDraw = {
    disc: (...values) => samples.push({ kind: "disc", values }),
    ring: (...values) => samples.push({ kind: "ring", values }),
    streak: (...values) => samples.push({ kind: "streak", values }),
    smoke: (...values) =>
      samples.push({ kind: values[4] ? "debris" : "smoke", values }),
  };
  sampleImpactRecipe({ event: event(), recipe: recipe() }, age, draw, wind);
  return samples;
}

test("recipe and impact admission reject non-finite and unbounded inputs", () => {
  validateImpactRecipe(recipe());
  validateImpactEvent(event());
  for (const invalid of [NaN, Infinity, -1, 4.01])
    assert.throws(() =>
      validateImpactRecipe({ ...recipe(), intensity: invalid }),
    );
  for (const invalid of [-1, 24.1, 25, NaN])
    assert.throws(() =>
      validateImpactRecipe({ ...recipe(), sparksCount: invalid }),
    );
  assert.throws(() => validateImpactRecipe({ ...recipe(), smoke: 1.1 }));
  assert.throws(() =>
    validateImpactRecipe({
      ...recipe(),
      smokePlume: { ...recipe().smokePlume, count: 13 },
    }),
  );
  assert.throws(() =>
    validateImpactRecipe({ ...recipe(), flash: { life: 30, size: 1 } }),
  );
  assert.throws(() => validateImpactEvent({ ...event(), normal: [0, 0, 0] }));
  assert.throws(() =>
    validateImpactEvent({ ...event(), position: [0, Infinity, 0] }),
  );
  assert.throws(() => validateImpactEvent({ ...event(), seed: -1 }));
  assert.throws(() => validateImpactEvent({ ...event(), radius: 8.01 }));
  assert.throws(() => validateImpactEvent({ ...event(), damage: 100 }));
  assert.throws(() =>
    validateImpactRecipe({ ...recipe(), shader: "arbitrary" }),
  );
  for (const module of ["flash", "sparks", "smokePlume", "debris"] as const)
    assert.throws(() =>
      validateImpactRecipe({
        ...recipe(),
        [module]: { ...recipe()[module], extra: 1 },
      }),
    );
});

test("pool summaries identify the newest retained event through live edits and expiry", () => {
  const pool = new ImpactEventPool();
  assert.deepEqual(pool.stats, {
    active: 0,
    emitted: 0,
    rejected: 0,
    newestId: 0,
    newestStartTime: 0,
    newestAge: 0,
  });
  pool.emit(event(95, 10), 10);
  pool.emit(event(1, 10.5), 10.5);
  pool.emit(event(9, 10.5), 10.5);
  pool.snapshot(11);
  assert.equal(pool.stats.newestId, 9);
  assert.equal(pool.stats.newestStartTime, 10.5);
  assert.equal(pool.stats.newestAge, 0.5);
  const before = pool.stats;
  pool.replaceRecipe({ ...recipe(), smoke: 0.1 });
  assert.deepEqual(pool.stats, before);
  pool.snapshot(10.6, true);
  assert.deepEqual(pool.stats, before);
  pool.snapshot(14);
  assert.equal(pool.stats.active, 2);
  assert.equal(pool.stats.newestId, 9);
  pool.snapshot(14.5);
  assert.deepEqual(pool.stats, {
    active: 0,
    emitted: 3,
    rejected: 0,
    newestId: 0,
    newestStartTime: 0,
    newestAge: 0,
  });
});

test("64-event pool rejects overflow explicitly, expires slots, and never mutates caller inputs", () => {
  const pool = new ImpactEventPool();
  const input = event(),
    original = structuredClone(input);
  assert.deepEqual(pool.emit(input, 10), { ok: true });
  input.position[0] = 99;
  assert.deepEqual(pool.snapshot(10)[0].origin, original.position);
  for (let id = 2; id <= IMPACT_RECIPE_LIMITS.events; id++)
    assert.equal(pool.emit(event(id), 10).ok, true);
  assert.equal(pool.stats.active, 64);
  assert.equal(pool.emit(event(65), 10).ok, false);
  assert.equal(pool.emit(event(1), 10).ok, false);
  assert.equal(pool.stats.rejected, 2);
  assert.equal(pool.snapshot(13.999).length, 64);
  assert.equal(pool.snapshot(14).length, 0);
  assert.equal(pool.emit(event(65, 14), 14).ok, true);
  assert.equal(pool.stats.active, 1);
  assert.equal(pool.emit(event(66, 15), 14).ok, false);
  assert.equal(pool.emit(event(67, 0), 14).ok, false);
  assert.throws(() => pool.snapshot(13));
  pool.clear();
  assert.equal(pool.stats.active, 0);
});

test("live recipe edits preserve event identity, seed, and age; failed edits retain the working recipe", () => {
  const pool = new ImpactEventPool();
  pool.emit(event(), 10);
  const before = pool.snapshot(10.5)[0],
    beforeEvent = before.impactRecipe!.event;
  const updated = { ...recipe(), intensity: 0.5, sparksCount: 3, smoke: 0.2 };
  assert.equal(pool.replaceRecipe(updated).ok, true);
  const after = pool.snapshot(10.75)[0];
  assert.equal(after, before);
  assert.equal(after.impactRecipe!.event, beforeEvent);
  assert.equal(after.id, 1);
  assert.equal(after.age, 0.75);
  assert.equal(after.impactRecipe!.event.seed, 12345);
  assert.equal(after.impactRecipe!.recipe.intensity, 0.5);
  const working = pool.recipe;
  assert.equal(pool.replaceRecipe({ ...updated, sparksCount: 1e8 }).ok, false);
  assert.equal(pool.recipe, working);
  assert.equal(pool.snapshot(11)[0].impactRecipe!.recipe, working);
  updated.smokePlume.size = 2;
  assert.notEqual(pool.recipe.smokePlume.size, updated.smokePlume.size);
  pool.snapshot(14);
  pool.replaceRecipe(recipe());
  assert.equal(pool.snapshot(15).length, 0);
});

test("fixed-time captures seek retained events without moving the live clock or expiring slots", () => {
  const pool = new ImpactEventPool();
  pool.emit(event(), 10);
  const live = pool.snapshot(11);
  assert.equal(pool.snapshot(10.2, true)[0].age, 10.2 - 10);
  assert.equal(live[0].age, 1);
  assert.deepEqual(pool.snapshot(9, true), []);
  assert.deepEqual(pool.snapshot(20, true), []);
  assert.equal(pool.stats.active, 1);
  assert.equal(pool.snapshot(11.1)[0].age, 11.1 - 10);
  assert.throws(() => pool.snapshot(10.5));
  assert.throws(() => pool.snapshot(NaN, true));
  pool.clear();
  assert.equal(pool.emit(event(2, 0), 0).ok, true);
});

test("analytic sampling is deterministic, finite, and independent of render frequency", () => {
  const at = collect(0.65);
  for (let age = 0; age < 0.65; age += 1 / 144) collect(age);
  assert.deepEqual(collect(0.65), at);
  for (const age of [-1, 0, 0.01, 0.2, 1, 2.5, 3.999, 4, 5, NaN]) {
    const values = collect(age);
    assert(values.length <= IMPACT_RECIPE_LIMITS.quadsPerEvent);
    assert(
      values
        .flatMap((entry) => entry.values.flat())
        .filter((value) => typeof value === "number")
        .every(Number.isFinite),
    );
    if (age < 0 || age >= 4 || Number.isNaN(age))
      assert.equal(values.length, 0);
  }
});

test("smoke samples shared world wind at deterministic times without advancing gameplay", () => {
  const seen: { position: V3; time: number }[] = [];
  const source = event(),
    saved = structuredClone(source);
  const calm = collect(1),
    moving = collect(1, (position, time) => {
      seen.push({ position: [...position], time });
      return [4, 0, -2];
    });
  assert.equal(seen.length, recipe().smokePlume.count);
  assert(seen.every(({ time }) => time === 10.5));
  const calmSmoke = calm.filter(({ kind }) => kind === "smoke");
  const movingSmoke = moving.filter(({ kind }) => kind === "smoke");
  assert.equal(calmSmoke.length, movingSmoke.length);
  for (let i = 0; i < calmSmoke.length; i++) {
    const a = calmSmoke[i].values[0] as V3,
      b = movingSmoke[i].values[0] as V3;
    assert(Math.abs(b[0] - a[0] - 2.4) < 1e-10);
    assert.equal(b[1], a[1]);
    assert(Math.abs(b[2] - a[2] + 1.2) < 1e-10);
  }
  assert.deepEqual(
    moving.filter(({ kind }) => kind !== "smoke"),
    calm.filter(({ kind }) => kind !== "smoke"),
  );
  assert.deepEqual(source, saved);
  assert(collect(1, () => [Infinity, NaN, -Infinity]).length > 0);
});

test("maximum composed recipe shares the bounded particle slab, with alpha smoke in a separate suffix", () => {
  const maximal = recipe();
  maximal.sparksCount = IMPACT_RECIPE_LIMITS.sparks;
  maximal.smokePlume.count = IMPACT_RECIPE_LIMITS.smokePuffs;
  maximal.debris.count = IMPACT_RECIPE_LIMITS.debris;
  const pool = new ImpactEventPool(maximal),
    sampler = new GameplayEffectSampler();
  for (let id = 0; id < 64; id++)
    assert.equal(pool.emit(event(id), 10).ok, true);
  const effects = pool.snapshot(10.1);
  const state = { destroyed: [], debris: [], effects };
  const before = structuredClone(state);
  validateGameplayVisualState(state);
  const result = sampler.sample(
    effects,
    [0, 4, 8],
    [0, 0, -1],
    undefined,
    () => [2, 0, 0],
  );
  assert.equal(result.data.buffer, sampler.data.buffer);
  assert(result.count <= 64 * IMPACT_RECIPE_LIMITS.quadsPerEvent * 6);
  assert(result.count <= GAMEPLAY_VISUAL_LIMITS.vertices);
  assert(result.smokeCount > 0);
  assert.equal(result.count, result.smokeStart + result.smokeCount);
  for (let i = 0; i < result.count; i++)
    assert.equal(result.data[i * 10 + 9] >= 3, i >= result.smokeStart);
  assert(result.data.every(Number.isFinite));
  assert.deepEqual(state, before);
  const captured = result.data.slice();
  assert.deepEqual(
    sampler.sample(effects, [0, 4, 8], [0, 0, -1], undefined, () => [2, 0, 0])
      .data,
    captured,
  );
});

test("recipe flash lights honor live strength and lifetime without changing legacy light behavior", () => {
  const pool = new ImpactEventPool();
  pool.emit(event(), 10);
  const initial = gameplayLights(pool.snapshot(10.1));
  assert.equal(initial.length, 1);
  pool.replaceRecipe({ ...recipe(), intensity: 0.5 });
  const dimmed = gameplayLights(pool.snapshot(10.1));
  assert.equal(dimmed[0].intensity, initial[0].intensity * 0.5);
  pool.replaceRecipe({ ...recipe(), intensity: 0 });
  assert.deepEqual(gameplayLights(pool.snapshot(10.1)), []);
  pool.replaceRecipe(recipe());
  assert.deepEqual(gameplayLights(pool.snapshot(10.3)), []);
});
