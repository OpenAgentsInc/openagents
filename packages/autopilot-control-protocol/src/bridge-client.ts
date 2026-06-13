import type { BridgeRequestEnvelope, BridgeRequestVerb } from "./bridge"
import {
  decodeSessionEvent,
  decodeSessionSummary,
  type SessionEvent,
  type SessionSummary,
} from "./control"

type BaseReadRequestInput = {
  pairingRef: string
  capabilityRef: string
  clientRequestId: string
  idempotencyKey: string
}

type SessionReadRequestInput = BaseReadRequestInput & {
  sessionRef: string
  cursor?: number
}

function buildReadRequest(
  verb: BridgeRequestVerb,
  input: BaseReadRequestInput & { sessionRef?: string; cursor?: number },
): BridgeRequestEnvelope {
  return {
    verb,
    clientRequestId: input.clientRequestId,
    idempotencyKey: input.idempotencyKey,
    pairingRef: input.pairingRef,
    capabilityRef: input.capabilityRef,
    ...(input.sessionRef === undefined ? {} : { sessionRef: input.sessionRef }),
    ...(input.cursor === undefined ? {} : { cursor: input.cursor }),
  }
}

export function buildListRequest(input: BaseReadRequestInput): BridgeRequestEnvelope {
  return buildReadRequest("session.list", input)
}

export function buildSubscribeRequest(input: SessionReadRequestInput): BridgeRequestEnvelope {
  return buildReadRequest("session.subscribe", input)
}

export function buildSnapshotRequest(input: SessionReadRequestInput): BridgeRequestEnvelope {
  return buildReadRequest("session.snapshot", input)
}

export function buildHistoryRequest(input: SessionReadRequestInput): BridgeRequestEnvelope {
  return buildReadRequest("session.history", input)
}

export function parseListResponse(raw: unknown): SessionSummary[] {
  if (!Array.isArray(raw)) throw new TypeError("Expected session list response to be an array")
  return raw.map((row) => decodeSessionSummary(row))
}

export function parseEventBatch(raw: unknown): SessionEvent[] {
  if (!Array.isArray(raw)) throw new TypeError("Expected event batch response to be an array")
  return raw.map((event) => decodeSessionEvent(event))
}
