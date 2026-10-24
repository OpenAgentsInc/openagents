import { add, cross, normalize, scale, type V3 } from "./math.ts";
import type { GameplayEffect } from "./gameplay-effects.ts";

export const IMPACT_RECIPE_LIMITS = Object.freeze({
  events: 64,
  lifetime: 4,
  sparks: 24,
  smokePuffs: 12,
  debris: 12,
  quadsPerEvent: 50,
});

/** Cosmetic inputs only. An impact does not damage, collide with, or edit the world. */
export interface ImpactEvent {
  id: number;
  position: V3;
  normal: V3;
  intensity: number;
  radius: number;
  seed: number;
  startTime: number;
}

/** Parameters replace data; the analytic particle population keeps its ages and seeds. */
export interface ImpactRecipe {
  schema: "aethyrforge.impact-recipe.v1";
  id: "meteor-impact";
  intensity: number;
  smoke: number;
  sparksCount: number;
  flash: { life: number; size: number };
  sparks: { life: number; speed: number };
  smokePlume: { life: number; count: number; rise: number; size: number };
  debris: { life: number; count: number; speed: number };
}
export interface ImpactRecipePayload {
  event: ImpactEvent;
  recipe: ImpactRecipe;
}
export type ImpactWindSampler = (position: V3, time: number) => V3;
export interface ImpactRecipeDraw {
  disc(center: V3, radius: number, color: V3, alpha: number): void;
  ring(center: V3, x: V3, y: V3, color: V3, alpha: number): void;
  streak(a: V3, b: V3, width: number, color: V3, alpha: number): void;
  smoke(
    center: V3,
    radius: number,
    color: V3,
    alpha: number,
    debris?: boolean,
  ): void;
}
export type ImpactUpdateResult = { ok: true } | { ok: false; error: string };
const bounded = (value: unknown, min: number, max: number): value is number =>
  typeof value === "number" &&
  Number.isFinite(value) &&
  value >= min &&
  value <= max;
const integer = (value: unknown, min: number, max: number): value is number =>
  bounded(value, min, max) && Number.isInteger(value);
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const onlyKeys = (value: Record<string, unknown>, keys: readonly string[]) =>
  Reflect.ownKeys(value).every(
    (key) => typeof key === "string" && keys.includes(key),
  );
const vector = (value: unknown): value is V3 =>
  Array.isArray(value) &&
  value.length === 3 &&
  value.every((n) => bounded(n, -20000, 20000));

