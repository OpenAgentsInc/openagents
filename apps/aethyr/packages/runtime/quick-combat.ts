import type { EnemyActorPose } from "./enemies.ts";

export interface CombatPlayer {
  health: number;
  maxHealth: number;
  alive: boolean;
  state: "idle" | "attack" | "death";
  actionTime: number;
  cooldown: number;
  attacks: number;
  hits: number;
}
export interface CombatActor {
  id: number;
  position: [number, number, number];
  yaw: number;
  health: number;
  maxHealth: number;
  alive: boolean;
  state: "idle" | "walk" | "attack" | "death";
  actionTime: number;
  animationTime: number;
}
export interface QuickCombat {
  player: CombatPlayer;
  actors: CombatActor[];
  total: number;
  alive: number;
  killed: number;
}
export interface CombatRendering {
  modelLoaded: boolean;
  submittedActors: number;
  submittedTriangles: number;
  submittedHealthBars: number;
  submittedHealthBarTriangles: number;
  geometryBytes: number;
}
export function validCombatRendering(value: unknown): value is CombatRendering {
  return (
    record(value) &&
    typeof value.modelLoaded === "boolean" &&
    count(value.submittedActors, 16) &&
    count(value.submittedTriangles, 1e7) &&
    count(value.submittedHealthBars, 16) &&
    count(value.submittedHealthBarTriangles, 64) &&
    count(value.geometryBytes, 256 * 1024 * 1024)
  );
}
const record = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === "object" && !Array.isArray(v);
const bounded = (v: unknown, max = 1e9): v is number =>
  typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= max;
const count = (v: unknown, max: number) =>
  bounded(v, max) && Number.isInteger(v);
function health(v: Record<string, unknown>) {
  return (
    bounded(v.health, 1e6) &&
    bounded(v.maxHealth, 1e6) &&
    v.maxHealth > 0 &&
    v.health <= v.maxHealth &&
    typeof v.alive === "boolean" &&
    v.alive === v.health > 0 &&
    (v.state === "death") === !v.alive &&
    bounded(v.actionTime)
  );
}
/** Admit the bounded simulation snapshot before exposing health or actor poses. */
export function validQuickCombat(value: unknown): value is QuickCombat {
  if (
    !record(value) ||
    !record(value.player) ||
    !health(value.player) ||
    !["idle", "attack", "death"].includes(String(value.player.state)) ||
    !bounded(value.player.cooldown) ||
    !count(value.player.attacks, 1e9) ||
    !count(value.player.hits, 1e9) ||
    !Array.isArray(value.actors) ||
    value.actors.length > 16 ||
    ![value.total, value.alive, value.killed].every((v) => count(v, 16))
  )
    return false;
  const ids = new Set<number>();
  for (const actor of value.actors) {
    if (
      !record(actor) ||
      !health(actor) ||
      !count(actor.id, 0xffff_fffe) ||
      ids.has(actor.id as number) ||
      !["idle", "walk", "attack", "death"].includes(String(actor.state)) ||
      !bounded(actor.animationTime) ||
      !Array.isArray(actor.position) ||
      actor.position.length !== 3 ||
      !actor.position.every(
        (n) =>
          typeof n === "number" && Number.isFinite(n) && Math.abs(n) <= 20_000,
      ) ||
      typeof actor.yaw !== "number" ||
      !Number.isFinite(actor.yaw)
    )
      return false;
    ids.add(actor.id as number);
  }
  return (
    value.total === value.actors.length &&
    value.alive === value.actors.filter((a) => a.alive).length &&
    Number(value.alive) + Number(value.killed) === value.total
  );
}

export function combatEnemyPoses(combat: QuickCombat): EnemyActorPose[] {
  return combat.actors.map((actor) => ({
    id: actor.id,
    position: [...actor.position],
    yaw: actor.yaw,
    alive: actor.alive,
    deathSeconds: actor.alive ? 0 : actor.actionTime,
    animationTime: actor.animationTime,
    animation: actor.state,
    health: actor.health,
    maxHealth: actor.maxHealth,
  }));
}

/** A small encounter starts inside the source NPC's acquisition distance. */
export function zombieSpawns(position: readonly number[], yaw: number) {
  return Array.from({ length: 8 }, (_, id) => {
    const angle = yaw + (id * Math.PI) / 4;
    const distance = 6.2 + (id % 2) * 1.2;
    return {
      id,
      position: [
        position[0] + Math.sin(angle) * distance,
        position[1],
        position[2] - Math.cos(angle) * distance,
      ],
    };
  });
}
