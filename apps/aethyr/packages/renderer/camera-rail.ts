// SPDX-License-Identifier: Apache-2.0
// Manually adapted from OpenAgents camera_rig.rs and film_shot.rs at
// 83fe8b7f6b3cd437ae9b7af213cf9f04875dd793 (OpenAgents, Apache-2.0):
// https://github.com/OpenAgentsInc/openagents/blob/83fe8b7f6b3cd437ae9b7af213cf9f04875dd793/crates/verse-engine/src/camera_rig.rs
// Integer timing and transported up/roll are original Aethyrforge extensions.

export type CameraRailVector = readonly [number, number, number];
export type CameraRailEase = "linear" | "in" | "out" | "in_out";
export interface CameraRailKey {
  /** Shot-local integer microseconds. Separate shots use separate rails. */
  timeUs: number;
  position: CameraRailVector;
  target: CameraRailVector;
  verticalFovDegrees: number;
  /** Defaults to world +Y; its component along the viewing direction is ignored. */
  up?: CameraRailVector;
  /** Unwrapped degrees, positive by the right-hand rule about eye-to-target. */
  rollDegrees?: number;
  /** Easing from this key to the next; linear means parameter speed. */
  ease?: CameraRailEase;
}
export interface CameraRail {
  keys: readonly CameraRailKey[];
}
export interface CameraRailSample {
  position: CameraRailVector;
  target: CameraRailVector;
  /** Unit, perpendicular to the view direction, with roll already applied. */
  up: CameraRailVector;
  verticalFovDegrees: number;
}
export interface CameraRailFrameRate {
  numerator: number;
  denominator: number;
}
export const CAMERA_RAIL_SENSOR_HEIGHT_MM = 20.25;
export const CAMERA_RAIL_MAX_TIME_US = 86_400_000_000;
const MAX_KEYS = 4096;
const MAX_POSITION = 20_000;
const EPSILON = 1e-8;
const prepared = Symbol("prepared-camera-rail");
interface PreparedKey {
  readonly timeUs: number;
  readonly position: CameraRailVector;
  readonly target: CameraRailVector;
  readonly forward: CameraRailVector;
  readonly up: CameraRailVector;
  readonly verticalFovDegrees: number;
  readonly logFocal: number;
  readonly rollDegrees: number;
  readonly ease: CameraRailEase;
}
export interface PreparedCameraRail {
  readonly [prepared]: true;
  readonly keys: readonly PreparedKey[];
}