export function validateImpactRecipe(
  value: unknown,
): asserts value is ImpactRecipe {
  if (
    !record(value) ||
    !onlyKeys(value, [
      "schema",
      "id",
      "intensity",
      "smoke",
      "sparksCount",
      "flash",
      "sparks",
      "smokePlume",
      "debris",
    ]) ||
    value.schema !== "aethyrforge.impact-recipe.v1" ||
    value.id !== "meteor-impact" ||
    !bounded(value.intensity, 0, 4) ||
    !bounded(value.smoke, 0, 1) ||
    !integer(value.sparksCount, 0, IMPACT_RECIPE_LIMITS.sparks) ||
    !record(value.flash) ||
    !onlyKeys(value.flash, ["life", "size"]) ||
    !bounded(value.flash.life, 0.05, 0.6) ||
    !bounded(value.flash.size, 0.1, 2) ||
    !record(value.sparks) ||
    !onlyKeys(value.sparks, ["life", "speed"]) ||
    !bounded(value.sparks.life, 0.1, 2) ||
    !bounded(value.sparks.speed, 0, 12) ||
    !record(value.smokePlume) ||
    !onlyKeys(value.smokePlume, ["life", "count", "rise", "size"]) ||
    !bounded(value.smokePlume.life, 0.25, 4) ||
    !integer(value.smokePlume.count, 0, IMPACT_RECIPE_LIMITS.smokePuffs) ||
    !bounded(value.smokePlume.rise, 0, 3) ||
    !bounded(value.smokePlume.size, 0.1, 2) ||
    !record(value.debris) ||
    !onlyKeys(value.debris, ["life", "count", "speed"]) ||
    !bounded(value.debris.life, 0.1, 3) ||
    !integer(value.debris.count, 0, IMPACT_RECIPE_LIMITS.debris) ||
    !bounded(value.debris.speed, 0, 8)
  )
    throw Error(
      "Choose an impact strength from 0 to 4, smoke from 0 to 1, and up to 24 sparks.",
    );
}
export function validateImpactEvent(
  value: unknown,
): asserts value is ImpactEvent {
  if (
    !record(value) ||
    !onlyKeys(value, [
      "id",
      "position",
      "normal",
      "intensity",
      "radius",
      "seed",
      "startTime",
    ]) ||
    !integer(value.id, 0, 0xffffffff) ||
    !integer(value.seed, 0, 0xffffffff) ||
    !vector(value.position) ||
    !vector(value.normal) ||
    !bounded(Math.hypot(...value.normal), 0.9, 1.1) ||
    !bounded(value.intensity, 0.05, 4) ||
    !bounded(value.radius, 0.1, 8) ||
    !bounded(value.startTime, 0, 1e9)
  )
    throw Error(
      "Choose a valid impact position, surface direction, size, and start time.",
    );
}
function freezeRecipe(value: ImpactRecipe): ImpactRecipe {
  return Object.freeze({
    schema: value.schema,
    id: value.id,
    intensity: value.intensity,
    smoke: value.smoke,
    sparksCount: value.sparksCount,
    flash: Object.freeze({ ...value.flash }),
    sparks: Object.freeze({ ...value.sparks }),
    smokePlume: Object.freeze({ ...value.smokePlume }),
    debris: Object.freeze({ ...value.debris }),
  });
}
export const DEFAULT_METEOR_IMPACT_RECIPE: ImpactRecipe = freezeRecipe({
  schema: "aethyrforge.impact-recipe.v1",
  id: "meteor-impact",
  intensity: 1,
  smoke: 0.7,
  sparksCount: 18,
  flash: { life: 0.28, size: 1.3 },
  sparks: { life: 1.4, speed: 6 },
  smokePlume: { life: 4, count: 8, rise: 0.85, size: 0.65 },
  debris: { life: 2, count: 8, speed: 3 },
});

