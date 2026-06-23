// Verse spawned isolated-scene layer (dev affordance — #6033 / EPIC #6017).
//
// PURE, Three.js-free transform that drops an ISOLATED scene slice (the Khala
// "crackling energy" inference effect, and an optional gateway portal) into the
// SAME live Verse world the Autopilot avatar already walks in — at a fixed
// in-world "scene station" near spawn. The point is to develop/eyeball one
// effect at a time inside the real world geometry: you spawn it with a key, then
// walk up to it with the existing third-person character controller and look at
// it from any angle, instead of opening a separate standalone pane.
//
// ISOLATED / HONEST. The scene is fed by a SYNTHETIC, simulated inference event
// (`simulated:true`, `evidenceMode:"optional"`, fixed `sourceRefs`) — there is
// NO Region Durable Object, NO D1, NO Worker, and NO live receipt behind it.
// This mirrors the standalone crackling-arc demo's contract exactly
// (three-effect/examples/crackling-arc-standalone + the audit doc
// 2026-06-22-isolated-verse-scene-harness-audit.md), but renders THROUGH the
// same `trainingRunView` renderer as the rest of the Verse: a
// `style:"crackling_arc"` beam becomes `createEvidenceBackedCracklingArc`, and a
// `visualKind:"gateway_portal"` entity becomes `createEvidenceBackedGatewayPortal`
// inside the live scene (trainingRun.ts). We reuse the SAME scene/render setup —
// no parallel renderer.
//
// EVIDENCE-BOUND, even though simulated. Each spawned beam/burst carries its
// `sourceRefs` + `motionKind` + `simulated:true`, and the layer forces
// `motionPolicy.evidence = "required"`, so nothing animates without the synthetic
// event — exactly like withVerseKhalaEffectLayer, the local sibling of this layer.
//
// EXTENSIBLE. Scenes are registered in VERSE_SPAWNABLE_SCENES; spawning is keyed
// by registry id, so more isolated scenes can be added + spawned the same way.

