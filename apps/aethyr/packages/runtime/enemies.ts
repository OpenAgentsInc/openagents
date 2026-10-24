import { readAsset } from "../assets/src/asset-download.ts";

export const ENEMY_LIMITS = Object.freeze({
  actors: 16,
  waypoints: 32,
  speed: 6.4,
  gaitSpeed: 3.4,
});
export type EnemyVector = [number, number, number];
export interface EnemyActorPose {
  id: number;
  position: EnemyVector;
  yaw: number;
  alive: boolean;
  deathSeconds: number;
  /** Accepted horizontal travel divided by the nominal 3.4 m/s gait speed. */
  animationTime: number;
  animation?: "idle" | "walk" | "run" | "attack" | "death";
  health?: number;
  maxHealth?: number;
}
export interface EnemySnapshot {
  total: number;
  alive: number;
  killed: number;
  actors: EnemyActorPose[];
}
export interface EnemyPatrol {
  id: number;
  route: EnemyVector[];
  speed: number;
}
export interface EnemyPatrolsManifest {
  schema: "aethyrforge.enemy-patrols.v1";
  scenarios: Record<"day" | "night", EnemyPatrol[]>;
}
const vector = (v: unknown): v is EnemyVector =>
  Array.isArray(v) &&
  v.length === 3 &&
  v.every(
    (x) => typeof x === "number" && Number.isFinite(x) && Math.abs(x) <= 20_000,
  );
const id = (v: unknown): v is number =>
  Number.isInteger(v) && Number(v) >= 0 && Number(v) < 0xffff_ffff;
const record = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === "object" && !Array.isArray(v);

/** Admit bounded runtime routes separately from authored building state. */
export function validateEnemyPatrols(value: unknown): EnemyPatrolsManifest {
  const fail = () => {
    throw new Error("Prepare complete knight routes for the town.");
  };
  if (
    !record(value) ||
    value.schema !== "aethyrforge.enemy-patrols.v1" ||
    !record(value.scenarios)
  )
    return fail();
  for (const mode of ["day", "night"] as const) {
    const actors = value.scenarios[mode];
    if (!Array.isArray(actors) || actors.length > ENEMY_LIMITS.actors)
      return fail();
    const ids = new Set<number>();
    for (const actor of actors) {
      if (
        !record(actor) ||
        !id(actor.id) ||
        ids.has(actor.id) ||
        typeof actor.speed !== "number" ||
        !Number.isFinite(actor.speed) ||
        actor.speed < 0.1 ||
        actor.speed > ENEMY_LIMITS.speed ||
        !Array.isArray(actor.route) ||
        actor.route.length < 2 ||
        actor.route.length > ENEMY_LIMITS.waypoints ||
        !actor.route.every(vector)
      )
        return fail();
      ids.add(actor.id);
      for (let i = 0; i < actor.route.length; i++) {
        const a = actor.route[i],
          b = actor.route[(i + 1) % actor.route.length];
        const distance = Math.hypot(a[0] - b[0], a[2] - b[2]);
        if (distance < 0.02 || distance > 256) return fail();
      }
    }
  }
  return structuredClone(value) as unknown as EnemyPatrolsManifest;
}

export async function loadEnemyPatrols(
  url: string,
): Promise<EnemyPatrolsManifest> {
  if (!/^\/private-assets\/[A-Za-z0-9._-]+\.json$/.test(url))
    throw new Error("Prepare the knight routes inside this world.");
  return readAsset(url, async (response) => {
    if (!response.ok)
      throw new Error("The town's knight routes could not open.");
    return validateEnemyPatrols(await response.json());
  });
}

/** Full actor poses are available on inspection; the UI receives counts only. */
export function validEnemySnapshot(value: unknown): value is EnemySnapshot {
  if (
    !record(value) ||
    !Array.isArray(value.actors) ||
    value.actors.length > ENEMY_LIMITS.actors ||
    ![value.total, value.alive, value.killed].every(
      (n) =>
        Number.isInteger(n) &&
        Number(n) >= 0 &&
        Number(n) <= ENEMY_LIMITS.actors,
    )
  )
    return false;
  const ids = new Set<number>();
  for (const actor of value.actors) {
    if (
      !record(actor) ||
      !id(actor.id) ||
      ids.has(actor.id) ||
      !vector(actor.position) ||
      typeof actor.alive !== "boolean" ||
      typeof actor.yaw !== "number" ||
      !Number.isFinite(actor.yaw) ||
      ![actor.deathSeconds, actor.animationTime].every(
        (n) => typeof n === "number" && Number.isFinite(n) && n >= 0,
      ) ||
      (actor.animation !== undefined &&
        !["idle", "walk", "run", "attack", "death"].includes(
          String(actor.animation),
        )) ||
      ((actor.health !== undefined || actor.maxHealth !== undefined) &&
        !(
          typeof actor.health === "number" &&
          Number.isFinite(actor.health) &&
          typeof actor.maxHealth === "number" &&
          Number.isFinite(actor.maxHealth) &&
          actor.maxHealth > 0 &&
          actor.maxHealth <= 1e6 &&
          actor.health >= 0 &&
          actor.health <= actor.maxHealth &&
          actor.alive === actor.health > 0
        ))
    )
      return false;
    ids.add(actor.id);
  }
  return (
    value.total === value.actors.length &&
    value.alive === value.actors.filter((a) => a.alive).length &&
    Number(value.alive) + Number(value.killed) === value.total
  );
}
