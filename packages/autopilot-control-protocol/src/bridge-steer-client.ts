// #5494 (epic #5492 G1): typed, capability-scoped bridge envelopes for the four
// steer-actions that were previously reachable from mobile only via the
// dev-token `/command` "type"-tagged path — spawn, submit-intent,
// pause/resume, deploy. (cancel + decision.resolve already had bridge builders
// in ./bridge-client + ./bridge-decision-client.) Each builder mirrors the
// existing bridge envelope shape (verb + idempotency + pairing/capability refs)
// and carries the verb-specific payload fields the node broker reads.
//
// Pure builders, no IO: the client mints the envelope, ./bridge-transport sends
// it over POST /bridge with the scoped Bridge credential, and the node enforces
// the capability from its STORED claims (never the client-sent set).

import {
  verbAllowedByCapabilities,
  type BridgeRequestEnvelope,
  type Capability,
} from "./bridge.js"

type BaseSteerInput = {
  pairingRef: string
  capabilityRef: string
  clientRequestId: string
  // Idempotency defaults to clientRequestId when omitted (one logical action).
  idempotencyKey?: string
}

function baseEnvelope(
  verb: BridgeRequestEnvelope["verb"],
  input: BaseSteerInput,
): BridgeRequestEnvelope {
  return {
    verb,
    clientRequestId: input.clientRequestId,
    idempotencyKey: input.idempotencyKey ?? input.clientRequestId,
    pairingRef: input.pairingRef,
    capabilityRef: input.capabilityRef,
  }
}

// ── session.spawn (spawn_session) ──────────────────────────────────────────
export type SpawnLane = "auto" | "local" | "cloud-gcp" | "cloud-shc"

export type BuildSpawnEnvelopeInput = BaseSteerInput & {
  adapter: "codex" | "claude_agent"
  objective: string
  verify?: string[]
  lane?: SpawnLane
}

export type SpawnEnvelope = BridgeRequestEnvelope & {
  adapter: "codex" | "claude_agent"
  objective: string
  verify: string[]
  lane: SpawnLane
}

export function buildSpawnEnvelope(input: BuildSpawnEnvelopeInput): SpawnEnvelope {
  return {
    ...baseEnvelope("session.spawn", input),
    adapter: input.adapter,
    objective: input.objective,
    verify: input.verify ?? [],
    lane: input.lane ?? "auto",
  }
}

// ── intent.submit (send_instruction) ───────────────────────────────────────
export type BuildIntentSubmitEnvelopeInput = BaseSteerInput & {
  title: string
  body: string
  scopeHint?: string
  submittedByClientRef?: string
}

export type IntentSubmitEnvelope = BridgeRequestEnvelope & {
  title: string
  body: string
  scopeHint?: string
  submittedByClientRef?: string
}

export function buildIntentSubmitEnvelope(input: BuildIntentSubmitEnvelopeInput): IntentSubmitEnvelope {
  return {
    ...baseEnvelope("intent.submit", input),
    title: input.title,
    body: input.body,
    ...(input.scopeHint === undefined ? {} : { scopeHint: input.scopeHint }),
    ...(input.submittedByClientRef === undefined ? {} : { submittedByClientRef: input.submittedByClientRef }),
  }
}

// ── turn.steer (send_instruction) ─────────────────────────────────────────
export type BuildTurnSteerEnvelopeInput = BaseSteerInput & {
  sessionRef: string
  instruction: string
  timeoutSeconds?: number
}

export type TurnSteerEnvelope = BridgeRequestEnvelope & {
  sessionRef: string
  instruction: string
  timeoutSeconds?: number
}

export function buildTurnSteerEnvelope(input: BuildTurnSteerEnvelopeInput): TurnSteerEnvelope {
  return {
    ...baseEnvelope("turn.steer", input),
    sessionRef: input.sessionRef,
    instruction: input.instruction,
    ...(input.timeoutSeconds === undefined ? {} : { timeoutSeconds: input.timeoutSeconds }),
  }
}

// ── coordinator.pause / coordinator.resume (pause_resume) ──────────────────
export function buildCoordinatorPauseEnvelope(input: BaseSteerInput): BridgeRequestEnvelope {
  return baseEnvelope("coordinator.pause", input)
}

export function buildCoordinatorResumeEnvelope(input: BaseSteerInput): BridgeRequestEnvelope {
  return baseEnvelope("coordinator.resume", input)
}

// ── deploy.cloud (deploy_cloud) ────────────────────────────────────────────
export type BuildDeployCloudEnvelopeInput = BaseSteerInput & {
  target: string
  ref: string
  env?: string
}

export type DeployCloudEnvelope = BridgeRequestEnvelope & {
  target: string
  ref: string
  env?: string
}

export function buildDeployCloudEnvelope(input: BuildDeployCloudEnvelopeInput): DeployCloudEnvelope {
  return {
    ...baseEnvelope("deploy.cloud", input),
    target: input.target,
    ref: input.ref,
    ...(input.env === undefined ? {} : { env: input.env }),
  }
}

// Capability predicates so a client can disable controls honestly before it
// even mints an envelope (mirrors canResolveDecision in ./bridge-decision-client).
export function canSpawnSession(capabilities: ReadonlyArray<Capability>): boolean {
  return verbAllowedByCapabilities("session.spawn", capabilities)
}

export function canSubmitIntent(capabilities: ReadonlyArray<Capability>): boolean {
  return verbAllowedByCapabilities("intent.submit", capabilities)
}

export function canSteerTurn(capabilities: ReadonlyArray<Capability>): boolean {
  return verbAllowedByCapabilities("turn.steer", capabilities)
}

export function canPauseResumeCoordinator(capabilities: ReadonlyArray<Capability>): boolean {
  return verbAllowedByCapabilities("coordinator.pause", capabilities)
}

export function canDeployCloud(capabilities: ReadonlyArray<Capability>): boolean {
  return verbAllowedByCapabilities("deploy.cloud", capabilities)
}
