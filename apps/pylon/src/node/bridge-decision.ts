// Bridge decision relay core (CL-11 / issue #4913).
//
// Pure, transport-agnostic module. Live control-server/session wiring belongs
// to CL-14; this file only tracks decision records and derives relay events.

import {
  applyExternalResolution,
  pendingDecision,
  resolveDecision,
  type DecisionRecord,
  type DecisionVerb,
} from "@openagentsinc/autopilot-control-protocol"

export type DecisionRegistry = {
  readonly decisions: ReadonlyMap<string, DecisionRecord>
}

export type DecisionRequestInput = {
  requestId: string
  actionRef: string
  expiresAtMs: number
}

export type DecisionAnswer = {
  requestId: string
  verb: DecisionVerb
}

export type ResolveOneOutcome = ReturnType<typeof resolveDecision>["outcome"]

export type DecisionBroadcastEvent = {
  name: "decision.resolved" | "decision.cancelled"
  requestId: string
}

export function createDecisionRegistry(): DecisionRegistry {
  return { decisions: new Map() }
}

export function requestDecision(
  reg: DecisionRegistry,
  input: DecisionRequestInput,
): DecisionRegistry {
  const decisions = new Map(reg.decisions)
  decisions.set(input.requestId, pendingDecision(input))
  return { decisions }
}

export function resolveOne(
  reg: DecisionRegistry,
  answer: DecisionAnswer,
  nowMs: number,
): { reg: DecisionRegistry; outcome: ResolveOneOutcome } {
  const existing = reg.decisions.get(answer.requestId)
  if (!existing) {
    return { reg, outcome: "unknown_request" }
  }

  const resolved = resolveDecision(existing, answer, nowMs)
  const decisions = new Map(reg.decisions)
  decisions.set(answer.requestId, resolved.record)

  return { reg: { decisions }, outcome: resolved.outcome }
}

export function applyExternalDecisionResolution(
  reg: DecisionRegistry,
  requestId: string,
  external: { state: "resolved" | "cancelled"; verb?: DecisionVerb },
): DecisionRegistry {
  const existing = reg.decisions.get(requestId)
  if (!existing) {
    return reg
  }

  const decisions = new Map(reg.decisions)
  decisions.set(requestId, applyExternalResolution(existing, external))
  return { decisions }
}

export function broadcastResolution(
  reg: DecisionRegistry,
  requestId: string,
): { reg: DecisionRegistry; event: DecisionBroadcastEvent | null } {
  const existing = reg.decisions.get(requestId)

  if (existing?.state === "resolved") {
    return { reg, event: { name: "decision.resolved", requestId } }
  }

  if (existing?.state === "cancelled") {
    return { reg, event: { name: "decision.cancelled", requestId } }
  }

  return { reg, event: null }
}

export function pendingForReplay(reg: DecisionRegistry): DecisionRecord[] {
  return [...reg.decisions.values()].filter((decision) => decision.state === "pending")
}
