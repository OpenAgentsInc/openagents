import type { BridgeRequestVerb, Capability } from "./bridge.js"

type BridgePairStatusClaims = {
  expiresAt?: string
  revoked?: boolean
  capabilities?: string[]
} | null

type BridgePairStatusProjection = {
  state: "unpaired" | "active" | "expired" | "revoked"
  verbCount: number
  reason: string
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

const VALID_CAPABILITIES = new Set<Capability>([
  "observe_public",
  "observe_private",
  "answer_decision",
  "send_instruction",
  "cancel",
  "pause_resume",
  "read_artifact",
])

export function projectPairStatus(
  claims: BridgePairStatusClaims,
  nowIso: string,
): BridgePairStatusProjection {
  if (claims === null) {
    return { state: "unpaired", verbCount: 0, reason: "no_pairing_claims" }
  }

  if (claims.revoked === true) {
    return { state: "revoked", verbCount: 0, reason: "pairing_revoked" }
  }

  if (isExpired(claims.expiresAt, nowIso)) {
    return { state: "expired", verbCount: 0, reason: "pairing_expired" }
  }

  return {
    state: "active",
    verbCount: countAllowedVerbs(projectCapabilities(claims.capabilities)),
    reason: "pairing_active",
  }
}

function projectCapabilities(capabilities: unknown): Capability[] {
  if (!Array.isArray(capabilities)) return []

  return capabilities.filter((capability): capability is Capability => (
    typeof capability === "string"
    && VALID_CAPABILITIES.has(capability as Capability)
  ))
}

function countAllowedVerbs(capabilities: ReadonlyArray<Capability>): number {
  return BRIDGE_REQUEST_VERBS.filter((verb) => verbAllowedByProjectedCapabilities(verb, capabilities)).length
}

function verbAllowedByProjectedCapabilities(verb: BridgeRequestVerb, capabilities: ReadonlyArray<Capability>): boolean {
  const has = (capability: Capability) => capabilities.includes(capability)
  switch (verb) {
    case "decision.resolve":
      return has("answer_decision")
    case "turn.steer":
      return has("send_instruction")
    case "turn.interrupt":
    case "session.cancel":
      return has("cancel")
    case "session.pause":
    case "session.resume":
      return has("pause_resume")
    case "artifact.read":
      return has("read_artifact")
    default:
      return capabilities.length > 0
  }
}

function isExpired(expiresAt: string | undefined, nowIso: string): boolean {
  if (typeof expiresAt !== "string" || expiresAt.length === 0) return false

  const expiresAtMs = Date.parse(expiresAt)
  const nowMs = Date.parse(nowIso)
  if (!Number.isFinite(expiresAtMs) || !Number.isFinite(nowMs)) return false

  return expiresAtMs < nowMs
}
