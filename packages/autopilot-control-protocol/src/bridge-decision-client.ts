import {
  verbAllowedByCapabilities,
  type BridgeRequestEnvelope,
  type Capability,
} from "./bridge"
import type { DecisionVerb } from "./decision"

export type BuildDecisionResolveEnvelopeInput = {
  pairingRef: string
  capabilityRef: string
  requestId: string
  verb: DecisionVerb
  clientRequestId: string
}

export type DecisionResolveEnvelope = BridgeRequestEnvelope & {
  requestId: string
  decisionVerb: DecisionVerb
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
  }
}

export function canResolveDecision(capabilities: ReadonlyArray<Capability>): boolean {
  return verbAllowedByCapabilities("decision.resolve", capabilities)
}
