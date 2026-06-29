import {
  verbAllowedByCapabilities,
  type BridgeRequestVerb,
  type Capability,
} from "./bridge.js"

export type BridgeCapabilityGrantProjection = {
  verbs: string[]
  readOnly: boolean
  canSpawn: boolean
  canApprove: boolean
}

const BRIDGE_REQUEST_VERBS = [
  "bridge.pair.exchange",
  "bridge.revoke",
  "bridge.clients.list",
  "session.list",
  "session.subscribe",
  "session.snapshot",
  "session.history",
  "turn.steer",
  "turn.interrupt",
  "session.cancel",
  "session.pause",
  "session.resume",
  "decision.resolve",
  "artifact.read",
  "capability.list",
  // #5494 (epic #5492 G1): the promoted steer verbs. Appended (not interleaved)
  // so the existing projection order is preserved for credentials that don't
  // hold the new steer capability classes.
  "session.spawn",
  "intent.submit",
  "coordinator.pause",
  "coordinator.resume",
  "deploy.cloud",
] satisfies ReadonlyArray<BridgeRequestVerb>

const MUTATING_BRIDGE_VERBS = new Set<BridgeRequestVerb>([
  "turn.steer",
  "turn.interrupt",
  "session.cancel",
  "session.pause",
  "session.resume",
  "decision.resolve",
  // #5494: the promoted steer verbs are all mutating (none observe-class).
  "session.spawn",
  "intent.submit",
  "coordinator.pause",
  "coordinator.resume",
  "deploy.cloud",
])

const VALID_CAPABILITIES = new Set<Capability>([
  "observe_public",
  "observe_private",
  "answer_decision",
  "send_instruction",
  "cancel",
  "pause_resume",
  "read_artifact",
  // #5494: steer-action capability classes.
  "spawn_session",
  "deploy_cloud",
])

function projectCapabilities(capabilities: ReadonlyArray<string> | undefined): Capability[] {
  if (!Array.isArray(capabilities)) return []

  return capabilities.filter((capability): capability is Capability => VALID_CAPABILITIES.has(capability as Capability))
}

export function projectGrantedCapabilities(
  claims: { capabilities?: string[] } | null,
): BridgeCapabilityGrantProjection {
  const capabilities = projectCapabilities(claims?.capabilities)
  const verbs = BRIDGE_REQUEST_VERBS.filter((verb) => verbAllowedByCapabilities(verb, capabilities))

  return {
    verbs,
    readOnly: verbs.every((verb) => !MUTATING_BRIDGE_VERBS.has(verb)),
    canSpawn: verbs.includes("turn.steer"),
    canApprove: verbs.includes("decision.resolve"),
  }
}
