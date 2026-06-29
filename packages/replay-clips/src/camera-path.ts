/**
 * Camera-path DSL for agent-directed replay clips (EPIC #5411, issue #5433).
 *
 * Gives an agent a compact, typed, bounded JSON grammar for directing a camera
 * over a replay without editing scene code. The DSL compiles into the existing
 * render-box camera-path input (the `{keyframes:[{second, mode, position,
 * target, fov}]}` shape consumed by `render-clip.mjs`), so it reuses the proven
 * renderer rather than introducing a parallel camera model.
 *
 * Boundary: parsing/compiling a camera path is pure data validation. It grants
 * no settlement, payout, deployment, accepted-work, provider, wallet, or
 * public-claim authority. Invalid paths fail closed with a useful error.
 *
 * The verbs mirror the proof-replay camera vocabulary so the compiled output is
 * something the existing three-effect scene already understands:
 * - `hold`        — stay on the current director track (no camera move).
 * - `orbit`       — orbit the proof/settlement focus.
 * - `follow`      — follow a named actor.
 * - `frame_actor` — frame a named actor (held framing).
 * - `frame_settlement` — frame the settlement/zap focus.
 */
import { Schema as S } from "effect"

export const REPLAY_CAMERA_PATH_SCHEMA_VERSION =
  "openagents.replay_camera_path.v1"

/** Hard bounds so an agent can never request an unbounded or absurd path. */
export const REPLAY_CAMERA_PATH_MAX_KEYFRAMES = 32
export const REPLAY_CAMERA_PATH_MAX_SECOND = 600
export const REPLAY_CAMERA_PATH_MIN_FOV = 10
export const REPLAY_CAMERA_PATH_MAX_FOV = 120
export const REPLAY_CAMERA_PATH_MAX_ABS_COORD = 1000

/** The bounded set of camera verbs an agent may emit. */
export const ReplayCameraVerb = S.Literals([
  "hold",
  "orbit",
  "follow",
  "frame_actor",
  "frame_settlement",
])
export type ReplayCameraVerb = typeof ReplayCameraVerb.Type

export const replayCameraVerbs: ReadonlyArray<ReplayCameraVerb> = [
  "hold",
  "orbit",
  "follow",
  "frame_actor",
  "frame_settlement",
]

/** Easing applied between this keyframe and the next. */
export const ReplayCameraEasing = S.Literals([
  "linear",
  "ease_in",
  "ease_out",
  "ease_in_out",
])
export type ReplayCameraEasing = typeof ReplayCameraEasing.Type

/**
 * A single DSL keyframe. `actorRef` is required by `follow`/`frame_actor` and
 * forbidden otherwise. `fov` is an optional bounded field-of-view override.
 */
export const ReplayCameraKeyframe = S.Struct({
  second: S.Number,
  verb: ReplayCameraVerb,
  actorRef: S.optional(S.String),
  fov: S.optional(S.Number),
  easing: S.optional(ReplayCameraEasing),
})
export type ReplayCameraKeyframe = typeof ReplayCameraKeyframe.Type

export const ReplayCameraPath = S.Struct({
  schemaVersion: S.Literal(REPLAY_CAMERA_PATH_SCHEMA_VERSION),
  keyframes: S.Array(ReplayCameraKeyframe),
})
export type ReplayCameraPath = typeof ReplayCameraPath.Type

export const decodeReplayCameraPath = S.decodeUnknownSync(ReplayCameraPath)

/**
 * The compiled camera-path shape the render box consumes. Matches the
 * `--camera <json>` keyframe contract of `render-clip.mjs`:
 * `{keyframes:[{second, mode, fov?}]}`. The render box resolves actor framing
 * from the bundle, so the compiled keyframe carries the renderer `mode` plus
 * the second and optional fov.
 */
export type CompiledCameraKeyframe = Readonly<{
  second: number
  mode:
    | "director_track"
    | "orbit_proof"
    | "follow_actor"
    | "zap_focus"
  fov?: number
}>

export type CompiledCameraPath = Readonly<{
  keyframes: ReadonlyArray<CompiledCameraKeyframe>
}>

const VERB_TO_RENDERER_MODE: Readonly<
  Record<ReplayCameraVerb, CompiledCameraKeyframe["mode"]>
> = {
  hold: "director_track",
  orbit: "orbit_proof",
  follow: "follow_actor",
  frame_actor: "follow_actor",
  frame_settlement: "zap_focus",
}

