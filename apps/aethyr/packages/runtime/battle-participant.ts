import type { BattleCrowdTerrain } from "../renderer/battle-crowd.ts";
import type { V3 } from "../renderer/math.ts";
import type { RuntimeRequests } from "./character-driver.ts";
import type {
  QuickMmorpgInput,
  QuickMmorpgMotion,
} from "./quick-mmorpg-controller.ts";

export type BattleParticipantClass = "paladin" | "sorceror";
export interface BattleParticipantSpawn {
  class: BattleParticipantClass;
  position: V3;
  yaw: number;
}
export interface BattleParticipantContactSample {
  epoch: number;
  tick: number;
  position: V3;
  contacts: readonly {
    id: number;
    position: V3;
    radius: number;
    velocity: readonly [number, number];
  }[];
  overflow: number;
}
export interface BattleParticipantCrowdContext {
  expectedEpoch: number;
  currentTick: number;
  ageSeconds: number;
  sample: BattleParticipantContactSample | null;
}
export interface BattleParticipantSnapshot {
  class: BattleParticipantClass;
  position: V3;
  eye: V3;
  yaw: number;
  pitch: number;
  height: number;
  grounded: boolean;
  velocity: V3;
  speed: number;
  state: QuickMmorpgMotion;
  actionTime: number;
  animationTime: number;
  alive: boolean;
  attackSequence: number;
  tick: number;
  droppedSeconds: number;
  crowdReady?: boolean;
  crowdBlocked?: boolean;
}
export type BattleParticipantMode =
  | { kind: "inactive" }
  | { kind: "active"; participant: BattleParticipantSnapshot };

const motion = new Set(["idle", "walk", "run", "attack", "dance", "death"]);
const finite = (value: unknown, max = 20_000): value is number =>
  typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= max;
const vector = (value: unknown): value is V3 =>
  Array.isArray(value) && value.length === 3 && value.every((v) => finite(v));

/** The battle's Wasm replies carry no shared-world player, health, or authored state. */
export function validateBattleParticipant(
  value: unknown,
): BattleParticipantSnapshot {
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("The battle did not return a complete player pose.");
  const v = value as BattleParticipantSnapshot;
  if (
    !["paladin", "sorceror"].includes(v.class) ||
    !vector(v.position) ||
    !vector(v.eye) ||
    !vector(v.velocity) ||
    !finite(v.yaw, 4) ||
    !finite(v.pitch, 2) ||
    !finite(v.height, 3) ||
    v.height <= 0 ||
    !finite(v.speed, 100) ||
    v.speed < 0 ||
    !finite(v.actionTime, 3601) ||
    v.actionTime < 0 ||
    !finite(v.animationTime, 3601) ||
    v.animationTime < 0 ||
    !finite(v.droppedSeconds, Number.MAX_SAFE_INTEGER) ||
    v.droppedSeconds < 0 ||
    typeof v.grounded !== "boolean" ||
    typeof v.alive !== "boolean" ||
    !motion.has(v.state) ||
    v.alive !== (v.state !== "death") ||
    !Number.isSafeInteger(v.attackSequence) ||
    v.attackSequence < 0 ||
    v.attackSequence > 0xffff_ffff ||
    !Number.isSafeInteger(v.tick) ||
    v.tick < 0 ||
    (v.crowdReady !== undefined && typeof v.crowdReady !== "boolean") ||
    (v.crowdBlocked !== undefined && typeof v.crowdBlocked !== "boolean") ||
    Math.abs(v.eye[0] - v.position[0]) > 0.002 ||
    Math.abs(v.eye[2] - v.position[2]) > 0.002 ||
    Math.abs(v.eye[1] - v.position[1] - 1.62) > 0.003
  )
    throw new Error("The battle returned an unsupported player pose.");
  return {
    ...v,
    position: [...v.position],
    eye: [...v.eye],
    velocity: [...v.velocity],
  };
}

/** A separate local controller; it never initializes, moves, or reconciles the MMO player. */
export class BattleParticipant {
  private current: BattleParticipantMode = { kind: "inactive" };
  constructor(private readonly kernel: RuntimeRequests) {}

  get mode(): BattleParticipantMode {
    return this.current.kind === "active"
      ? {
          kind: "active",
          participant: validateBattleParticipant(this.current.participant),
        }
      : { kind: "inactive" };
  }

