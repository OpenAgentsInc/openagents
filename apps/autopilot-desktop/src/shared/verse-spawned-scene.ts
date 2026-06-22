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
} from "./verse-scene-helpers.js"
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

// Fixed in-world "scene station" anchor: a short walk from spawn so the avatar
// can stroll up to it. Tuned HERE (the mapper), not in the renderer. The crackling
// arc hangs above the station; the optional portal sits just below it.
export const VERSE_SPAWNED_SCENE_STATION_POSITION: TrainingRunVector =
  roundedVerseVector([0, 0, -6])

// ── Eyeball knobs (mirrors the standalone's URL knobs as typed options) ───────

// The crackling-arc knobs the standalone scene exposes (strandCount/rate/color/
// endpoints). Defaults match crackling-arc-standalone so the in-world spawn reads
// the same as the isolated page. Endpoints are RELATIVE to the station anchor.
export type VerseSpawnedSceneKnobs = Readonly<{
  /** arc start, relative to the station anchor. */
  fromOffset?: TrainingRunVector
  /** arc end, relative to the station anchor. */
  toOffset?: TrainingRunVector
  strandCount?: number
  rate?: number
  opacity?: number
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

// The crackling-energy scene defaults, matched to the standalone demo.
const CRACKLING_ENERGY_DEFAULTS: VerseSpawnedSceneKnobs = {
  fromOffset: [-1.6, 1.6, 0],
  toOffset: [1.6, 2.6, 0],
  strandCount: 5,
  rate: 2.6,
  opacity: 0.78,
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

const addOffset = (
  anchor: TrainingRunVector,
  offset: TrainingRunVector,
): TrainingRunVector =>
  roundedVerseVector([
    anchor[0] + offset[0],
    anchor[1] + offset[1],
    anchor[2] + offset[2],
  ])

// One spawned scene's input: which registry scene, where its station sits, and
// the (optional) knob overrides. `generatedAt` rides the evidence so the renderer
// can derive motion timing; the synthetic source ref is the evidence.
export type VerseSpawnedSceneInput = Readonly<{
  sceneId: string
  /** where the scene station sits in the live world (defaults to the anchor). */
  station?: TrainingRunVector
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

  const stationAnchor = input.station ?? VERSE_SPAWNED_SCENE_STATION_POSITION
  if (!finite(stationAnchor[0]) || !finite(stationAnchor[1]) || !finite(stationAnchor[2])) {
    return EMPTY_LAYER
  }

  const knobs: Required<
    Pick<
      VerseSpawnedSceneKnobs,
      "fromOffset" | "toOffset" | "showPortal" | "portalLane"
    >
  > = {
    fromOffset:
      input.knobs?.fromOffset ?? scene.defaultKnobs.fromOffset ?? [-1.6, 1.6, 0],
    toOffset:
      input.knobs?.toOffset ?? scene.defaultKnobs.toOffset ?? [1.6, 2.6, 0],
    showPortal: input.knobs?.showPortal ?? scene.defaultKnobs.showPortal ?? false,
    portalLane:
      input.knobs?.portalLane ?? scene.defaultKnobs.portalLane ?? "openrouter",
  }

  const keyBase = `${VERSE_SPAWNED_SCENE_NODE_PREFIX}${scene.id}`
  const fromId = `${keyBase}:from`
  const toId = `${keyBase}:to`
  const portalId = `${keyBase}:portal`

  const fromPosition = addOffset(stationAnchor, knobs.fromOffset)
  const toPosition = addOffset(stationAnchor, knobs.toOffset)

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

  const beams: TrainingRunBeamDefinition[] = [
    { fromId, toId, style: "crackling_arc", ...evidence },
  ]
  const bursts: TrainingRunBurstDefinition[] = []

  if (knobs.showPortal) {
    entities.push({
      id: portalId,
      label: "Gateway portal",
      detail: `${VERSE_SPAWNED_SCENE_SOURCE_REF} · gateway_portal · ${knobs.portalLane} · simulated`,
      status: "active",
      position: addOffset(stationAnchor, [0, 0.4, 0]),
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
