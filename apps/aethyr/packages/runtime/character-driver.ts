import type { CharacterCamera } from "../renderer/character-camera.ts";
import type { Camera } from "../renderer/renderer.ts";
import type { CharacterRenderPose } from "../renderer/character.ts";
import type {
  CharacterLocomotion,
  CharacterPlayer,
} from "./character-state.ts";

export interface RuntimeRequests {
  request<T = any>(message: object): T;
}

/** Raw pointer pixels use the same sensitivity as the editor preview mouse input. */
export function pointerLook(dx: number, dy: number) {
  if (
    ![dx, dy].every(
      (value) => Number.isFinite(value) && Math.abs(value) <= 10_000,
    )
  )
    throw new Error("Use finite mouse movements within the supported range.");
  return { yawDelta: dx * 0.002, pitchDelta: -dy * 0.002 };
}

export function runtimeResult<
  T extends { ok: boolean; message?: string; error?: any },
>(result: T): T {
  if (!result.ok)
    throw new Error(
      result.message ??
        result.error?.message ??
        "The world could not apply this action.",
    );
  return result;
}

/** Shared controller-to-camera framing for preview and standalone play. */
export function updateCharacterFrame(
  kernel: RuntimeRequests,
  value: CharacterPlayer,
  mode: "walk" | "fly",
  camera: Camera,
  characterCamera: CharacterCamera,
  locomotion: CharacterLocomotion | undefined,
  groundY: number,
  elapsed: number,
): CharacterRenderPose | undefined {
  if (mode === "walk") {
    characterCamera.pitch = Math.max(-1.45, Math.min(1.45, value.pitch));
    const framed = characterCamera.frame(
      value.position,
      value.yaw,
      (from, to, radius) =>
        runtimeResult(
          kernel.request({ type: "query_camera", from, to, radius }),
        ).fraction,
      groundY,
      elapsed,
    );
    camera.eye = framed.eye;
    camera.yaw = value.yaw + characterCamera.yawOffset;
    camera.pitch = characterCamera.pitch;
    camera.near = framed.near;
    return locomotion?.update(value, elapsed, !framed.hideAvatar);
  }
  camera.eye = value.eye;
  camera.yaw = value.yaw;
  camera.pitch = value.pitch;
  camera.near = undefined;
  return undefined;
}

/** One fixed-step admission boundary; the Rust kernel owns time and collision. */
export function stepRuntime(
  kernel: RuntimeRequests,
  elapsed: number,
  input: object,
) {
  return runtimeResult(kernel.request({ type: "step", elapsed, input }));
}
