// Controller behavior adapted from Quick_3D_MMORPG. See third_party/quick-mmorpg/NOTICE.
import type {
  CharacterClip,
  CharacterMotion,
  CharacterRenderPose,
} from "../renderer/character.ts";
import type { V3 } from "../renderer/math.ts";
import type { CharacterPlayer } from "./character-state.ts";

export interface QuickMmorpgInput {
  forward: number;
  turn?: number;
  sprint: boolean;
  attack: boolean;
  dance: boolean;
}
export type QuickMmorpgMotion =
  "idle" | "walk" | "run" | "attack" | "dance" | "death";
export const QUICK_MMORPG_MOTIONS: readonly QuickMmorpgMotion[] = [
  "idle",
  "walk",
  "run",
  "attack",
  "dance",
  "death",
];

/** The original main loop caps every controller, mixer, and camera update at 1/30 s. */
export function quickMmorpgFrameDelta(elapsed: number): number {
  if (!Number.isFinite(elapsed) || elapsed < 0)
    throw new Error("Use a finite character frame duration.");
  return Math.min(1 / 30, elapsed);
}

export function validateQuickMmorpgScale(worldScale: number): number {
  if (!Number.isFinite(worldScale) || worldScale < 0.01 || worldScale > 1)
    throw new Error("Choose a character world scale from 0.01 to 1.");
  return worldScale;
}

function checkedInput(input: QuickMmorpgInput) {
  if (
    !input ||
    ![input.forward, input.turn ?? 0].every(
      (value) => Number.isFinite(value) && Math.abs(value) <= 1,
    ) ||
    ![input.sprint, input.attack, input.dance].every(
      (value) => typeof value === "boolean",
    )
  )
    throw new Error("Use supported character movement buttons.");
}
const oneShot = (motion: QuickMmorpgMotion) =>
  motion === "attack" || motion === "dance" || motion === "death";

export function quickMmorpgClipTime(
  motion: QuickMmorpgMotion,
  time: number,
  duration: number,
): number {
  return oneShot(motion) ? Math.min(duration, time) : time % duration;
}

/** Three crossFadeFrom(..., true) linearly warps both active clip speeds. */
export function quickMmorpgWarpScales(
  fromDuration: number,
  toDuration: number,
  weight: number,
) {
  return {
    incoming: (toDuration / fromDuration) * (1 - weight) + weight,
    outgoing: 1 + (fromDuration / toDuration - 1) * weight,
  };
}

/** Input-driven clip clocks and source cross-fades, independent of UI notifications. */
export class QuickMmorpgLocomotion {
  private clips = new Map<QuickMmorpgMotion, CharacterClip>();
  private current: QuickMmorpgMotion = "idle";
  private currentTime = 0;
  private previous?: { motion: QuickMmorpgMotion; time: number };
  private transitionTime = 0;
  private transitionDuration = 0;

  constructor(clips: readonly CharacterClip[]) {
    for (const name of QUICK_MMORPG_MOTIONS) {
      const clip = clips.find((value) => value.name === name);
      if (!clip || !Number.isFinite(clip.duration) || clip.duration <= 0)
        throw new Error("Prepare all six character animation clips.");
      this.clips.set(name, clip);
    }
  }

  get animation(): QuickMmorpgMotion {
    return this.current;
  }
  get time(): number {
    return this.currentTime;
  }
  get movementLocked(): boolean {
    return oneShot(this.current);
  }

  reset() {
    this.current = "idle";
    this.currentTime = 0;
    this.previous = undefined;
    this.transitionTime = this.transitionDuration = 0;
  }

  die() {
    if (this.current !== "death") this.change("death", 0.2);
  }

  private duration(motion: QuickMmorpgMotion): number {
    return this.clips.get(motion)!.duration;
  }

  private change(next: QuickMmorpgMotion, duration: number) {
    const from = this.current;
    this.previous = { motion: from, time: this.currentTime };
    this.current = next;
    this.currentTime =
      (from === "walk" && next === "run") || (from === "run" && next === "walk")
        ? (this.previous.time * this.duration(next)) / this.duration(from)
        : 0;
    this.transitionTime = 0;
    this.transitionDuration = duration;
  }

  private clipTime(motion: QuickMmorpgMotion, time: number): number {
    return quickMmorpgClipTime(motion, time, this.duration(motion));
  }

