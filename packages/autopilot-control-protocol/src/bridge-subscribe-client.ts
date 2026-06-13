import type { BridgeRequestEnvelope } from "./bridge"
import { buildSubscribeRequest, parseEventBatch } from "./bridge-client"
import type { SessionEvent } from "./control"

export type BuildSubscribeEnvelopeInput = {
  pairingRef: string
  capabilityRef: string
  sessionRef: string
  clientRequestId: string
  cursor?: number
}

export function buildSubscribeEnvelope(input: BuildSubscribeEnvelopeInput): BridgeRequestEnvelope {
  return buildSubscribeRequest({
    ...input,
    idempotencyKey: input.clientRequestId,
  })
}

export function parseEventBatchResponse(raw: unknown): SessionEvent[] {
  return parseEventBatch(raw)
}