/** A shared, fixed-capacity event stream. Expired events never resume after a recipe edit. */
export class ImpactEventPool {
  private slots: (GameplayEffect | undefined)[] = new Array(
    IMPACT_RECIPE_LIMITS.events,
  );
  private currentRecipe: ImpactRecipe;
  private live: GameplayEffect[] = [];
  private emitted = 0;
  private rejected = 0;
  private lastTime = 0;
  constructor(recipe: ImpactRecipe = DEFAULT_METEOR_IMPACT_RECIPE) {
    validateImpactRecipe(recipe);
    this.currentRecipe = freezeRecipe(recipe);
  }
  get recipe() {
    return this.currentRecipe;
  }
  get stats() {
    let active = 0;
    let newest: GameplayEffect | undefined;
    for (const effect of this.slots) {
      if (!effect?.impactRecipe) continue;
      active++;
      const startTime = effect.impactRecipe.event.startTime;
      const newestTime = newest?.impactRecipe?.event.startTime ?? -1;
      if (
        startTime > newestTime ||
        (startTime === newestTime && effect.id > (newest?.id ?? -1))
      )
        newest = effect;
    }
    return {
      active,
      emitted: this.emitted,
      rejected: this.rejected,
      newestId: newest?.id ?? 0,
      newestStartTime: newest?.impactRecipe?.event.startTime ?? 0,
      newestAge: newest?.age ?? 0,
    };
  }
  replaceRecipe(candidate: unknown): ImpactUpdateResult {
    try {
      validateImpactRecipe(candidate);
      const recipe = freezeRecipe(candidate);
      this.currentRecipe = recipe;
      for (const effect of this.slots)
        if (effect?.impactRecipe) effect.impactRecipe.recipe = recipe;
      return { ok: true };
    } catch (error) {
      return {
        ok: false,
        error:
          error instanceof Error
            ? error.message
            : "The impact settings could not be changed.",
      };
    }
  }
  emit(candidate: unknown, now: number): ImpactUpdateResult {
    try {
      validateImpactEvent(candidate);
      if (
        !bounded(now, this.lastTime, 1e9) ||
        candidate.startTime > now ||
        candidate.startTime + IMPACT_RECIPE_LIMITS.lifetime <= now
      )
        throw Error("This impact needs a current start time.");
      this.snapshot(now);
      if (this.slots.some((entry) => entry?.id === candidate.id))
        throw Error("This impact is already playing.");
      const slot = this.slots.findIndex((entry) => entry === undefined);
      if (slot < 0)
        throw Error("Too many impacts are playing. Wait for one to finish.");
      const event: ImpactEvent = Object.freeze({
        ...candidate,
        position: Object.freeze([...candidate.position]) as unknown as V3,
        normal: Object.freeze(normalize(candidate.normal)) as unknown as V3,
      });
      this.slots[slot] = {
        id: event.id,
        kind: "impact",
        origin: event.position,
        target: add(event.position, event.normal),
        age: now - event.startTime,
        life: IMPACT_RECIPE_LIMITS.lifetime,
        radius: event.radius,
        impactRecipe: { event, recipe: this.currentRecipe },
      };
      this.emitted++;
      return { ok: true };
    } catch (error) {
      this.rejected++;
      return {
        ok: false,
        error:
          error instanceof Error
            ? error.message
            : "The impact could not be played.",
      };
    }
  }
  snapshot(now: number, capture = false): readonly GameplayEffect[] {
    if (capture) {
      if (!bounded(now, 0, 1e9))
        throw Error("Impact capture needs a valid time.");
      const captured: GameplayEffect[] = [];
      for (const effect of this.slots) {
        if (!effect?.impactRecipe) continue;
        const age = now - effect.impactRecipe.event.startTime;
        if (age >= 0 && age < IMPACT_RECIPE_LIMITS.lifetime)
          captured.push({ ...effect, age });
      }
      return captured;
    }
    if (!bounded(now, this.lastTime, 1e9))
      throw Error("Impact playback needs a forward-moving clock.");
    this.lastTime = now;
    this.live.length = 0;
    for (let i = 0; i < this.slots.length; i++) {
      const effect = this.slots[i];
      if (!effect?.impactRecipe) continue;
      effect.age = now - effect.impactRecipe.event.startTime;
      if (effect.age >= IMPACT_RECIPE_LIMITS.lifetime)
        this.slots[i] = undefined;
      else this.live.push(effect);
    }
    return this.live;
  }
  clear() {
    this.slots.fill(undefined);
    this.live.length = 0;
    this.lastTime = 0;
  }
}
function random(seed: number, salt: number) {
  let x = (seed ^ Math.imul(salt + 1, 0x9e3779b9)) >>> 0;
  x = Math.imul(x ^ (x >>> 16), 0x7feb352d);
  x = Math.imul(x ^ (x >>> 15), 0x846ca68b);
  return ((x ^ (x >>> 16)) >>> 0) / 4294967296;
}