  enter(terrain: BattleCrowdTerrain, spawn: BattleParticipantSpawn) {
    if (
      terrain.layout !== "mmorpg-patch" ||
      terrain.resolution !== 257 ||
      terrain.size !== 3328 ||
      !(terrain.heights instanceof Float32Array) ||
      terrain.heights.length !== 257 ** 2 ||
      !Array.isArray(terrain.center) ||
      terrain.center.length !== 2 ||
      terrain.center.some((v) => !finite(v) || v % 16 !== 0) ||
      terrain.heights.some((v) => !finite(v))
    )
      throw new Error(
        "Prepare the displayed terrain before entering the battle.",
      );
    this.checkSpawn(spawn);
    return this.apply({
      type: "battle_participant_init",
      terrain: { ...terrain, heights: Array.from(terrain.heights) },
      ...spawn,
    });
  }

  advance(
    elapsed: number,
    input: QuickMmorpgInput,
    alive: boolean,
    yawDelta = 0,
    pitchDelta = 0,
    crowd?: BattleParticipantCrowdContext,
  ) {
    this.active();
    if (
      !finite(elapsed, 10) ||
      elapsed < 0 ||
      !input ||
      !finite(input.forward, 1) ||
      !finite(input.turn ?? 0, 1) ||
      ![input.sprint, input.attack, input.dance, alive].every(
        (v) => typeof v === "boolean",
      ) ||
      !finite(yawDelta, Math.PI) ||
      !finite(pitchDelta, Math.PI)
    )
      throw new Error(
        "Use supported battle movement buttons and frame duration.",
      );
    if (crowd) validateCrowdContext(crowd);
    return this.apply({
      type: "battle_participant_step",
      elapsed,
      input: { ...input, turn: input.turn ?? 0 },
      alive,
      yawDelta,
      pitchDelta,
      crowd: crowd ?? null,
    });
  }

  step(
    elapsed: number,
    input: QuickMmorpgInput,
    alive: boolean,
    yawDelta = 0,
    pitchDelta = 0,
    crowd?: BattleParticipantCrowdContext,
  ) {
    return this.advance(elapsed, input, alive, yawDelta, pitchDelta, crowd);
  }

  respawn(spawn: Omit<BattleParticipantSpawn, "class">) {
    const value = this.active();
    this.checkSpawn({ ...spawn, class: value.class });
    return this.apply({ type: "battle_participant_respawn", ...spawn });
  }

  get snapshot(): BattleParticipantSnapshot | undefined {
    return this.current.kind === "active"
      ? validateBattleParticipant(this.current.participant)
      : undefined;
  }

  clear() {
    if (this.current.kind === "inactive") return;
    const reply = this.kernel.request<{ ok: boolean; message?: string }>({
      type: "battle_participant_clear",
    });
    if (!reply.ok)
      throw new Error(
        reply.message ?? "The battle could not release the player.",
      );
    this.current = { kind: "inactive" };
  }

  private active() {
    if (this.current.kind !== "active")
      throw new Error("Enter the battle before moving.");
    return this.current.participant;
  }
  private checkSpawn(spawn: BattleParticipantSpawn) {
    if (
      !spawn ||
      !["paladin", "sorceror"].includes(spawn.class) ||
      !vector(spawn.position) ||
      !finite(spawn.yaw, 100_000)
    )
      throw new Error(
        "Choose a supported battle position and player character.",
      );
  }
  private apply(request: object) {
    const reply = this.kernel.request<{
      ok: boolean;
      participant?: unknown;
      message?: string;
    }>(request);
    if (!reply.ok)
      throw new Error(reply.message ?? "The battle could not move the player.");
    const participant = validateBattleParticipant(reply.participant);
    this.current = { kind: "active", participant };
    return validateBattleParticipant(participant);
  }
}

function validateCrowdContext(value: BattleParticipantCrowdContext) {
  const u32 = (v: number) => Number.isInteger(v) && v >= 0 && v <= 0xffff_ffff;
  if (
    !u32(value.expectedEpoch) ||
    !u32(value.currentTick) ||
    !finite(value.ageSeconds, 10) ||
    value.ageSeconds < 0 ||
    (value.sample !== null &&
      (!u32(value.sample.epoch) ||
        !u32(value.sample.tick) ||
        !u32(value.sample.overflow) ||
        !vector(value.sample.position) ||
        !Array.isArray(value.sample.contacts) ||
        value.sample.contacts.length > 64 ||
        new Set(value.sample.contacts.map((c) => c.id)).size !==
          value.sample.contacts.length ||
        value.sample.contacts.some(
          (c) =>
            !Number.isInteger(c.id) ||
            c.id < 0 ||
            c.id >= 1_000_000 ||
            !vector(c.position) ||
            !finite(c.radius, 1.3301) ||
            c.radius < 0.1 ||
            !Array.isArray(c.velocity) ||
            c.velocity.length !== 2 ||
            c.velocity.some((v: unknown) => !finite(v, 20)),
        )))
  )
    throw new Error("Use complete nearby battle contacts.");
}
