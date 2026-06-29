import {
  verbAllowedByCapabilities,
  type BridgeRequestEnvelope,
  type Capability,
} from "./bridge.js"
import type { DecisionVerb } from "./decision.js"

export type BuildDecisionResolveEnvelopeInput = {
  pairingRef: string
  capabilityRef: string
  requestId: string
  verb: DecisionVerb
  clientRequestId: string
  // Free-text answer, required only when verb === "answer".
  answer?: string
}

export type DecisionResolveEnvelope = BridgeRequestEnvelope & {
  requestId: string
  decisionVerb: DecisionVerb
  answer?: string
}

export function buildDecisionResolveEnvelope(
  input: BuildDecisionResolveEnvelopeInput,
): DecisionResolveEnvelope {
  return {
    verb: "decision.resolve",
    clientRequestId: input.clientRequestId,
    idempotencyKey: input.clientRequestId,
    pairingRef: input.pairingRef,
    capabilityRef: input.capabilityRef,
    requestId: input.requestId,
    decisionVerb: input.verb,
    ...(input.answer === undefined ? {} : { answer: input.answer }),
  }
}

export function canResolveDecision(capabilities: ReadonlyArray<Capability>): boolean {
  return verbAllowedByCapabilities("decision.resolve", capabilities)
}