/** Every pose comes from initial conditions and age; render frequency never advances particles. */
export function sampleImpactRecipe(
  payload: ImpactRecipePayload,
  age: number,
  draw: ImpactRecipeDraw,
  wind?: ImpactWindSampler,
): void {
  if (
    !bounded(age, 0, IMPACT_RECIPE_LIMITS.lifetime) ||
    age >= IMPACT_RECIPE_LIMITS.lifetime
  )
    return;
  const { event, recipe } = payload,
    normal = event.normal;
  const tangent = normalize(
    cross(normal, Math.abs(normal[1]) > 0.95 ? [1, 0, 0] : [0, 1, 0]),
  );
  const bitangent = cross(normal, tangent),
    power = recipe.intensity * event.intensity;
  if (power <= 0) return;
  const radius = event.radius,
    seed = event.seed;
  const frameVector = (x: number, y: number, z: number): V3 =>
    add(add(scale(tangent, x), scale(normal, y)), scale(bitangent, z));
  if (age < recipe.flash.life) {
    const t = age / recipe.flash.life,
      pulse = (1 - t) ** 2 * power;
    draw.ring(
      add(event.position, scale(normal, 0.06)),
      scale(tangent, radius * recipe.flash.size * Math.sqrt(t)),
      scale(bitangent, radius * recipe.flash.size * Math.sqrt(t)),
      [1, 0.32, 0.035],
      pulse * 2,
    );
    draw.disc(
      add(event.position, scale(normal, 0.3)),
      radius * recipe.flash.size * (1 - t) * 0.6,
      [1, 0.62, 0.17],
      pulse * 5,
    );
  }
  if (age < recipe.sparks.life) {
    const fade = (1 - age / recipe.sparks.life) ** 2;
    for (let i = 0; i < recipe.sparksCount; i++) {
      const angle = random(seed, i * 4) * Math.PI * 2,
        speed = recipe.sparks.speed * (0.4 + random(seed, i * 4 + 1) * 0.6);
      const velocity = frameVector(
        Math.cos(angle) * speed,
        (0.7 + random(seed, i * 4 + 2)) * speed,
        Math.sin(angle) * speed,
      );
      const p = add(
        event.position,
        add(scale(velocity, age), [0, -4.9 * age * age, 0]),
      );
      const previousAge = Math.max(0, age - 0.045);
      const previous = add(
        event.position,
        add(scale(velocity, previousAge), [
          0,
          -4.9 * previousAge * previousAge,
          0,
        ]),
      );
      if (p[1] >= event.position[1] - 0.05)
        draw.streak(
          previous,
          p,
          0.018 + radius * 0.012,
          [1, 0.35 + random(seed, i + 150) * 0.3, 0.035],
          fade * power * 4,
        );
    }
  }
  if (age < recipe.smokePlume.life && recipe.smoke > 0) {
    const t = age / recipe.smokePlume.life;
    for (let i = 0; i < recipe.smokePlume.count; i++) {
      const angle = random(seed, 200 + i * 3) * Math.PI * 2;
      const spread = radius * (0.2 + random(seed, 201 + i * 3) * 0.7);
      const origin = add(
        event.position,
        frameVector(Math.cos(angle) * spread, 0.15, Math.sin(angle) * spread),
      );
      // The same world wind is sampled at deterministic ages, not integrated per render.
      const flow = wind?.(origin, event.startTime + age * 0.5) ?? [0, 0, 0];
      const finiteFlow: V3 = flow.map((n) =>
        Number.isFinite(n) ? Math.max(-30, Math.min(30, n)) : 0,
      ) as V3;
      const p = add(
        origin,
        add(
          scale(finiteFlow, age * 0.6),
          scale(normal, age * recipe.smokePlume.rise),
        ),
      );
      const size =
        radius *
        recipe.smokePlume.size *
        (0.25 + t * 0.9) *
        (0.8 + random(seed, i + 300) * 0.4);
      const alpha =
        Math.min(0.75, power * recipe.smoke * 0.55) *
        Math.min(1, age / 0.18) *
        (1 - t) ** 1.4;
      draw.smoke(p, size, [0.18, 0.15, 0.12], alpha);
    }
  }
  if (age < recipe.debris.life) {
    const t = age / recipe.debris.life;
    for (let i = 0; i < recipe.debris.count; i++) {
      const angle = random(seed, 500 + i * 3) * Math.PI * 2,
        speed = recipe.debris.speed * (0.5 + random(seed, 501 + i * 3));
      const velocity = frameVector(
        Math.cos(angle) * speed,
        (0.8 + random(seed, 502 + i * 3)) * speed,
        Math.sin(angle) * speed,
      );
      const p = add(
        event.position,
        add(scale(velocity, age), [0, 0.1 - 4.9 * age * age, 0]),
      );
      if (p[1] >= event.position[1])
        draw.smoke(
          p,
          radius * (0.025 + random(seed, 600 + i) * 0.04),
          [0.13, 0.085, 0.045],
          Math.min(1, power) * (1 - t) ** 0.5,
          true,
        );
    }
  }
}
