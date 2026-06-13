import { Schema as S } from "effect"

import { ProjectionLevel } from "./control"

// The remote session bridge vocabulary (terminal-agent system #39). Clients
// reach a non-local node through this typed protocol rather than the node's
// all-purpose bearer token. Verbs and events are explicit names, never log
// scraping.

export const BridgeRequestVerb = S.Literals([
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
])
export type BridgeRequestVerb = typeof BridgeRequestVerb.Type

export const BridgeEventName = S.Literals([
  "session.status.changed",
  "turn.started",
  "turn.completed",
  "item.progress.delta",
  "decision.requested",
  "decision.cancelled",
  "decision.resolved",
  "artifact.available",
  "bridge.client.connected",
  "bridge.client.disconnected",
  "bridge.client.revoked",
  "stream.heartbeat",
  "stream.lagged",
])
export type BridgeEventName = typeof BridgeEventName.Type

// Capability classes are individually policy-gated. A read-only viewer cannot
// interrupt, cancel, answer, spawn, or steer.
export const Capability = S.Literals([
  "observe_public",
  "observe_private",
  "answer_decision",
  "send_instruction",
  "cancel",
  "pause_resume",
  "read_artifact",
])
export type Capability = typeof Capability.Type

// Every action request carries identity + idempotency + cursor so the node can
// serialize, dedupe, and resume.
export const BridgeRequestEnvelope = S.Struct({
  verb: BridgeRequestVerb,
  clientRequestId: S.String,
  idempotencyKey: S.String,
  pairingRef: S.String,
  capabilityRef: S.String,
  sessionRef: S.optional(S.String),
  cursor: S.optional(S.Number),
})
export type BridgeRequestEnvelope = typeof BridgeRequestEnvelope.Type

// Typed result states so a client never hangs on an unknown outcome.
export const BridgeResultStatus = S.Literals([
  "ok",
  "duplicate",
  "expired",
  "cancelled",
  "revoked",
  "stale",
  "unauthorized",
  "unsupported",
  "overloaded",
])
export type BridgeResultStatus = typeof BridgeResultStatus.Type

export const BridgeActionReceipt = S.Struct({
  clientRequestId: S.String,
  receiptRef: S.NullOr(S.String),
  status: BridgeResultStatus,
})
export type BridgeActionReceipt = typeof BridgeActionReceipt.Type

// A scoped pairing credential's public-safe projection (the secret itself lives
// only on the client; the node stores a hash + revocation state).
export const PairingCredentialClaims = S.Struct({
  pairingRef: S.String,
  clientId: S.String,
  deviceClass: S.String,
  issuer: S.String,
  audience: S.String,
  expiresAt: S.String,
  jti: S.String,
  projectionLevel: ProjectionLevel,
  capabilities: S.Array(Capability),
})
export type PairingCredentialClaims = typeof PairingCredentialClaims.Type

const READ_ONLY_CAPABILITIES = new Set<Capability>(["observe_public", "observe_private", "read_artifact"])

// Effectful verbs require a non-read-only capability. Used by both the node
// (enforcement) and clients (disable controls honestly).
export function verbAllowedByCapabilities(verb: BridgeRequestVerb, capabilities: ReadonlyArray<Capability>): boolean {
  const has = (c: Capability) => capabilities.includes(c)
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
      // read/list/subscribe/snapshot/history/pair/clients are observe-class.
      return capabilities.length > 0
  }
}

export function isReadOnlyCapabilitySet(capabilities: ReadonlyArray<Capability>): boolean {
  return capabilities.length > 0 && capabilities.every((c) => READ_ONLY_CAPABILITIES.has(c))
}
