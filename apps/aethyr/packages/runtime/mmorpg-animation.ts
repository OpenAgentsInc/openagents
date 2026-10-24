// Animation timing adapted from Quick_3D_MMORPG. See third_party/quick-mmorpg/NOTICE.
import type { MmorpgActorPose } from "../assets/src/mmorpg.ts";
import type {
  CharacterBlend,
  CharacterManifest,
} from "../renderer/character.ts";
import {
  QUICK_MMORPG_MOTIONS,
  quickMmorpgClipTime,
  quickMmorpgFrameDelta,
  type QuickMmorpgMotion,
} from "./quick-mmorpg-controller.ts";

export type MmorpgAnimationSample = Pick<
  MmorpgActorPose,
  "state" | "actionTime" | "animationTime"
>;
export interface MmorpgAnimationPose {
  animation: QuickMmorpgMotion;
  time: number;
  blend?: CharacterBlend;
}

/** Source player-state.js durations, selected by the accepted incoming state. */
export const MMORPG_ANIMATION_FADES: Readonly<
  Record<QuickMmorpgMotion, number>
> = Object.freeze({
  idle: 0.25,
  walk: 0.1,
  run: 0.1,
  attack: 0.4,
  dance: 0.2,
  death: 0.2,
});
const oneShot = (state: QuickMmorpgMotion) =>
  state === "attack" || state === "dance" || state === "death";
interface Envelope {
  start: number;
  end: number;
  from: number;
  to: number;
}
interface Action {
  motion: QuickMmorpgMotion;
  time: number;
  enabled: boolean;
  paused: boolean;
  scale: number;
  weight: number;
  fade?: Envelope;
  warp?: Envelope;
}
// Three r124 stores control-interpolant times, values, and evaluated results as Float32.
const envelope = (
  now: number,
  duration: number,
  from: number,
  to: number,
): Envelope => ({
  start: Math.fround(now),
  end: Math.fround(now + duration),
  from: Math.fround(from),
  to: Math.fround(to),
});
function evaluate(value: Envelope, now: number) {
  const t = Math.min(
    1,
    Math.max(0, (now - value.start) / (value.end - value.start)),
  );
  return Math.fround(value.from * (1 - t) + value.to * t);
}

/** Presentation only: accepted states drive transitions; clip completion never does. */
export class MmorpgAnimationClock {
  private durations = new Map<QuickMmorpgMotion, number>();
  // Original mixer actions remain active after fading out. First activation fixes blend order.
  private actions = new Map<QuickMmorpgMotion, Action>();
  private current?: QuickMmorpgMotion;
  private previous?: QuickMmorpgMotion;
  private mixerTime = 0;
  private lastClock?: number;
  private lastAcceptedTime = 0;

  constructor(manifest: Pick<CharacterManifest, "clips">) {
    if (!manifest || !Array.isArray(manifest.clips))
      throw Error("Prepare the character animation clips.");
    for (const clip of manifest.clips) {
      if (!(QUICK_MMORPG_MOTIONS as readonly string[]).includes(clip.name))
        continue;
      const name = clip.name as QuickMmorpgMotion;
      if (
        !Number.isFinite(clip.duration) ||
        clip.duration <= 0 ||
        this.durations.has(name)
      )
        throw Error("Prepare one valid clip for each character action.");
      this.durations.set(name, clip.duration);
    }
    if (
      ["idle", "walk", "attack", "death"].some(
        (name) => !this.durations.has(name as QuickMmorpgMotion),
      )
    )
      throw Error("Prepare idle, walk, attack, and death animations.");
  }

  reset() {
    this.actions.clear();
    this.current = this.previous = undefined;
    this.mixerTime = this.lastAcceptedTime = 0;
    this.lastClock = undefined;
  }

  private action(motion: QuickMmorpgMotion) {
    let action = this.actions.get(motion);
    if (!action) {
      action = {
        motion,
        time: 0,
        enabled: true,
        paused: false,
        scale: 1,
        weight: 1,
      };
      this.actions.set(motion, action);
    }
    return action;
  }

  private clipTime(motion: QuickMmorpgMotion, time: number) {
    return quickMmorpgClipTime(motion, time, this.durations.get(motion)!);
  }

