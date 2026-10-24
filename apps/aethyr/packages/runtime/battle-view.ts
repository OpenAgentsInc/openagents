import type { Camera } from "../renderer/renderer.ts";
import type { BattleCrowdTerrain } from "../renderer/battle-crowd.ts";
import { mmorpgHeight } from "./mmorpg-terrain.ts";

/** Sample once when entering the local experiment, never in the UI or frame loop. */
export function battleTerrain(
  center: readonly [number, number],
): BattleCrowdTerrain {
  if (
    center.length !== 2 ||
    center.some((value) => !Number.isFinite(value) || Math.abs(value) > 20000)
  )
    throw new Error("Choose a supported battlefield center.");
  const resolution = 257,
    size = 3328,
    aligned: [number, number] = [
      Math.floor(center[0] / 16) * 16,
      Math.floor(center[1] / 16) * 16,
    ],
    heights = new Float32Array(resolution ** 2);
  // Preserve the displayed vertices, including rectangular 2×24 m boundary strips.
  // Uniform resampling cannot preserve those strips' slanted triangle creases.
  const displacement = (index: number) => {
    const offset = index - 128,
      radius = Math.abs(offset);
    return (
      Math.sign(offset) * (radius <= 64 ? radius * 2 : 128 + (radius - 64) * 24)
    );
  };
  for (let z = 0; z < resolution; z++)
    for (let x = 0; x < resolution; x++)
      heights[z * resolution + x] = mmorpgHeight(
        aligned[0] + displacement(x),
        aligned[1] + displacement(z),
      );
  return { layout: "mmorpg-patch", resolution, size, heights, center: aligned };
}

export type BattleView =
  "overview" | "orbit" | "player" | "first" | "third" | "dolly";
export interface BattleParticipantView {
  position: readonly [number, number, number];
  yaw: number;
  eye?: readonly [number, number, number];
  pitch?: number;
  height?: number;
}
export const BATTLE_ORBIT_RANGE = [2.5, 18] as const;
export interface BattleOrbit {
  yaw: number;
  pitch: number;
  distance: number;
}
const angle = (value: number) => Math.atan2(Math.sin(value), Math.cos(value));
const clamp = (value: number, min: number, max: number) =>
  Math.max(min, Math.min(max, value));

/** Camera-only state. Looking around a stationary fighter never changes its pose. */
export class BattleFollowCamera {
  private current: BattleOrbit = { yaw: 0, pitch: -0.22, distance: 6 };
  private attackUntil = 0;

  get orbit(): BattleOrbit {
    return { ...this.current };
  }
  reset(pose?: Pick<BattleParticipantView, "yaw" | "pitch">) {
    this.current = {
      yaw: angle(pose?.yaw ?? 0),
      pitch: clamp((pose?.pitch ?? 0) - 0.22, -1.3, 1.25),
      distance: 6,
    };
    this.cancelAttack();
  }
  followFirst(pose: Pick<BattleParticipantView, "yaw" | "pitch">) {
    this.current.yaw = angle(pose.yaw);
    this.current.pitch = clamp(pose.pitch ?? 0, -1.3, 1.25);
  }
  look(yaw: number, pitch: number) {
    if (![yaw, pitch].every(Number.isFinite)) return;
    this.current.yaw = angle(this.current.yaw + clamp(yaw, -Math.PI, Math.PI));
    this.current.pitch = clamp(
      this.current.pitch + clamp(pitch, -Math.PI, Math.PI),
      -1.3,
      1.25,
    );
  }
  zoom(distance: number) {
    if (!Number.isFinite(distance))
      throw Error("Choose a supported camera distance.");
    this.current.distance = clamp(distance, ...BATTLE_ORBIT_RANGE);
  }
  cancelAttack() {
    this.attackUntil = 0;
  }
  /** A brief click survives the bounded turn; cancel on blur, death, or a new life. */
  aimAttack(yaw: number, elapsed: number, pressed: boolean, now: number) {
    if (pressed) this.attackUntil = now + 1200;
    if (
      !Number.isFinite(now) ||
      !Number.isFinite(yaw) ||
      !Number.isFinite(elapsed) ||
      elapsed < 0 ||
      now >= this.attackUntil
    )
      return { yawDelta: 0, attack: false };
    const delta = angle(this.current.yaw - yaw);
    const step = clamp(
      delta,
      -4 * Math.min(elapsed, 0.05),
      4 * Math.min(elapsed, 0.05),
    );
    return { yawDelta: step, attack: Math.abs(delta - step) <= 0.075 };
  }
}

function aim(
  eye: [number, number, number],
  target: readonly number[],
  fov: number,
  near: number,
  sourceFogScale: number,
): Camera {
  const delta = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
  return {
    eye,
    yaw: Math.atan2(delta[0], -delta[2]),
    pitch: Math.atan2(delta[1], Math.hypot(delta[0], delta[2])),
    fov,
    near,
    far: 5000,
    sourceFogScale,
  };
}

