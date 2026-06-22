// Verse Khala in-world effect projection (EPIC #6017 — talk to Khala from a Verse
// textbox; the audit doc 2026-06-22-talk-to-khala-from-verse-audit.md gap #3).
//
// PURE, Three.js-free transform that turns ONE local Khala cockpit turn receipt
// (the `openagents` block projected by `khalaTurn` — see shared/khala-cockpit.ts)
// plus the local avatar position into a crackling-arc layer the shared
// three-effect `trainingRunView` renders IMMEDIATELY, without waiting on the
// ~5–10s public-activity-timeline poll. The public-timeline projection
// (chat-world-visualization.ts `chatWorldInferenceLayer`) still fires for other
// viewers; this is the responsive LOCAL mirror.
//
// EVIDENCE-BOUND (the §5 motion contract). The arc only renders when the turn
// carried a REAL receipt ref. No receipt → no effect. The receipt ref rides on
// the beam (motion evidence) and on the clickable endpoint entities (click →
// receipt detail), exactly like the gateway-backed inference layer, so the local
// arc obeys the same `motionPolicy.evidence = "required"` backstop.
//
// This is the LOCAL sibling of `chatWorldInferenceLayer`: same crackling-arc
// encoding (Khala nexus → avatar), keyed to the receipt instead of a projected
// world `inference_event` row.

