import type { BridgeRequestEnvelope, BridgeRequestVerb } from "./bridge.js"
import {
  decodeSessionEvent,
  decodeSessionSummary,
  type SessionEvent,
  type SessionSummary,
} from "./control.js"

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

// session.cancel is a session-scoped write verb (cancel capability). Same
// envelope shape as the reads; the node gates it on the stored credential.
export function buildCancelRequest(input: SessionReadRequestInput): BridgeRequestEnvelope {
  return buildReadRequest("session.cancel", input)
}

// artifact.read is a session-scoped read verb (read_artifact capability). It
// returns the retained proof/failure artifact a completed session produced —
// projection-safe + redaction-scanned at write time, so safe to render inline.
// Same envelope shape as the other reads; the node gates it on the stored
// credential's read_artifact capability.
export function buildArtifactReadRequest(input: SessionReadRequestInput): BridgeRequestEnvelope {
  return buildReadRequest("artifact.read", input)
}

export function parseListResponse(raw: unknown): SessionSummary[] {
  if (!Array.isArray(raw)) throw new TypeError("Expected session list response to be an array")
  return raw.map((row) => decodeSessionSummary(row))
}

export function parseEventBatch(raw: unknown): SessionEvent[] {
  if (!Array.isArray(raw)) throw new TypeError("Expected event batch response to be an array")
  return raw.map((event) => decodeSessionEvent(event))
}