import { verseIconRecipeForId } from "@openagentsinc/three-effect/core"
import type {
  InferenceGatewayLane,
  TrainingRunBeamDefinition,
  TrainingRunBurstDefinition,
  TrainingRunVector,
  TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"

import {
  appendVerseVisualization,
  roundedVerseVector,
  verseSceneWorldToRootLocal,
  VERSE_ROOT_SCALE,
} from "./verse-scene-helpers.js"
// Re-export so existing importers (and tests) keep their import path.
export { verseSceneWorldToRootLocal } from "./verse-scene-helpers.js"
import type { ChatWorldVisualEntityDefinition } from "./chat-world-visualization.js"

// Stable scene-node prefix so a spawned isolated scene never collides on id with
// the live world / payment / Khala layers. Each spawned instance is keyed by its
// registry scene id (one fixed station per scene id), so respawning is idempotent.
export const VERSE_SPAWNED_SCENE_NODE_PREFIX = "verse:spawned:"

// The synthetic source ref every spawned scene carries — a real public ref (this
// issue) so the on-screen evidence is honest about what authorizes the motion,
// while `simulated:true` keeps it explicitly labelled as a developer simulation.
export const VERSE_SPAWNED_SCENE_SOURCE_REF =
  "github:OpenAgentsInc/openagents#6033"

// ── THE COORDINATE FRAME (the real no-particle-effect bug) ────────────────────
//
// The renderer (three-effect trainingRun.ts, perspective_walk) builds the WALKABLE
// world in two DIFFERENT spaces:
//
//   • The avatar + camera live in SCENE-WORLD space (the third-person controller's
//     `getPosition`, which is what `verseSceneRestorePose` captures). The camera
//     follows the avatar there. "Forward" for the avatar is -Z, "up" is +Y, "right"
//     is +X — plain scene-world axes.
//
//   • Entity / beam positions are added to a `root` group that is ROTATED -90° about
//     X, SCALED by `tassadarSceneScale`, and TRANSLATED to the Tassadar street lot
//     (`tassadarLotX/Z`). So a beam endpoint position is interpreted in ROOT-LOCAL
//     space, NOT scene-world space.
//
// The old code placed the arc endpoints using the avatar's scene-world pose AS IF
// they were root-local — so after the root rotate+scale+offset they landed ~5 units
// up in the air and ~3-8 units to the side, far outside the camera's view. The arc
// DID render; you just never saw it. (Earlier "chest-height" tuning only moved it
// around inside the wrong frame.) The sibling `withVerseKhalaEffectLayer` has the
// SAME latent bug; it only looks fine because its nexus is pinned near root-local
// origin, which happens to fall on the lot.
//
// THE FIX: author the arc in SCENE-WORLD, in front of the avatar, then convert each
// endpoint to ROOT-LOCAL with the inverse of the documented root transform before
// handing it to the renderer. Now the arc lands exactly where the avatar is looking.
// How far IN FRONT of the avatar (along its facing/-Z by default) the scene
// station is dropped, plus how high the arc hangs, in SCENE-WORLD units. Close
// enough to fill the avatar's view, far enough not to clip the camera.
export const VERSE_SPAWNED_SCENE_AVATAR_FORWARD = 3.0
export const VERSE_SPAWNED_SCENE_AVATAR_HEIGHT = 1.4

// Fallback avatar (scene-world) when no pose has been captured yet: the third-
// person controller's own default spawn. Keeps the arc in front of the camera on
// the very first spawn, before any pose round-trips back into the model.
export const VERSE_SPAWNED_SCENE_DEFAULT_AVATAR: VerseSpawnedSceneAvatarAnchor = {
  x: 0,
  y: 0,
  z: 4.4,
  yaw: 0,
}

// Fallback scene station (ROOT-LOCAL): the default avatar's scene-world spawn,
// pushed forward + lifted to chest height, then converted to root-local — so the
// pose-less spawn still lands in the camera's view rather than at a fixed far lot.
export const VERSE_SPAWNED_SCENE_STATION_POSITION: TrainingRunVector =
  verseSceneWorldToRootLocal([
    VERSE_SPAWNED_SCENE_DEFAULT_AVATAR.x,
    VERSE_SPAWNED_SCENE_AVATAR_HEIGHT,
    VERSE_SPAWNED_SCENE_DEFAULT_AVATAR.z - VERSE_SPAWNED_SCENE_AVATAR_FORWARD,
  ])

// ── Eyeball knobs (mirrors the standalone's URL knobs as typed options) ───────

// The crackling-arc knobs the standalone scene exposes (strandCount/rate/color/
// endpoints). Defaults match crackling-arc-standalone so the in-world spawn reads
// the same as the isolated page. Endpoint offsets are RELATIVE to the station
// anchor, in SCENE-WORLD avatar axes: X = screen-right, Y = up/height, Z = forward
// (toward where the avatar faces). They are converted to root-local with the
// station, so the arc reads correctly from the avatar's point of view.
export type VerseSpawnedSceneKnobs = Readonly<{
  /** arc start offset (scene-world avatar axes: X=right, Y=up, Z=forward). */
  fromOffset?: TrainingRunVector
  /** arc end offset (scene-world avatar axes: X=right, Y=up, Z=forward). */
  toOffset?: TrainingRunVector
  strandCount?: number
  rate?: number
  opacity?: number
  /** per-strand wobble amplitude — higher reads as more violent crackling. */
  jitter?: number
  color?: number
  secondaryColor?: number
  /** also spawn the gateway-portal variant just below the arc (2nd toggle). */
  showPortal?: boolean
  /** lane color/labeling for the optional portal. */
  portalLane?: InferenceGatewayLane
}>

// A registered spawnable scene. `id` is the stable key; `motionKind` is the
// public meaning the evidence overlay shows; `build` returns the in-world layer.
export type VerseSpawnableScene = Readonly<{
  id: string
  label: string
  motionKind: string
  defaultKnobs: VerseSpawnedSceneKnobs
}>

// The crackling-energy scene defaults. Dialed UP from the standalone demo so the
// arc reads as REAL crackling energy in the dark live Verse, not a faint hairline
// at a far station (the #6033 owner report). The arc spans ~3.2 units wide and
// hangs at chest/head height in front of the avatar; the strand count, opacity,
// and jitter are pushed well above the renderer's faint default.
// Offsets are RELATIVE to the (avatar-derived) station, in SCENE-WORLD avatar
// axes: X = screen width (left/right), Y = HEIGHT (so the arc hangs at chest/head
// height), Z = forward depth (a slight near/far stagger so the arc has a little
// depth toward the avatar). Converted to root-local with the station.
const CRACKLING_ENERGY_DEFAULTS: VerseSpawnedSceneKnobs = {
  fromOffset: [-1.6, 0, 0.2],
  toOffset: [1.6, 0, -0.2],
  strandCount: 11,
  rate: 2.6,
  opacity: 0.95,
  jitter: 0.34,
  color: 0x93c5fd,
  secondaryColor: 0xf8fafc,
  showPortal: false,
  portalLane: "openrouter",
} as const

// The registry. Start with crackling-energy (+ a portal toggle); add more here.
export const VERSE_SPAWNABLE_SCENES: ReadonlyArray<VerseSpawnableScene> = [
  {
    id: "crackling-energy",
    label: "Crackling energy",
    motionKind: "crackling_energy",
    defaultKnobs: CRACKLING_ENERGY_DEFAULTS,
  },
] as const

export const DEFAULT_SPAWNABLE_SCENE_ID = "crackling-energy"

export const verseSpawnableSceneById = (
  id: string,
): VerseSpawnableScene | null =>
  VERSE_SPAWNABLE_SCENES.find((scene) => scene.id === id) ?? null

// ── The spawned-scene layer ───────────────────────────────────────────────────

export type VerseSpawnedSceneLayer = Readonly<{
  entities: ReadonlyArray<ChatWorldVisualEntityDefinition>
  beams: ReadonlyArray<TrainingRunBeamDefinition>
  bursts: ReadonlyArray<TrainingRunBurstDefinition>
}>

const EMPTY_LAYER: VerseSpawnedSceneLayer = { entities: [], beams: [], bursts: [] }

const finite = (value: number): boolean =>
  Number.isFinite(value) && !Number.isNaN(value)

// Apply an avatar-axis offset (X=right, Y=up, Z=forward) to a SCENE-WORLD anchor,
// rotating the X/Z (ground) part by the avatar's yaw so "right"/"forward" track
// the avatar's facing. Y (height) is yaw-invariant. Avatar faces -Z at yaw 0.
const addSceneWorldOffset = (
  anchorWorld: TrainingRunVector,
  offset: TrainingRunVector,
  yaw: number,
): TrainingRunVector => {
  const cos = Math.cos(yaw)
  const sin = Math.sin(yaw)
  const right = offset[0]
  const up = offset[1]
  const forward = offset[2]
  // Avatar faces -Z at yaw 0; yaw rotates facing in the X/Z plane. "forward" is
  // -Z, "right" is +X (rotated by yaw). Mirrors the third-person controller frame.
  const dx = right * cos + forward * sin
  const dz = -forward * cos + right * sin
  return roundedVerseVector([
    anchorWorld[0] + dx,
    anchorWorld[1] + up,
    anchorWorld[2] + dz,
  ])
}

// One spawned scene's input: which registry scene, where its station sits, and
// the (optional) knob overrides. `generatedAt` rides the evidence so the renderer
// can derive motion timing; the synthetic source ref is the evidence.
// The avatar's last pose, used to drop the scene station RIGHT IN FRONT of the
// avatar (so the crackling arc is immediately visible) instead of at a fixed far
// station. Only x/y/z/yaw matter here.
export type VerseSpawnedSceneAvatarAnchor = Readonly<{
  x: number
  y: number
  z: number
  yaw?: number
}>

// Drop the scene station in front of the avatar along its facing direction, at a
// comfortable viewing height. Falls back to the fixed station anchor when there
// is no pose yet. Pure; tuned here, not in the renderer.
//
// Returns the station in ROOT-LOCAL coordinates (what the renderer's entity
// positions are interpreted in). It does so by placing the station in SCENE-WORLD
// — in front of the avatar (-Z, rotated by yaw) at chest height (+Y) — and then
// converting to root-local with `verseSceneWorldToRootLocal`. The avatar pose
// itself is captured in SCENE-WORLD (the third-person controller's getPosition),
// so this is the only frame in which "in front of the avatar" is meaningful; the
// earlier code treated the scene-world pose as if it were root-local and the arc
// ended up ~5 units in the air, far outside the camera — the real no-effect bug.
export const verseSpawnedSceneStationWorldForAvatar = (
  anchor: VerseSpawnedSceneAvatarAnchor | null | undefined,
): TrainingRunVector | null => {
  if (
    anchor === null ||
    anchor === undefined ||
    !finite(anchor.x) ||
    !finite(anchor.y) ||
    !finite(anchor.z)
  ) {
    return null
  }
  const yaw = finite(anchor.yaw ?? Number.NaN) ? (anchor.yaw as number) : 0
  // Scene-world: forward is -Z (avatar faces -Z at yaw 0), up is +Y, rotated by yaw.
  return addSceneWorldOffset(
    [anchor.x, anchor.y, anchor.z],
    [0, VERSE_SPAWNED_SCENE_AVATAR_HEIGHT, VERSE_SPAWNED_SCENE_AVATAR_FORWARD],
    yaw,
  )
}

export const verseSpawnedSceneStationForAvatar = (
  anchor: VerseSpawnedSceneAvatarAnchor | null | undefined,
): TrainingRunVector => {
  const world = verseSpawnedSceneStationWorldForAvatar(anchor)
  if (world === null) return VERSE_SPAWNED_SCENE_STATION_POSITION
  return verseSceneWorldToRootLocal(world)
}

export type VerseSpawnedSceneInput = Readonly<{
  sceneId: string
  /** where the scene station sits in the live world (defaults to the anchor). */
  station?: TrainingRunVector
  /** the avatar pose; when set, the station is dropped in front of the avatar. */
  avatar?: VerseSpawnedSceneAvatarAnchor | null
  knobs?: VerseSpawnedSceneKnobs
  generatedAt?: string
}>

// Build the in-world layer for ONE spawned isolated scene. Returns an empty layer
// for an unknown scene id (never fabricate a scene). The crackling arc is a
// `crackling_arc` beam between two positioned scene-station endpoints; the optional
// gateway portal is a `gateway_portal` entity below the arc. Every beam/burst is
// evidence-bound to the synthetic ref + simulated:true (the §5 contract, honestly
// labelled as a simulation).
export const verseSpawnedSceneLayer = (
  input: VerseSpawnedSceneInput,
): VerseSpawnedSceneLayer => {
  const scene = verseSpawnableSceneById(input.sceneId)
  if (scene === null) return EMPTY_LAYER

  // The arc geometry is authored in SCENE-WORLD avatar axes (X=right, Y=up,
  // Z=forward), placed in front of the avatar, then converted to ROOT-LOCAL for
  // the renderer. We compute the SCENE-WORLD station + yaw here so each endpoint
  // offset is applied in the avatar's own frame before the single root-local
  // conversion. This is what makes the arc land in the camera's actual view.
  //
  //   • explicit `station` (rare/dev): given in root-local, used as-is.
  //   • avatar pose: station in front of the avatar in scene-world.
  //   • neither: the fixed fallback station (already root-local, in front of the
  //     default camera).
  const explicitRootLocalStation = input.station
  const stationWorld = verseSpawnedSceneStationWorldForAvatar(input.avatar)
  const yaw =
    input.avatar !== null &&
    input.avatar !== undefined &&
    finite(input.avatar.yaw ?? Number.NaN)
      ? (input.avatar.yaw as number)
      : 0

  // Resolve one endpoint offset (scene-world avatar axes) to a ROOT-LOCAL vector.
  const resolveEndpoint = (offset: TrainingRunVector): TrainingRunVector => {
    if (stationWorld !== null) {
      return verseSceneWorldToRootLocal(
        addSceneWorldOffset(stationWorld, offset, yaw),
      )
    }
    // No avatar pose: fall back to a fixed root-local station and add the offset's
    // X (width) directly; map Y (height) onto root-local Z and Z (forward) onto
    // root-local -Y, matching `verseSceneWorldToRootLocal`'s axis mapping.
    const base = explicitRootLocalStation ?? VERSE_SPAWNED_SCENE_STATION_POSITION
    return roundedVerseVector([
      base[0] + offset[0] / VERSE_ROOT_SCALE,
      base[1] - offset[2] / VERSE_ROOT_SCALE,
      base[2] + offset[1] / VERSE_ROOT_SCALE,
    ])
  }
  const stationAnchor =
    explicitRootLocalStation ?? verseSpawnedSceneStationForAvatar(input.avatar)
  if (!finite(stationAnchor[0]) || !finite(stationAnchor[1]) || !finite(stationAnchor[2])) {
    return EMPTY_LAYER
  }

  const knobs: Required<
    Pick<
      VerseSpawnedSceneKnobs,
      | "fromOffset"
      | "toOffset"
      | "showPortal"
      | "portalLane"
      | "strandCount"
      | "opacity"
      | "jitter"
      | "rate"
      | "color"
      | "secondaryColor"
    >
  > = {
    fromOffset:
      input.knobs?.fromOffset ?? scene.defaultKnobs.fromOffset ?? [-1.6, 0, 0.2],
    toOffset:
      input.knobs?.toOffset ?? scene.defaultKnobs.toOffset ?? [1.6, 0, 1.2],
    showPortal: input.knobs?.showPortal ?? scene.defaultKnobs.showPortal ?? false,
    portalLane:
      input.knobs?.portalLane ?? scene.defaultKnobs.portalLane ?? "openrouter",
    strandCount:
      input.knobs?.strandCount ?? scene.defaultKnobs.strandCount ?? 11,
    opacity: input.knobs?.opacity ?? scene.defaultKnobs.opacity ?? 0.95,
    jitter: input.knobs?.jitter ?? scene.defaultKnobs.jitter ?? 0.34,
    rate: input.knobs?.rate ?? scene.defaultKnobs.rate ?? 2.6,
    color: input.knobs?.color ?? scene.defaultKnobs.color ?? 0x93c5fd,
    secondaryColor:
      input.knobs?.secondaryColor ??
      scene.defaultKnobs.secondaryColor ??
      0xf8fafc,
  }

  const keyBase = `${VERSE_SPAWNED_SCENE_NODE_PREFIX}${scene.id}`
  const fromId = `${keyBase}:from`
  const toId = `${keyBase}:to`
  const portalId = `${keyBase}:portal`

  const fromPosition = resolveEndpoint(knobs.fromOffset)
  const toPosition = resolveEndpoint(knobs.toOffset)

  const sourceRefs = [VERSE_SPAWNED_SCENE_SOURCE_REF] as const
  const detail = `${VERSE_SPAWNED_SCENE_SOURCE_REF} · ${scene.motionKind} · simulated`

  const entities: ChatWorldVisualEntityDefinition[] = [
    {
      id: fromId,
      label: scene.label,
      detail,
      status: "active",
      position: fromPosition,
      iconRecipe: verseIconRecipeForId("khala:nexus"),
    },
    {
      id: toId,
      label: "Scene station",
      detail,
      status: "active",
      position: toPosition,
      iconRecipe: verseIconRecipeForId(scene.id),
    },
  ]

  const evidence = {
    motionId: `verse-spawned:${scene.id}`,
    motionKind: scene.motionKind,
    sourceRefs,
    ...(input.generatedAt === undefined ? {} : { generatedAt: input.generatedAt }),
    // Explicitly labelled as a developer simulation — the whole point of an
    // isolated scene. Never a real live receipt.
    simulated: true,
  } as const

  // Carry the dialed-up appearance knobs on the beam so the shared renderer
  // makes the arc bright/thick/violent enough to read as real crackling energy
  // in the dark Verse (three-effect TrainingRunBeamAppearance pass-through).
  const beams: TrainingRunBeamDefinition[] = [
    {
      fromId,
      toId,
      style: "crackling_arc",
      appearance: {
        strandCount: knobs.strandCount,
        opacity: knobs.opacity,
        jitter: knobs.jitter,
        rate: knobs.rate,
        color: knobs.color,
        secondaryColor: knobs.secondaryColor,
      },
      ...evidence,
    },
  ]
  const bursts: TrainingRunBurstDefinition[] = []

  if (knobs.showPortal) {
    entities.push({
      id: portalId,
      label: "Gateway portal",
      detail: `${VERSE_SPAWNED_SCENE_SOURCE_REF} · gateway_portal · ${knobs.portalLane} · simulated`,
      status: "active",
      // Just below the arc (lower height: a negative Y offset in avatar axes).
      position: resolveEndpoint([0, -0.6, 0]),
      iconRecipe: verseIconRecipeForId(`gateway:${knobs.portalLane}:${scene.id}`),
      visualKind: "gateway_portal",
      gatewayLane: knobs.portalLane,
    })
  }

  return { entities, beams, bursts }
}

// Overlay every currently-spawned isolated scene onto the base visualization.
// Forces `motionPolicy.evidence = "required"` so the shared renderer animates a
// spawned arc ONLY when it carries the synthetic source ref — the same hard
// backstop as the payment / inference / Khala layers. An empty spawn list adds
// nothing (the Verse is byte-identical to before any spawn).
export const withVerseSpawnedSceneLayer = (
  base: TrainingRunVisualizationOptions,
  inputs: ReadonlyArray<VerseSpawnedSceneInput>,
): TrainingRunVisualizationOptions => {
  const entities: ChatWorldVisualEntityDefinition[] = []
  const beams: TrainingRunBeamDefinition[] = []
  const bursts: TrainingRunBurstDefinition[] = []
  for (const input of inputs) {
    const layer = verseSpawnedSceneLayer(input)
    entities.push(...layer.entities)
    beams.push(...layer.beams)
    bursts.push(...layer.bursts)
  }
  if (entities.length === 0) return base
  return {
    ...appendVerseVisualization(base, { entities, beams, bursts }),
    motionPolicy: { ...(base.motionPolicy ?? {}), evidence: "required" },
  }
}

// The evidence overlay text for a spawned scene, mirroring the standalone's
// on-screen `<pre class="evidence">` block (motionKind / sourceRefs / simulated /
// evidenceMode / generatedAt). Returned as ordered label lines so the view can
// render it as a small in-Verse chip without re-deriving the contract.
export const verseSpawnedSceneEvidenceLines = (
  input: VerseSpawnedSceneInput,
): ReadonlyArray<string> => {
  const scene = verseSpawnableSceneById(input.sceneId)
  if (scene === null) return []
  return [
    `scene:        ${scene.label}`,
    `motionKind:   ${scene.motionKind}`,
    `sourceRefs:   ${VERSE_SPAWNED_SCENE_SOURCE_REF}`,
    `simulated:    true`,
    `evidenceMode: optional`,
    ...(input.generatedAt === undefined
      ? []
      : [`generatedAt:  ${input.generatedAt}`]),
    `portal:       ${(input.knobs?.showPortal ?? scene.defaultKnobs.showPortal) ? "on" : "off"}`,
  ]
}
