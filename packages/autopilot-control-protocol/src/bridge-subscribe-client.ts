import type { BridgeRequestEnvelope } from "./bridge.js"
import { buildSubscribeRequest, parseEventBatch } from "./bridge-client.js"
import type { SessionEvent } from "./control.js"

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

// The node's bounded inline event-tail projection, as returned by the
// session.subscribe / session.history bridge verbs (control-sessions `events()`:
// { sessionRef, eventsPath, state, recentEvents }). Remote clients (RN/Expo
// fetch can't consume the SSE stream cleanly) render a live session-detail
// timeline by polling this verb and resuming from the highest eventIndex they
// have already seen. The rows are the loose, redaction-scanned
// ControlSessionEvent shape; we surface the fields a timeline plus the
// artifact/receipt round-trip need and TOLERATE unknown phases/kinds (e.g.
// future cloud.gce.* lane events) by passing them through rather than dropping
// or hard-failing the batch.
export type BridgeSessionEventRow = {
  sessionRef: string
  eventIndex: number
  phase: string
  state: string
  observedAt: string
  messageText?: string
  artifactRef?: string
  resultRef?: string
}

export type BridgeEventBatch = {
  sessionRef: string
  state: string
  // Only rows strictly newer than the requested cursor, ascending by eventIndex.
  events: BridgeSessionEventRow[]
  // The highest eventIndex in this batch (or the input cursor when empty): the
  // resume point to pass as `cursor` on the next poll. `-1` means nothing has
  // been seen yet (a fresh poll over an empty session).
  cursor: number
}

const asString = (value: unknown): string | undefined => (typeof value === "string" ? value : undefined)

// Parse the node's events() projection into a cursor-resumable batch. Pure +
// transport-agnostic; dedups by eventIndex against `sinceCursor` so a client can
// poll repeatedly without re-ingesting events it already has. `sinceCursor`
// defaults to -1 so a fresh poll (no cursor) includes eventIndex 0.
export function parseBridgeEventBatch(raw: unknown, sinceCursor = -1): BridgeEventBatch {
  const envelope = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>
  const sessionRef = asString(envelope.sessionRef) ?? ""
  const state = asString(envelope.state) ?? "unknown"
  const rows = Array.isArray(envelope.recentEvents) ? envelope.recentEvents : []
  let cursor = sinceCursor
  const events: BridgeSessionEventRow[] = []
  for (const entry of rows) {
    const row = (typeof entry === "object" && entry !== null ? entry : {}) as Record<string, unknown>
    const eventIndex = typeof row.eventIndex === "number" ? row.eventIndex : -1
    if (eventIndex <= sinceCursor) continue
    const messageText = asString(row.messageText)
    const artifactRef = asString(row.artifactRef)
    const resultRef = asString(row.resultRef)
    events.push({
      sessionRef: asString(row.sessionRef) ?? sessionRef,
      eventIndex,
      phase: asString(row.phase) ?? "unknown",
      state: asString(row.state) ?? state,
      observedAt: asString(row.observedAt) ?? "",
      ...(messageText === undefined ? {} : { messageText }),
      ...(artifactRef === undefined ? {} : { artifactRef }),
      ...(resultRef === undefined ? {} : { resultRef }),
    })
    if (eventIndex > cursor) cursor = eventIndex
  }
  events.sort((a, b) => a.eventIndex - b.eventIndex)
  return { sessionRef, state, events, cursor }
}
