import {
  verbAllowedByCapabilities,
  type BridgeRequestVerb,
  type Capability,
} from "./bridge"

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
] satisfies ReadonlyArray<BridgeRequestVerb>

const MUTATING_BRIDGE_VERBS = new Set<BridgeRequestVerb>([
  "turn.steer",
  "turn.interrupt",
  "session.cancel",
  "session.pause",
  "session.resume",
  "decision.resolve",
])

const VALID_CAPABILITIES = new Set<Capability>([
  "observe_public",
  "observe_private",
  "answer_decision",
  "send_instruction",
  "cancel",
  "pause_resume",
  "read_artifact",
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