  private enter(motion: QuickMmorpgMotion, acceptedTime: number, dt: number) {
    const from = this.action(this.current!),
      target = this.action(motion);
    const phase =
      (from.motion === "walk" && motion === "run") ||
      (from.motion === "run" && motion === "walk");
    if (oneShot(motion)) {
      // AnimationAction.reset clears pause/fade/warp, but preserves its base time scale.
      target.paused = false;
      target.fade = target.warp = undefined;
    } else if (!phase) {
      target.scale = 1;
      target.fade = target.warp = undefined;
    }
    target.enabled = true;
    target.time = phase
      ? (from.time * this.durations.get(motion)!) /
        this.durations.get(from.motion)!
      : Math.max(0, acceptedTime - dt);
    const duration = MMORPG_ANIMATION_FADES[motion],
      fromDuration = this.durations.get(from.motion)!,
      toDuration = this.durations.get(motion)!;
    from.fade = envelope(this.mixerTime, duration, 1, 0);
    target.fade = envelope(this.mixerTime, duration, 0, 1);
    from.warp = envelope(
      this.mixerTime,
      duration,
      1 / from.scale,
      fromDuration / toDuration / from.scale,
    );
    target.warp = envelope(
      this.mixerTime,
      duration,
      toDuration / fromDuration / target.scale,
      1 / target.scale,
    );
    this.previous = from.motion;
    this.current = motion;
  }

  private update(dt: number) {
    this.mixerTime += dt;
    for (const action of this.actions.values()) {
      if (!action.enabled) {
        action.weight = 0;
        continue;
      }
      let scale = action.paused ? 0 : action.scale;
      if (!action.paused && action.warp) {
        scale *= evaluate(action.warp, this.mixerTime);
        if (this.mixerTime > action.warp.end) {
          action.warp = undefined;
          if (scale === 0) action.paused = true;
          else action.scale = scale;
        }
      }
      action.time = this.clipTime(action.motion, action.time + dt * scale);
      if (
        oneShot(action.motion) &&
        action.time >= this.durations.get(action.motion)!
      )
        action.paused = true;
      action.weight = action.fade ? evaluate(action.fade, this.mixerTime) : 1;
      if (action.fade && this.mixerTime > action.fade.end) {
        action.fade = undefined;
        if (action.weight === 0) action.enabled = false;
      }
    }
  }

  sample(
    accepted: MmorpgAnimationSample,
    elapsed: number,
    frameClock: number,
  ): MmorpgAnimationPose {
    const capped = quickMmorpgFrameDelta(elapsed);
    if (
      !accepted ||
      !this.durations.has(accepted.state) ||
      ![accepted.actionTime, accepted.animationTime].every(
        (time) => Number.isFinite(time) && time >= 0 && time <= 1e12,
      ) ||
      !Number.isFinite(frameClock) ||
      Math.abs(frameClock) > 1e12
    )
      throw Error("Use a complete accepted character animation.");
    const motion = accepted.state,
      acceptedTime = oneShot(motion)
        ? accepted.actionTime
        : accepted.animationTime;
    if (
      this.current === undefined ||
      (this.lastClock !== undefined && frameClock < this.lastClock)
    ) {
      this.reset();
      this.current = motion;
      this.action(motion).time = this.clipTime(motion, acceptedTime);
    } else {
      const dt = frameClock === this.lastClock ? 0 : capped;
      const restart =
        motion === this.current &&
        oneShot(motion) &&
        acceptedTime + 0.05 < this.lastAcceptedTime;
      if (motion !== this.current || restart)
        this.enter(motion, acceptedTime, dt);
      this.update(dt);
    }
    this.lastClock = frameClock;
    this.lastAcceptedTime = acceptedTime;
    const current = this.action(motion);
    const layers = [...this.actions.values()]
      .filter((action) => action.weight > 0)
      .map((action) => ({
        animation: action.motion,
        time: action.time,
        weight: action.weight,
      }));
    const blending =
      layers.length !== 1 ||
      layers[0].animation !== motion ||
      layers[0].weight !== 1;
    const previous = this.action(this.previous ?? motion);
    return {
      animation: motion,
      time: current.time,
      ...(blending
        ? {
            blend: {
              from: previous.motion,
              fromTime: previous.time,
              weight: current.weight,
              layers,
            },
          }
        : {}),
    };
  }
}