/** Render-only shots. Participation follows the local fixed-step actor, never the shared MMO player. */
export function battleCamera(
  center: readonly number[],
  radius: number,
  view: BattleView,
  seconds: number,
  participant?: BattleParticipantView,
  orbit?: BattleOrbit,
): Camera {
  if (
    center.length !== 3 ||
    center.some((n) => !Number.isFinite(n) || Math.abs(n) > 20000) ||
    !Number.isFinite(radius) ||
    radius <= 0 ||
    radius > 1000 ||
    !Number.isFinite(seconds) ||
    seconds < 0
  )
    throw new Error("Choose a complete battlefield camera.");
  if (view === "first" || view === "third" || view === "player") {
    if (
      !participant ||
      participant.position.length !== 3 ||
      participant.position.some(
        (n) => !Number.isFinite(n) || Math.abs(n) > 20000,
      ) ||
      !Number.isFinite(participant.yaw)
    )
      throw new Error(
        "Enter the local battle before choosing a following camera.",
      );
    const height = participant.height ?? 1.8;
    if (
      !Number.isFinite(height) ||
      height < 0.5 ||
      height > 5 ||
      (participant.pitch !== undefined &&
        (!Number.isFinite(participant.pitch) ||
          Math.abs(participant.pitch) > 1.5)) ||
      (participant.eye &&
        (participant.eye.length !== 3 ||
          participant.eye.some(
            (n) => !Number.isFinite(n) || Math.abs(n) > 20000,
          )))
    )
      throw new Error("Choose a supported battle participant camera.");
    const [x, y, z] = participant.position;
    if (view === "first")
      return {
        eye: participant.eye ? [...participant.eye] : [x, y + height * 0.9, z],
        yaw: participant.yaw,
        pitch: participant.pitch ?? 0,
        fov: Math.PI / 3,
        near: 0.06,
        far: 5000,
        sourceFogScale: 1,
      };
    if (
      orbit &&
      (!Number.isFinite(orbit.yaw) ||
        !Number.isFinite(orbit.pitch) ||
        Math.abs(orbit.pitch) > 1.5 ||
        !Number.isFinite(orbit.distance) ||
        orbit.distance < BATTLE_ORBIT_RANGE[0] ||
        orbit.distance > BATTLE_ORBIT_RANGE[1])
    )
      throw Error("Choose a supported battle orbit.");
    const sin = Math.sin(orbit?.yaw ?? participant.yaw),
      cos = Math.cos(orbit?.yaw ?? participant.yaw);
    const orbitPitch = Math.max(
      -1.3,
      Math.min(1.25, orbit?.pitch ?? (participant.pitch ?? 0) - 0.22),
    );
    const distance = orbit?.distance ?? 6;
    const horizontal = Math.cos(orbitPitch),
      vertical = Math.sin(orbitPitch);
    const pivotY = y + height * 0.82;
    const eye: [number, number, number] = [
      x - sin * horizontal * distance + cos * 0.48,
      pivotY - vertical * distance,
      z + cos * horizontal * distance + sin * 0.48,
    ];
    eye[1] = Math.max(eye[1], mmorpgHeight(eye[0], eye[2]) + 0.4);
    return aim(
      eye,
      [
        x + sin * horizontal * 2.6,
        pivotY + vertical * 2.6,
        z - cos * horizontal * 2.6,
      ],
      Math.PI / 3,
      0.1,
      1,
    );
  }
  if (view === "dolly") {
    // A short track through the fighting front keeps human-scale actors readable at every pool size.
    const x = center[0] + Math.sin(seconds * 0.055) * 9;
    const z = center[2] + Math.sin(seconds * 0.035 + 0.4) * 6;
    const targetY = mmorpgHeight(x, z);
    const eye: [number, number, number] = [
      x - 8 + Math.sin(seconds * 0.04) * 2,
      targetY + 12 + Math.sin(seconds * 0.045) * 1.5,
      z + 12,
    ];
    eye[1] = Math.max(eye[1], mmorpgHeight(eye[0], eye[2]) + 9);
    return aim(eye, [x, targetY + 0.9, z], 0.78, 0.15, 2);
  }
  if (view !== "overview" && view !== "orbit")
    throw new Error("Choose a supported battle camera.");
  const angle = view === "orbit" ? seconds * 0.065 : 0.32;
  const distance = Math.max(32, radius * (view === "orbit" ? 2.3 : 2.9));
  const eye: [number, number, number] = [
    center[0] + Math.sin(angle) * distance,
    center[1] + Math.max(14, radius * (view === "orbit" ? 0.8 : 1.85)),
    center[2] + Math.cos(angle) * distance,
  ];
  const delta = [
    center[0] - eye[0],
    center[1] + 1 - eye[1],
    center[2] - eye[2],
  ];
  return {
    eye,
    yaw: Math.atan2(delta[0], -delta[2]),
    pitch: Math.atan2(delta[1], Math.hypot(delta[0], delta[2])),
    fov: view === "orbit" ? 1 : 0.9,
    near: 0.5,
    far: 5000,
    sourceFogScale: Math.max(10, radius * 0.08),
  };
}