const verbRequiresActor = (verb: ReplayCameraVerb): boolean =>
  verb === "follow" || verb === "frame_actor"

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, value))

const unsafeCameraPathMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|bearer\s|mnemonic|oauth|preimage|private[_-]?key|secret|seed[_-]?phrase|wallet[._-]?(key|mnemonic|secret|seed))/i

/** True when a camera path payload contains raw/private material. */
export const replayCameraPathHasUnsafeMaterial = (value: unknown): boolean =>
  unsafeCameraPathMaterialPattern.test(JSON.stringify(value))

/**
 * Parse + validate a camera-path DSL value, clamping bounded numeric fields.
 * Fails closed with a useful message on structural, bounds, or actor-ref
 * errors. Returns the validated, clamped DSL value.
 */
export const parseReplayCameraPath = (input: unknown): ReplayCameraPath => {
  const path = decodeReplayCameraPath(input)

  if (replayCameraPathHasUnsafeMaterial(path)) {
    throw new Error("Replay camera path contains raw/private material")
  }

  if (path.keyframes.length === 0) {
    throw new Error("Replay camera path must provide at least one keyframe")
  }

  if (path.keyframes.length > REPLAY_CAMERA_PATH_MAX_KEYFRAMES) {
    throw new Error(
      `Replay camera path exceeds ${REPLAY_CAMERA_PATH_MAX_KEYFRAMES} keyframes`,
    )
  }

  const clampedKeyframes = path.keyframes.map((keyframe, index) => {
    if (!Number.isFinite(keyframe.second) || keyframe.second < 0) {
      throw new Error(
        `Replay camera keyframe ${index} second must be a non-negative number`,
      )
    }
    if (keyframe.second > REPLAY_CAMERA_PATH_MAX_SECOND) {
      throw new Error(
        `Replay camera keyframe ${index} second exceeds ${REPLAY_CAMERA_PATH_MAX_SECOND}`,
      )
    }

    if (verbRequiresActor(keyframe.verb)) {
      if (keyframe.actorRef === undefined || keyframe.actorRef.length === 0) {
        throw new Error(
          `Replay camera keyframe ${index} verb ${keyframe.verb} requires an actorRef`,
        )
      }
    } else if (keyframe.actorRef !== undefined) {
      throw new Error(
        `Replay camera keyframe ${index} verb ${keyframe.verb} must not carry an actorRef`,
      )
    }

    const fov =
      keyframe.fov === undefined
        ? undefined
        : (() => {
            if (!Number.isFinite(keyframe.fov)) {
              throw new Error(
                `Replay camera keyframe ${index} fov must be a finite number`,
              )
            }
            return clamp(
              keyframe.fov,
              REPLAY_CAMERA_PATH_MIN_FOV,
              REPLAY_CAMERA_PATH_MAX_FOV,
            )
          })()

    return ReplayCameraKeyframe.make({
      second: keyframe.second,
      verb: keyframe.verb,
      ...(keyframe.actorRef === undefined ? {} : { actorRef: keyframe.actorRef }),
      ...(fov === undefined ? {} : { fov }),
      ...(keyframe.easing === undefined ? {} : { easing: keyframe.easing }),
    })
  })

  const ordered = [...clampedKeyframes].sort(
    (left, right) => left.second - right.second,
  )

  return ReplayCameraPath.make({
    schemaVersion: REPLAY_CAMERA_PATH_SCHEMA_VERSION,
    keyframes: ordered,
  })
}

/**
 * Compile a validated camera-path DSL value into the render-box keyframe shape.
 * Throws if the input has not been validated (re-validates defensively).
 */
export const compileReplayCameraPath = (
  input: unknown,
): CompiledCameraPath => {
  const path = parseReplayCameraPath(input)

  return {
    keyframes: path.keyframes.map(keyframe => ({
      second: keyframe.second,
      mode: VERB_TO_RENDERER_MODE[keyframe.verb],
      ...(keyframe.fov === undefined ? {} : { fov: keyframe.fov }),
    })),
  }
}

/** Build a camera-path DSL value from a list of keyframe inputs. */
export const makeReplayCameraPath = (
  keyframes: ReadonlyArray<{
    second: number
    verb: ReplayCameraVerb
    actorRef?: string
    fov?: number
    easing?: ReplayCameraEasing
  }>,
): ReplayCameraPath =>
  parseReplayCameraPath({
    schemaVersion: REPLAY_CAMERA_PATH_SCHEMA_VERSION,
    keyframes,
  })