function dot(a: CameraRailVector, b: CameraRailVector) {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}
function add(a: CameraRailVector, b: CameraRailVector): CameraRailVector {
  return [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
}
function scale(a: CameraRailVector, k: number): CameraRailVector {
  return [a[0] * k, a[1] * k, a[2] * k];
}
function subtract(a: CameraRailVector, b: CameraRailVector): CameraRailVector {
  return add(a, scale(b, -1));
}
function cross(a: CameraRailVector, b: CameraRailVector): CameraRailVector {
  return [
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  ];
}
function unit(a: CameraRailVector, name: string): CameraRailVector {
  const length = Math.hypot(...a);
  if (!Number.isFinite(length) || length < EPSILON)
    throw new RangeError(`${name} needs a nonzero finite direction.`);
  return scale(a, 1 / length);
}
function vector(value: CameraRailVector, name: string, bound: number) {
  if (
    !Array.isArray(value) ||
    value.length !== 3 ||
    ![...value].every((v) => Number.isFinite(v) && Math.abs(v) <= bound)
  )
    throw new RangeError(`${name} needs three finite values within ${bound}.`);
  return Object.freeze([...value]) as unknown as CameraRailVector;
}
function perpendicularUp(up: CameraRailVector, forward: CameraRailVector) {
  const normalized = unit(up, "Camera up");
  const projected = subtract(
    normalized,
    scale(forward, dot(normalized, forward)),
  );
  if (Math.hypot(...projected) < 1e-6)
    throw new RangeError(
      "Camera up must not be parallel to its view direction.",
    );
  return unit(projected, "Camera up");
}
function time(value: number, allowNegative = false) {
  if (
    !Number.isSafeInteger(value) ||
    value > CAMERA_RAIL_MAX_TIME_US ||
    value < (allowNegative ? -CAMERA_RAIL_MAX_TIME_US : 0)
  )
    throw new RangeError("Camera time needs bounded integer microseconds.");
}

export function cameraRailEase(ease: CameraRailEase, progress: number) {
  if (!Number.isFinite(progress))
    throw new RangeError("Camera progress must be finite.");
  const u = Math.max(0, Math.min(1, progress));
  switch (ease) {
    case "linear":
      return u;
    case "in":
      return u * u * u;
    case "out":
      return 1 - (1 - u) ** 3;
    case "in_out":
      return u * u * u * (u * (6 * u - 15) + 10);
    default:
      throw new RangeError("Choose a supported camera easing curve.");
  }
}

/** Copy and validate a single shot's rail; no input arrays remain mutable. */
export function prepareCameraRail(input: CameraRail): PreparedCameraRail {
  if (
    !input ||
    !Array.isArray(input.keys) ||
    !input.keys.length ||
    input.keys.length > MAX_KEYS
  )
    throw new RangeError(`A camera rail needs one to ${MAX_KEYS} keys.`);
  const keys = Array.from(input.keys, (key, i) => {
    if (!key || typeof key !== "object")
      throw new RangeError("Camera key is missing.");
    time(key.timeUs);
    if (i > 0 && key.timeUs <= input.keys[i - 1].timeUs)
      throw new RangeError("Camera key times must increase strictly.");
    const position = vector(key.position, "Camera position", MAX_POSITION);
    const target = vector(key.target, "Camera target", MAX_POSITION);
    const forward = unit(subtract(target, position), "Camera view");
    const up = perpendicularUp(
      vector(key.up ?? [0, 1, 0], "Camera up", 1e6),
      forward,
    );
    const fov = key.verticalFovDegrees;
    if (!Number.isFinite(fov) || fov < 0.1 || fov > 179)
      throw new RangeError(
        "Camera field of view must be from 0.1 to 179 degrees.",
      );
    const rollDegrees = key.rollDegrees ?? 0;
    if (!Number.isFinite(rollDegrees) || Math.abs(rollDegrees) > 36_000)
      throw new RangeError("Camera roll must be finite and within 100 turns.");
    const ease = key.ease ?? "linear";
    cameraRailEase(ease, 0);
    return Object.freeze({
      timeUs: key.timeUs,
      position,
      target,
      forward: Object.freeze(forward),
      up: Object.freeze(up),
      verticalFovDegrees: fov,
      logFocal: Math.log(
        CAMERA_RAIL_SENSOR_HEIGHT_MM / (2 * Math.tan((fov * Math.PI) / 360)),
      ),
      rollDegrees,
      ease,
    });
  });
  return Object.freeze({
    [prepared]: true as const,
    keys: Object.freeze(keys),
  });
}

function centripetal(points: readonly CameraRailVector[], u: number) {
  const [p0, p1, p2, p3] = points;
  if (Math.hypot(...subtract(p1, p2)) < EPSILON) return p1;
  const knot = (a: CameraRailVector, b: CameraRailVector) =>
    Math.max(1e-4, Math.sqrt(Math.hypot(...subtract(a, b))));
  const t0 = 0,
    t1 = knot(p0, p1),
    t2 = t1 + knot(p1, p2),
    t3 = t2 + knot(p2, p3);
  const t = t1 + (t2 - t1) * u;
  const blend = (
    a: CameraRailVector,
    b: CameraRailVector,
    start: number,
    end: number,
  ) =>
    add(
      scale(a, (end - t) / (end - start)),
      scale(b, (t - start) / (end - start)),
    );
  const a = blend(p0, p1, t0, t1),
    b = blend(p1, p2, t1, t2),
    c = blend(p2, p3, t2, t3);
  return blend(blend(a, b, t0, t2), blend(b, c, t1, t3), t1, t2);
}
function rotate(v: CameraRailVector, axis: CameraRailVector, angle: number) {
  return add(
    add(scale(v, Math.cos(angle)), scale(cross(axis, v), Math.sin(angle))),
    scale(axis, dot(axis, v) * (1 - Math.cos(angle))),
  );
}
/** Carry an up vector by the shortest rotation from its key's view to this view. */
function transportUp(key: PreparedKey, forward: CameraRailVector) {
  const c = Math.max(-1, Math.min(1, dot(key.forward, forward)));
  if (c < -1 + 1e-10) return perpendicularUp(key.up, forward);
  const axis = cross(key.forward, forward);
  return perpendicularUp(
    add(
      add(key.up, cross(axis, key.up)),
      scale(cross(axis, cross(axis, key.up)), 1 / (1 + c)),
    ),
    forward,
  );
}

/** Random-access sampling; no integration, frame history, clock, or randomness. */
export function sampleCameraRail(
  rail: PreparedCameraRail,
  timeUs: number,
): CameraRailSample {
  if (!rail || rail[prepared] !== true)
    throw new RangeError("Prepare the camera rail before sampling.");
  time(timeUs, true);
  const keys = rail.keys;
  let low = 0,
    high = keys.length - 1;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (keys[middle].timeUs <= timeUs) low = middle;
    else high = middle - 1;
  }
  const i = low;
  const a = keys[i],
    b = keys[Math.min(i + 1, keys.length - 1)];
  const u =
    a === b
      ? 0
      : cameraRailEase(a.ease, (timeUs - a.timeUs) / (b.timeUs - a.timeUs));
  const point = (field: "position" | "target") => {
    if (u === 0) return [...a[field]] as unknown as CameraRailVector;
    if (u === 1) return [...b[field]] as unknown as CameraRailVector;
    const p1 = a[field],
      p2 = b[field];
    return centripetal(
      [
        i === 0 ? subtract(scale(p1, 2), p2) : keys[i - 1][field],
        p1,
        p2,
        i + 2 >= keys.length ? subtract(scale(p2, 2), p1) : keys[i + 2][field],
      ],
      u,
    );
  };
  const position = vector(
    point("position"),
    "Sampled camera position",
    MAX_POSITION,
  );
  const target = vector(point("target"), "Sampled camera target", MAX_POSITION);
  const forward = unit(subtract(target, position), "Sampled camera view");
  const firstUp = transportUp(a, forward),
    lastUp = transportUp(b, forward);
  const cosine = Math.max(-1, Math.min(1, dot(firstUp, lastUp)));
  const sine = dot(forward, cross(firstUp, lastUp));
  const angle = cosine < -1 + 1e-10 ? Math.PI : Math.atan2(sine, cosine);
  const baseUp = rotate(firstUp, forward, angle * u);
  const roll =
    ((a.rollDegrees + (b.rollDegrees - a.rollDegrees) * u) * Math.PI) / 180;
  const up = perpendicularUp(rotate(baseUp, forward, roll), forward);
  const focal = Math.exp(a.logFocal + (b.logFocal - a.logFocal) * u);
  return {
    position,
    target,
    up,
    verticalFovDegrees:
      u === 0
        ? a.verticalFovDegrees
        : u === 1
          ? b.verticalFovDegrees
          : (360 / Math.PI) *
            Math.atan(CAMERA_RAIL_SENSOR_HEIGHT_MM / (2 * focal)),
  };
}

/** Nearest integer microsecond PTS (ties later), calculated without float accumulation. */
export function frameIndexToPtsUs(
  frameIndex: number,
  rate: CameraRailFrameRate,
): number {
  if (
    !Number.isSafeInteger(frameIndex) ||
    frameIndex < 0 ||
    !rate ||
    !Number.isSafeInteger(rate.numerator) ||
    !Number.isSafeInteger(rate.denominator) ||
    rate.numerator < 1 ||
    rate.denominator < 1 ||
    rate.numerator > 1_000_000 ||
    rate.denominator > 1_000_000
  )
    throw new RangeError(
      "Use a nonnegative frame index and bounded positive integer frame rate.",
    );
  const n = BigInt(rate.numerator),
    d = BigInt(rate.denominator),
    frame = BigInt(frameIndex);
  if (n < d || n > 240n * d)
    throw new RangeError(
      "Frame rate must be from one to 240 frames per second.",
    );
  if (frame * d > 86_400n * n)
    throw new RangeError("Frame time exceeds one day.");
  return Number((frame * d * 1_000_000n + n / 2n) / n);
}
