import type {
  TrainingRunNodeDefinition,
  TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"

import {
  type ChatWorldPylonScene,
  type LivePylonNode,
  type PaymentParticle,
  type PylonGrowthTier,
  pylonGrowthTier,
} from "./chat-world-scene"
import type { OnboardingStatusResponse, OnboardingStepStatus } from "./onboarding-status"
import type {
  IdentityChoiceStateResponse,
  TrainingOperatorReadinessResponse,
} from "./rpc"
import { VERSE_TASSADAR_CORE_NODE_ID } from "./verse-training-visualization"

export const PYLON_BASE_NODE_ID = "verse-pylon-base:my-base"
export const PYLON_BASE_NODE_PREFIX = "verse-pylon-base:"
export const PYLON_BASE_IDENTITY_MISSING_BLOCKER =
  "desktop.pylon_base.identity_missing"

export type PylonBaseStatus =
  | "missing"
  | "blocked"
  | "offline"
  | "online"
  | "wallet_ready"
  | "assignment_ready"

export type PylonBaseReadiness = {
  readonly identityPresent: boolean
  readonly online: boolean
  readonly presence: boolean
  readonly walletReady: boolean
  readonly assignmentReady: boolean
  readonly localPylonReady: boolean
}

export type PylonBaseMana = {
  readonly current: number
  readonly total: number
  readonly ratio: number
}

export type PylonBaseProjection = {
  readonly status: PylonBaseStatus
  readonly label: string
  readonly pylonRef: string | null
  readonly matchedFleetNodeId: string | null
  readonly readiness: PylonBaseReadiness
  readonly mana: PylonBaseMana
  readonly blockerRefs: readonly string[]
  readonly nextAction: string | null
  readonly settledSats: number
  readonly sourceRefs: readonly string[]
  readonly growth: PylonGrowthTier
  readonly fleetGrowth: PylonGrowthTier
  readonly statusLine: string
}

export type ProjectPylonBaseInput = {
  readonly identityChoice: IdentityChoiceStateResponse | null
  readonly trainingOperatorReadiness: TrainingOperatorReadinessResponse | null
  readonly onboardingStatus: OnboardingStatusResponse | null
  readonly chatWorldScene: ChatWorldPylonScene | null
  readonly particles: ReadonlyArray<PaymentParticle>
}

const uniqueStrings = (
  values: ReadonlyArray<string | null | undefined>,
): string[] => {
  const out: string[] = []
  for (const value of values) {
    const trimmed = typeof value === "string" ? value.trim() : ""
    if (trimmed.length === 0 || out.includes(trimmed)) continue
    out.push(trimmed)
  }
  return out
}

const normalizeToken = (value: string): string =>
  value.trim().toLowerCase().replace(/^pylon[.:]/, "")

const identityTokens = (
  identityChoice: IdentityChoiceStateResponse | null,
  readiness: TrainingOperatorReadinessResponse | null,
): string[] =>
  uniqueStrings([
    readiness?.pylonRef ?? null,
    identityChoice?.detected.pylonRef ?? null,
    identityChoice?.detected.shortLabel ?? null,
    identityChoice?.detected.npub ?? null,
  ]).flatMap((token) => {
    const normalized = normalizeToken(token)
    return uniqueStrings([token, normalized, `pylon.${normalized}`, `pylon:${normalized}`])
  })

const matchesAnyToken = (
  value: string | null | undefined,
  tokens: ReadonlyArray<string>,
): boolean => {
  if (typeof value !== "string" || value.trim().length === 0) return false
  const normalized = normalizeToken(value)
  return tokens.some((token) => {
    const normalizedToken = normalizeToken(token)
    return (
      normalized === normalizedToken ||
      normalized.includes(normalizedToken) ||
      normalizedToken.includes(normalized)
    )
  })
}

const findFleetNode = (
  scene: ChatWorldPylonScene | null,
  tokens: ReadonlyArray<string>,
): LivePylonNode | null => {
  if (scene === null || scene.empty || tokens.length === 0) return null
  return (
    scene.nodes.find((node) =>
      [node.id, node.label, ...node.products].some((value) =>
        matchesAnyToken(value, tokens),
      ),
    ) ?? null
  )
}

const stepStatus = (
  status: OnboardingStatusResponse | null,
  id: string,
): OnboardingStepStatus | null =>
  status?.steps.find((step) => step.id === id)?.status ?? null

const stepDone = (status: OnboardingStatusResponse | null, id: string): boolean =>
  stepStatus(status, id) === "done"

const stepActiveOrDone = (
  status: OnboardingStatusResponse | null,
  id: string,
): boolean => {
  const statusValue = stepStatus(status, id)
  return statusValue === "active" || statusValue === "done"
}

const settledSatsForBase = (
  particles: ReadonlyArray<PaymentParticle>,
  tokens: ReadonlyArray<string>,
): { settledSats: number; sourceRefs: readonly string[] } => {
  let settledSats = 0
  const sourceRefs: string[] = []
  for (const particle of particles) {
    if (particle.sourceRefs.length === 0) continue
    const endpointMatch =
      matchesAnyToken(particle.fromRef, tokens) ||
      matchesAnyToken(particle.toRef, tokens)
    if (!endpointMatch) continue
    settledSats += Math.max(0, particle.amountSats)
    for (const ref of particle.sourceRefs) {
      if (!sourceRefs.includes(ref)) sourceRefs.push(ref)
    }
  }
  return { settledSats, sourceRefs }
}

const readinessStatus = (
  missing: boolean,
  readiness: PylonBaseReadiness,
  blockerRefs: ReadonlyArray<string>,
): PylonBaseStatus => {
  if (missing) return "missing"
  if (blockerRefs.length > 0 && !readiness.localPylonReady) return "blocked"
  if (readiness.assignmentReady) return "assignment_ready"
  if (readiness.walletReady) return "wallet_ready"
  if (readiness.online || readiness.presence || readiness.localPylonReady) return "online"
  return "offline"
}

const pylonBaseMana = (
  readiness: PylonBaseReadiness,
  settledSats: number,
  earnedStepDone: boolean,
): PylonBaseMana => {
  const total = 5
  const current = [
    readiness.identityPresent,
    readiness.online || readiness.presence || readiness.localPylonReady,
    readiness.walletReady,
    readiness.assignmentReady,
    settledSats > 0 || earnedStepDone,
  ].filter(Boolean).length
  return {
    current,
    total,
    ratio: Number((current / total).toFixed(3)),
  }
}

const nextActionFor = (
  status: PylonBaseStatus,
  readiness: PylonBaseReadiness,
): string | null => {
  if (status === "missing") return "Choose or create a Pylon identity"
  if (!readiness.localPylonReady) return "Start the local Pylon"
  if (!readiness.walletReady) return "Prepare the wallet receive path"
  if (!readiness.assignmentReady) return "Claim a Tassadar assignment"
  return null
}

const statusLineFor = (projection: {
  readonly pylonRef: string | null
  readonly mana: PylonBaseMana
  readonly settledSats: number
  readonly nextAction: string | null
}): string => {
  const name = projection.pylonRef ?? "no Pylon identity"
  const mana = `${projection.mana.current}/${projection.mana.total} mana`
  const sats = `${projection.settledSats} settled sats`
  if (projection.nextAction !== null) {
    return `${name} · ${mana} · ${sats} · ${projection.nextAction}`
  }
  return `${name} · ${mana} · ${sats}`
}

export const projectPylonBase = (
  input: ProjectPylonBaseInput,
): PylonBaseProjection => {
  const pylonRef =
    input.trainingOperatorReadiness?.pylonRef ??
    input.identityChoice?.detected.pylonRef ??
    null
  const tokens = identityTokens(input.identityChoice, input.trainingOperatorReadiness)
  const matchedNode = findFleetNode(input.chatWorldScene, tokens)
  const missing = pylonRef === null
  const settled = settledSatsForBase(input.particles, tokens)
  const readiness: PylonBaseReadiness = {
    identityPresent: !missing,
    online:
      matchedNode?.online === true ||
      input.trainingOperatorReadiness?.localPylonReady === true ||
      stepDone(input.onboardingStatus, "node-online"),
    presence: stepDone(input.onboardingStatus, "presence"),
    walletReady:
      matchedNode?.state === "wallet_ready" ||
      matchedNode?.state === "assignment_ready" ||
      stepDone(input.onboardingStatus, "wallet"),
    assignmentReady:
      matchedNode?.state === "assignment_ready" ||
      stepActiveOrDone(input.onboardingStatus, "claimed") ||
      stepDone(input.onboardingStatus, "tassadar"),
    localPylonReady: input.trainingOperatorReadiness?.localPylonReady === true,
  }
  const blockerRefs = missing
    ? [PYLON_BASE_IDENTITY_MISSING_BLOCKER]
    : uniqueStrings(input.trainingOperatorReadiness?.blockerRefs ?? [])
  const mana = pylonBaseMana(
    readiness,
    settled.settledSats,
    stepDone(input.onboardingStatus, "earned"),
  )
  const status = readinessStatus(missing, readiness, blockerRefs)
  const nextAction = nextActionFor(status, readiness)
  const projection = {
    status,
    label: input.identityChoice?.detected.shortLabel ?? pylonRef ?? "My Pylon Base",
    pylonRef,
    matchedFleetNodeId: matchedNode?.id ?? null,
    readiness,
    mana,
    blockerRefs,
    nextAction,
    settledSats: settled.settledSats,
    sourceRefs: settled.sourceRefs,
    growth: pylonGrowthTier(settled.settledSats),
    fleetGrowth: input.chatWorldScene?.growth ?? pylonGrowthTier(0),
  }
  return {
    ...projection,
    statusLine: statusLineFor(projection),
  }
}

const baseNodeStatus = (
  projection: PylonBaseProjection,
): TrainingRunNodeDefinition["status"] => {
  if (projection.status === "missing" || projection.status === "blocked") {
    return "blocked"
  }
  if (projection.status === "assignment_ready") return "active"
  if (projection.status === "wallet_ready") return "sync"
  if (projection.status === "online") return "queued"
  if (projection.settledSats > 0) return "verified"
  return "planned"
}

export const pylonBaseNode = (
  projection: PylonBaseProjection,
): TrainingRunNodeDefinition => {
  const refs =
    projection.sourceRefs.length > 0
      ? ` · refs ${projection.sourceRefs.slice(0, 3).join(", ")}`
      : ""
  const blocker =
    projection.blockerRefs.length > 0
      ? ` · blockers ${projection.blockerRefs.slice(0, 3).join(", ")}`
      : ""
  return {
    id: PYLON_BASE_NODE_ID,
    label: "My Pylon Base",
    detail: `${projection.statusLine}${refs}${blocker}`,
    role:
      projection.status === "missing" || projection.status === "blocked"
        ? "gate"
        : "lifecycle",
    status: baseNodeStatus(projection),
    position: [0, 2.72, 0],
    connectedTo: [VERSE_TASSADAR_CORE_NODE_ID],
  }
}

export const withPylonBaseLayer = (
  base: TrainingRunVisualizationOptions,
  projection: PylonBaseProjection,
): TrainingRunVisualizationOptions => ({
  ...base,
  nodes: [...(base.nodes ?? []), pylonBaseNode(projection)],
})
