// Adapted from OpenAgents Verse's FollowCamera at 83fe8b7f6b3cd437ae9b7af213cf9f04875dd793.
// See third_party/openagents-camera/NOTICE for attribution and coordinate changes.
import { add, direction, scale, sub, type V3 } from "./math.ts";

export type CharacterPerspective = "first" | "third";
export type CameraSweep = (from: V3, to: V3, radius: number) => number;
export type CharacterFraming = {
  eye: V3;
  direction: V3;
  near: number;
  hideAvatar: boolean;
  limited: boolean;
  perspective: CharacterPerspective;
  orbitDistance: number;
  blend: number;
};

const MIN_DISTANCE = 2.5;
const MAX_DISTANCE = 40;
const FIRST_PERSON_PUSH = 0.15;
const EYE_HEIGHT = 1.7;
const FOCUS_HEIGHT = 1.6;
const PIVOT_HEIGHT = 1.9;
const PROBE_RADIUS = 0.12;
const TRANSITION_SECONDS = 0.25;
const wrap = (angle: number) =>
  ((((angle + Math.PI) % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI)) -
  Math.PI;
const lerp = (a: V3, b: V3, t: number): V3 => add(a, scale(sub(b, a), t));
const finite = (vector: V3) => vector.every(Number.isFinite);

/** Player-relative orbit; positive yaw looks right and positive pitch looks up. */
export class CharacterCamera {
  yawOffset = 0;
  pitch = -0.28;
  orbitDistance = 6;
  perspective: CharacterPerspective = "third";
  private push = 0;
  private blendValue = 0;
  private boom = Infinity;
  private tracked?: V3;

  /** Free orbit leaves the character's facing unchanged. */
  orbit(dx: number, dy: number): void {
    if (!Number.isFinite(dx) || !Number.isFinite(dy)) return;
    this.yawOffset = wrap(this.yawOffset + dx * 0.004);
    this.pitch = Math.max(-1.45, Math.min(1.45, this.pitch - dy * 0.004));
  }

  /** Returns the character yaw change while the camera follows its facing. */
  mouselook(dx: number, dy: number): number {
    if (!Number.isFinite(dx) || !Number.isFinite(dy)) return 0;
    this.pitch = Math.max(-1.45, Math.min(1.45, this.pitch - dy * 0.004));
    return dx * 0.004;
  }

  takeOffset(): number {
    const offset = this.yawOffset;
    this.yawOffset = 0;
    return offset;
  }

  setPerspective(perspective: CharacterPerspective, immediate = false): void {
    this.perspective = perspective;
    this.push = 0;
    if (immediate) this.blendValue = perspective === "first" ? 1 : 0;
  }

  setOrbitDistance(distance: number): void {
    if (!Number.isFinite(distance)) return;
    this.orbitDistance = Math.max(
      MIN_DISTANCE,
      Math.min(MAX_DISTANCE, distance),
    );
    this.setPerspective("third");
  }

  /** Factors above one zoom in; a deliberate extra push enters first person. */
  zoomBy(factor: number): void {
    if (!Number.isFinite(factor) || factor <= 0) return;
    const step = Math.log(factor);
    if (this.perspective === "first") {
      this.push = step < 0 ? this.push - step : 0;
      if (this.push >= FIRST_PERSON_PUSH) {
        this.setPerspective("third");
        this.orbitDistance = MIN_DISTANCE;
      }
      return;
    }
    const wanted = this.orbitDistance / factor;
    if (step > 0 && wanted < MIN_DISTANCE) {
      this.push += Math.log(MIN_DISTANCE / wanted);
      this.orbitDistance = MIN_DISTANCE;
      if (this.push >= FIRST_PERSON_PUSH) this.setPerspective("first");
    } else {
      this.push = 0;
      this.orbitDistance = Math.max(
        MIN_DISTANCE,
        Math.min(MAX_DISTANCE, wanted),
      );
    }
  }

  /** Free orbit settles behind a moving character after orbit input stops. */
  settle(dt: number): void {
    if (Number.isFinite(dt) && dt > 0) this.yawOffset *= 0.02 ** dt;
  }

  resetCollision(): void {
    this.boom = Infinity;
    this.tracked = undefined;
  }

  /** Advances the camera once per engine frame, independently of UI state. */
  frame(
    feet: V3,
    playerYaw: number,
    sweep: CameraSweep,
    groundY: number,
    elapsed: number,
  ): CharacterFraming {
    if (
      !finite(feet) ||
      !Number.isFinite(playerYaw) ||
      !Number.isFinite(groundY)
    ) {
      throw new Error("The character camera needs a finite player position.");
    }
    const dt = Number.isFinite(elapsed)
      ? Math.max(0, Math.min(0.25, elapsed))
      : 0;
    const target = this.perspective === "first" ? 1 : 0;
    const step = dt / TRANSITION_SECONDS;
    this.blendValue =
      this.blendValue < target
        ? Math.min(target, this.blendValue + step)
        : Math.max(target, this.blendValue - step);
    const forward = direction(playerYaw + this.yawOffset, this.pitch);
    const focus = add(feet, [0, FOCUS_HEIGHT, 0]);
    const head = add(feet, [0, EYE_HEIGHT, 0]);
    const orbit = add(focus, scale(forward, -this.orbitDistance));
    const t = this.blendValue;
    let eye = lerp(orbit, head, t * t * (3 - 2 * t));
    if (t < 1) eye[1] = Math.max(eye[1], groundY + 0.4);
    let limited = false;
    let close = false;
    if (t < 1) {
      const cast = (from: V3, to: V3): number => {
        const fraction = sweep(from, to, PROBE_RADIUS);
        // An invalid collision answer closes the boom instead of putting it through a wall.
        return Number.isFinite(fraction)
          ? Math.max(0, Math.min(1, fraction))
          : 0;
      };
      const over = add(feet, [0, PIVOT_HEIGHT, 0]);
      const pivot = lerp(focus, over, cast(focus, over));
      const offset = sub(eye, pivot);
      const length = Math.hypot(...offset);
      if (!this.tracked || Math.hypot(...sub(pivot, this.tracked)) > 3)
        this.boom = Infinity;
      this.tracked = pivot;
      const free = cast(pivot, eye) * length;
      const wanted = Math.min(free, length);
      if (wanted < this.boom) this.boom = wanted;
      else if (Number.isFinite(this.boom)) {
        const gap = wanted - this.boom;
        this.boom += Math.min(
          gap,
          Math.max(gap * (1 - Math.exp(-4 * dt)), 1.5 * dt),
        );
      }
      if (this.boom >= length - 0.001) this.boom = Infinity;
      const distance = Math.min(free, this.boom, length);
      if (length > 0.00001 && distance < length - 0.00001) {
        eye = add(pivot, scale(offset, distance / length));
        limited = true;
        close = distance < 1;
      }
    }
    return {
      eye,
      direction: forward,
      near: limited || t > 0 ? 0.05 : 0.1,
      hideAvatar: close || t >= 0.8,
      limited,
      perspective: this.perspective,
      orbitDistance: this.orbitDistance,
      blend: t,
    };
  }
}
