import { viewAngles, type V3 } from "./math.ts";
import type { SourceVignetteOverride } from "../assets/src/scene.ts";
import {
  validateCaptureActorReferenceMetadata,
  type ActorCaptureReferenceMetadata,
} from "../runtime/capture-actors.ts";
import type { EnvironmentSettings } from "../editor-services/protocol.ts";
import {
  validateFilmTransforms,
  type FilmSourceTransform,
} from "./film-transforms.ts";
export const MAX_CAPTURE_BYTES = 32 * 1024 * 1024;
export interface FilmCameraPose {
  eye: V3;
  target: V3;
  up?: V3;
  /** Vertical field of view in radians; omitted inherits the selected lens. */
  fov?: number;
  /** Omitted inherits the selected camera; null explicitly removes its vignette. */
  vignette?: SourceVignetteOverride | null;
}
export interface FrameCaptureOptions {
  actorReference?: ActorCaptureReferenceMetadata;
  time?: number;
  camera?: FilmCameraPose;
  /** Absolute render exposure for this frame only; omitted inherits live lighting. */
  exposure?: number;
  transforms?: FilmSourceTransform[];
  format?: "png" | "rgba";
}
export type FramePixelOptions = Omit<FrameCaptureOptions, "format">;
export function validateFramePixelOptions(value: unknown): FramePixelOptions {
  keys(value, ["time", "camera", "exposure", "transforms", "actorReference"]);
  return validateFrameCaptureOptions(value);
}
export interface FilmCaptureSize {
  width: number;
  height: number;
}
function keys(
  value: unknown,
  allowed: readonly string[],
): asserts value is Record<string, unknown> {
  if (
    !value ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.keys(value).some((k) => !allowed.includes(k))
  )
    throw new Error("Choose supported frame capture options.");
}
function vector(value: unknown): asserts value is V3 {
  if (
    !Array.isArray(value) ||
    value.length !== 3 ||
    !value.every(
      (n) =>
        typeof n === "number" && Number.isFinite(n) && Math.abs(n) <= 20000,
    )
  )
    throw new Error("Choose a finite camera position and orientation.");
}
export function validateFrameCaptureOptions(
  value: unknown,
): FrameCaptureOptions {
  keys(value, [
    "time",
    "camera",
    "exposure",
    "transforms",
    "format",
    "actorReference",
  ]);
  if (value.actorReference !== undefined)
    validateCaptureActorReferenceMetadata(value.actorReference);
  if (value.transforms !== undefined) validateFilmTransforms(value.transforms);
  if (
    value.time !== undefined &&
    (typeof value.time !== "number" ||
      !Number.isFinite(value.time) ||
      value.time < 0 ||
      value.time > 86400)
  )
    throw new Error("Choose a screenshot time between zero and 24 hours.");
  if (
    value.exposure !== undefined &&
    (typeof value.exposure !== "number" ||
      !Number.isFinite(value.exposure) ||
      value.exposure < 0.01 ||
      value.exposure > 64)
  )
    throw new Error("Choose a frame exposure from 0.01 to 64.");
  if (
    value.format !== undefined &&
    value.format !== "png" &&
    value.format !== "rgba"
  )
    throw new Error("Choose PNG or raw RGBA capture.");
  if (value.camera !== undefined) {
    keys(value.camera, ["eye", "target", "up", "fov", "vignette"]);
    const c = value.camera;
    vector(c.eye);
    vector(c.target);
    const eye = c.eye,
      target = c.target;
    const direction = target.map((n, i) => n - eye[i]),
      length = Math.hypot(...direction);
    if (length < 0.01)
      throw new Error("Choose different camera and target positions.");
    if (
      c.fov !== undefined &&
      (typeof c.fov !== "number" ||
        !Number.isFinite(c.fov) ||
        c.fov < 0.15 ||
        c.fov > 2.5)
    )
      throw new Error(
        "Choose a camera field of view from 0.15 to 2.5 radians.",
      );
    if (c.up !== undefined) {
      vector(c.up);
      const up = c.up,
        n = Math.hypot(...up);
      const cross = [
        direction[1] * up[2] - direction[2] * up[1],
        direction[2] * up[0] - direction[0] * up[2],
        direction[0] * up[1] - direction[1] * up[0],
      ];
      if (n < 1e-6 || Math.hypot(...cross) / (n * length) < 1e-4)
        throw new Error(
          "Choose a camera up direction across the viewing direction.",
        );
    }
    if (c.vignette !== undefined && c.vignette !== null) {
      keys(c.vignette, ["shape", "intensity", "blendWeight"]);
      if (
        c.vignette.shape !== "cosine-fourth" ||
        ![c.vignette.intensity, c.vignette.blendWeight].every(
          (n) =>
            typeof n === "number" && Number.isFinite(n) && n >= 0 && n <= 1,
        )
      )
        throw new Error("Choose a supported camera vignette.");
    }
  }
  return value as FrameCaptureOptions;
}
export function validateFilmCaptureSize(value: unknown): FilmCaptureSize {
  keys(value, ["width", "height"]);
  if (
    !Number.isInteger(value.width) ||
    !Number.isInteger(value.height) ||
    Number(value.width) < 1 ||
    Number(value.width) > 1920 ||
    Number(value.height) < 1 ||
    Number(value.height) > 1080
  )
    throw new Error("Choose a film size up to 1920 by 1080 pixels.");
  return { width: Number(value.width), height: Number(value.height) };
}
interface RenderCamera {
  eye: V3;
  yaw: number;
  pitch: number;
  up?: V3;
  fov?: number;
  vignette?: SourceVignetteOverride;
}
/** A capture lens never changes the live camera, kernel, or selected source lens. */
export function cameraForCapture(
  base: RenderCamera,
  pose?: FilmCameraPose,
): RenderCamera {
  const result = {
    ...base,
    eye: [...base.eye] as V3,
    up: base.up ? ([...base.up] as V3) : undefined,
    vignette: base.vignette ? { ...base.vignette } : undefined,
  };
  if (!pose) return result;
  validateFrameCaptureOptions({ camera: pose });
  result.eye = [...pose.eye];
  Object.assign(result, viewAngles(pose.eye, pose.target));
  result.up = pose.up ? [...pose.up] : [0, 1, 0];
  result.fov = pose.fov ?? base.fov;
  if (pose.vignette !== undefined)
    result.vignette = pose.vignette ? { ...pose.vignette } : undefined;
  return result;
}
/** All render passes receive the same temporary exposure; live lighting is unchanged. */
export function environmentForCapture(
  base: EnvironmentSettings,
  exposure?: number,
): EnvironmentSettings {
  validateFrameCaptureOptions({ exposure });
  return {
    ...base,
    exposure: exposure ?? base.exposure,
  };
}
/** The worker uses this policy for live scheduling, input, and capture admission. */
export class FilmSessionState {
  private size?: FilmCaptureSize;
  private capturing = false;
  get manual() {
    return this.size !== undefined;
  }
  get inFlight() {
    return this.capturing;
  }
  get liveAllowed() {
    return !this.manual && !this.capturing;
  }
  begin(value: unknown) {
    if (this.manual || this.capturing)
      throw new Error("Finish the current capture first.");
    this.size = validateFilmCaptureSize(value);
    return { ...this.size };
  }
  enterFrame() {
    if (this.capturing)
      throw new Error("Let the current screenshot finish first.");
    this.capturing = true;
  }
  finishFrame() {
    this.capturing = false;
  }
  end() {
    if (this.capturing)
      throw new Error("Let the current screenshot finish first.");
    this.size = undefined;
  }
}