import { verseIconRecipeForId } from "@openagentsinc/three-effect/core"
import type {
  TrainingRunBeamDefinition,
  TrainingRunBurstDefinition,
  TrainingRunVector,
  TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"

import type { KhalaReceiptProjection } from "./khala-cockpit.js"
import { isLiveReceipt } from "./khala-cockpit.js"
import {
  appendVerseVisualization,
  roundedVerseVector,
} from "./verse-scene-helpers.js"
import type { ChatWorldVisualEntityDefinition } from "./chat-world-visualization.js"

// Stable scene-node prefix for the local Khala effect endpoints, distinct from
// the world-projection inference prefix so a local arc and the later public-
// timeline arc never collide on id.
export const VERSE_KHALA_NEXUS_NODE_ID = "verse:khala:nexus"
export const VERSE_KHALA_EFFECT_NODE_PREFIX = "verse:khala:effect:"

// Where the local Khala nexus sits in the scene when we have no projected world
// nexus position. Above the play space so the arc reads as energy coming down to
// the avatar. Tuned here, not in the renderer.
const DEFAULT_NEXUS_POSITION: TrainingRunVector = roundedVerseVector([0, 1.85, 0.6])

// A tiny upward offset so the arc terminates at the avatar's torso, not its feet.
const AVATAR_ARC_HEIGHT_OFFSET = 1.1

// The local avatar anchor (the user's character) the arc terminates at.
export type VerseKhalaAvatarAnchor = Readonly<{
  x: number
  y: number
  z: number
}>

// One local Khala turn's effect input: the receipt projection (the evidence) and
// the avatar anchor (where the arc lands). `generatedAt` lets the renderer derive
// motion timing; the effect key is the receipt ref.
export type VerseKhalaEffectInput = Readonly<{
  receipt: KhalaReceiptProjection | null
  avatar: VerseKhalaAvatarAnchor | null
  generatedAt?: string
}>

export type VerseKhalaEffectLayer = Readonly<{
  entities: ReadonlyArray<ChatWorldVisualEntityDefinition>
  beams: ReadonlyArray<TrainingRunBeamDefinition>
  bursts: ReadonlyArray<TrainingRunBurstDefinition>
}>

const EMPTY_LAYER: VerseKhalaEffectLayer = { entities: [], beams: [], bursts: [] }

const finite = (value: number): boolean =>
  Number.isFinite(value) && !Number.isNaN(value)

const avatarVector = (
  anchor: VerseKhalaAvatarAnchor | null,
): TrainingRunVector | null => {
  if (anchor === null) return null
  if (!finite(anchor.x) || !finite(anchor.y) || !finite(anchor.z)) return null
  return roundedVerseVector([anchor.x, anchor.y + AVATAR_ARC_HEIGHT_OFFSET, anchor.z])
}

// The verification class maps to the same scene status language the world
// inference layer uses: a verified Khala turn glows gold-family ("verified"),
// a failed one reads "blocked", anything else is a live but unverified "active".
const receiptStatus = (receipt: KhalaReceiptProjection): string => {
  if (receipt.verification === "failed") return "blocked"
  if (receipt.verification === "test_passed") return "verified"
  return "active"
}

// Build the evidence-bound local crackling-arc layer for ONE Khala turn. Returns
// an empty layer (no motion) unless the turn carried a REAL receipt ref — the
// evidence gate. The avatar anchor is required to land the arc; without it we
// fall back to a fixed point near the nexus so the energy is still visible.
export const verseKhalaEffectLayer = (
  input: VerseKhalaEffectInput,
): VerseKhalaEffectLayer => {
  const receipt = input.receipt
  // EVIDENCE GATE: only animate with a real receipt ref (audit §5; the same gate
  // as the cockpit "live" badge — isLiveReceipt).
  if (receipt === null || !isLiveReceipt(receipt) || receipt.receipt === null) {
    return EMPTY_LAYER
  }
  const receiptRef = receipt.receipt

  const avatar =
    avatarVector(input.avatar) ?? roundedVerseVector([0.0, 0.6, 1.2])

  const status = receiptStatus(receipt)
  const fromId = VERSE_KHALA_NEXUS_NODE_ID
  const toId = `${VERSE_KHALA_EFFECT_NODE_PREFIX}${receiptRef}`

  const servedLabel =
    receipt.servedModel.trim().length > 0 ? receipt.servedModel : "Khala"
  const verifiedDetail =
    receipt.verification === "test_passed"
      ? "verified (tests passed)"
      : receipt.verification === "failed"
        ? "verification failed"
        : "not verified"

  const entities: ChatWorldVisualEntityDefinition[] = [
    {
      id: fromId,
      label: "Khala",
      detail: `${receiptRef} · ${receipt.requestedModel || "openagents/khala"} · ${receipt.lane} lane`,
      status,
      position: DEFAULT_NEXUS_POSITION,
      iconRecipe: verseIconRecipeForId("khala:nexus"),
    },
    {
      id: toId,
      label: "You",
      detail: `${receiptRef} · ${servedLabel} · ${verifiedDetail}`,
      status,
      position: avatar,
      iconRecipe: verseIconRecipeForId(receiptRef),
    },
  ]

  const evidence = {
    motionId: `verse-khala:${receiptRef}`,
    motionKind: "khala_in_world_inference",
    sourceRefs: [receiptRef],
    ...(input.generatedAt === undefined ? {} : { generatedAt: input.generatedAt }),
    // A real local turn against the live gateway — never a labelled simulation.
    simulated: false,
  } as const

  const beams: TrainingRunBeamDefinition[] = [
    { fromId, toId, style: "crackling_arc", ...evidence },
  ]
  // A settlement-style burst at the avatar only when the turn actually verified.
  const bursts: TrainingRunBurstDefinition[] =
    receipt.verification === "test_passed"
      ? [{ atId: toId, ...evidence }]
      : []

  return { entities, beams, bursts }
}

// Overlay the local Khala crackling-arc layer onto the base visualization. Forces
// `motionPolicy.evidence = "required"` so the renderer animates the arc ONLY when
// it carries the receipt ref — the same hard backstop as the payment/inference
// layers. A no-receipt input adds nothing (no motion, no entities).
export const withVerseKhalaEffectLayer = (
  base: TrainingRunVisualizationOptions,
  input: VerseKhalaEffectInput,
): TrainingRunVisualizationOptions => {
  const layer = verseKhalaEffectLayer(input)
  if (layer.entities.length === 0) return base
  return {
    ...appendVerseVisualization(base, {
      entities: layer.entities,
      beams: layer.beams,
      bursts: layer.bursts,
    }),
    motionPolicy: { ...(base.motionPolicy ?? {}), evidence: "required" },
  }
}