  /** Advance before Rust movement; movementLocked preserves source one-shot momentum. */
  advance(input: QuickMmorpgInput, elapsed: number) {
    checkedInput(input);
    const dt = quickMmorpgFrameDelta(elapsed);
    const moving = input.forward !== 0;
    if (this.current === "idle") {
      if (moving) this.change("walk", 0.1);
      else if (input.attack) this.change("attack", 0.4);
      else if (input.dance) this.change("dance", 0.2);
    } else if (this.current === "walk") {
      if (!moving) this.change("idle", 0.25);
      else if (input.sprint) this.change("run", 0.1);
    } else if (this.current === "run") {
      if (!moving) this.change("idle", 0.25);
      else if (!input.sprint) this.change("walk", 0.1);
    }
    // Three.js crossFadeFrom(..., true) linearly warps both active clip speeds.
    const previous = this.previous;
    this.transitionTime += dt;
    const weight =
      this.transitionDuration > 0
        ? Math.min(1, this.transitionTime / this.transitionDuration)
        : 1;
    const scales = previous
      ? quickMmorpgWarpScales(
          this.duration(previous.motion),
          this.duration(this.current),
          weight,
        )
      : { incoming: 1, outgoing: 1 };
    this.currentTime = this.clipTime(
      this.current,
      this.currentTime + dt * scales.incoming,
    );
    if (previous) {
      previous.time = this.clipTime(
        previous.motion,
        previous.time + dt * scales.outgoing,
      );
      if (weight >= 1) this.previous = undefined;
    }
    // The source mixer finishes the action before the movement gate runs this frame.
    if (
      (this.current === "attack" || this.current === "dance") &&
      this.currentTime >= this.duration(this.current)
    )
      this.change("idle", 0.25);
    return {
      animation: this.current,
      movementLocked: this.movementLocked,
      elapsed: dt,
    };
  }

  pose(
    player: Pick<CharacterPlayer, "position" | "yaw">,
    visible = true,
  ): CharacterRenderPose {
    if (
      player.position.length !== 3 ||
      !player.position.every(Number.isFinite) ||
      !Number.isFinite(player.yaw)
    )
      throw new Error("Use a complete finite character pose.");
    const pose: CharacterRenderPose = {
      position: [...player.position],
      yaw: Math.PI - player.yaw,
      animation: this.current as CharacterMotion,
      time: this.currentTime,
      visible,
    };
    if (this.previous)
      pose.blend = {
        from: this.previous.motion as CharacterMotion,
        fromTime: this.previous.time,
        weight: Math.min(1, this.transitionTime / this.transitionDuration),
      };
    return pose;
  }
}

export interface QuickMmorpgFraming {
  eye: V3;
  target: V3;
  direction: V3;
  up: V3;
  fov: number;
  near: number;
  far: number;
}

/** Source follow offsets and exponential smoothing, expressed in world meters. */
export class QuickMmorpgCamera {
  private position: V3 = [0, 0, 0];
  private lookAt: V3 = [0, 0, 0];
  constructor(readonly worldScale = 1 / 6) {
    validateQuickMmorpgScale(worldScale);
  }

  reset() {
    this.position = [0, 0, 0];
    this.lookAt = [0, 0, 0];
  }

  frame(
    feet: V3,
    yaw: number,
    groundHeight: number,
    elapsed: number,
  ): QuickMmorpgFraming {
    if (
      feet.length !== 3 ||
      !feet.every(Number.isFinite) ||
      !Number.isFinite(yaw) ||
      !Number.isFinite(groundHeight)
    )
      throw new Error("Use a complete finite character camera pose.");
    const dt = quickMmorpgFrameDelta(elapsed),
      scale = this.worldScale;
    const forward: V3 = [Math.sin(yaw), 0, -Math.cos(yaw)];
    const idealPosition: V3 = [
      feet[0] - forward[0] * 15 * scale,
      Math.max(feet[1] + 10 * scale, groundHeight + 5 * scale),
      feet[2] - forward[2] * 15 * scale,
    ];
    const idealLookAt: V3 = [
      feet[0] + forward[0] * 20 * scale,
      feet[1] + 5 * scale,
      feet[2] + forward[2] * 20 * scale,
    ];
    const weight = 1 - Math.pow(0.01, dt);
    for (let axis = 0; axis < 3; axis++) {
      this.position[axis] +=
        (idealPosition[axis] - this.position[axis]) * weight;
      this.lookAt[axis] += (idealLookAt[axis] - this.lookAt[axis]) * weight;
    }
    const delta = this.lookAt.map(
      (value, axis) => value - this.position[axis],
    ) as V3;
    const length = Math.hypot(...delta);
    return {
      eye: [...this.position],
      target: [...this.lookAt],
      direction:
        length > 1e-9 ? (delta.map((value) => value / length) as V3) : forward,
      up: [0, 1, 0],
      fov: Math.PI / 3,
      near: scale,
      far: 10000 * scale,
    };
  }
}
