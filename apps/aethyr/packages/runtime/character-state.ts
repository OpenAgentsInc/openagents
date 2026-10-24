import type {
  CharacterManifest,
  CharacterMotion,
  CharacterRenderPose,
} from "../renderer/character.ts";
import type { CharacterStatus } from "../editor-services/protocol.ts";
import type { V3 } from "../renderer/math.ts";

export interface CharacterPlayer {
  position: V3;
  eye: V3;
  yaw: number;
  pitch: number;
  grounded: boolean;
  velocity: V3;
  speed: number;
  ahead: number;
  side: number;
}

export function characterAnimation(
  player: CharacterPlayer,
): CharacterStatus["animation"] {
  if (!player.grounded) return player.velocity[1] > 0.05 ? "jump" : "fall";
  if (player.speed < 0.05) return "idle";
  return player.speed < 2.8 ? "walk" : "run";
}

/** The admitted distance drives footfalls; blocked movement leaves the avatar idle. */
export class CharacterLocomotion {
  private previous?: V3;
  private motion: CharacterMotion = "idle";
  private phase = 0;
  constructor(private clips: CharacterManifest["clips"]) {}
  update(
    player: CharacterPlayer,
    elapsed: number,
    visible: boolean,
  ): CharacterRenderPose {
    const state = characterAnimation(player);
    const motion: CharacterMotion =
      state === "fall"
        ? "jump"
        : state === "idle" || state === "jump"
          ? state
          : player.ahead < -0.1
            ? "backpedal"
            : Math.abs(player.side) > Math.abs(player.ahead)
              ? player.side < 0
                ? "strafe_left"
                : "strafe_right"
              : state;
    if (motion !== this.motion) {
      this.motion = motion;
      this.phase = 0;
    }
    const clip = this.clips.find((clip) => clip.name === motion)!;
    const distance = this.previous
      ? Math.hypot(
          player.position[0] - this.previous[0],
          player.position[2] - this.previous[2],
        )
      : 0;
    if (clip.distance > 0)
      this.phase +=
        ((distance < 3 ? distance : 0) * clip.duration) / clip.distance;
    else this.phase += Math.max(0, Math.min(elapsed, 0.1));
    this.previous = [...player.position];
    return {
      position: [...player.position],
      yaw: Math.PI - player.yaw,
      animation: motion,
      time:
        motion === "jump"
          ? Math.min(this.phase, clip.duration - 1e-5)
          : this.phase,
      visible,
    };
  }
}
