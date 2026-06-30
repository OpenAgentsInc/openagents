// Generic Khala chat — the client flow state machine for the `/khala` chat box.
//
// This is the GENERIC public Khala chat (NOT the Autopilot Concierge /
// onboarding intake). It mirrors the CHAT PART of the autopilot onboarding flow
// (streaming, markdown, scroll, composer) but is deliberately minimal: a running
// transcript, a composer draft, a request status, an in-flight streaming reply,
// a single pending-turn descriptor the subscription keys off, and an info-popup
// open flag. It is STATELESS end-to-end: there is no session id, no durable
// resume, no persistence — the subscription re-sends the whole running message
// list to `POST /api/khala/chat` each turn.

import { Option, Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

// MODEL -------------------------------------------------------------------

// A single rendered transcript turn. user = the visitor; assistant = Khala.
export const KhalaChatTurn = S.Struct({
  role: S.Literals(['user', 'assistant']),
  content: S.String,
})
export type KhalaChatTurn = typeof KhalaChatTurn.Type

// The request lifecycle for a turn:
//   - `idle`        — no request in flight.
//   - `submitting`  — a turn is posted; the SSE stream may not have opened yet.
//   - `streaming`   — the assistant reply is landing token-by-token.
//   - `error`       — the last turn failed; `errorReason` carries the message.
export const KhalaChatStatus = S.Literals([
  'idle',
  'submitting',
  'streaming',
  'error',
])
export type KhalaChatStatus = typeof KhalaChatStatus.Type

// A turn waiting for (or receiving) its streamed reply. The subscription reads
// this to open the SSE stream and dispatch deltas; `id` is the stable per-turn
// key so the stream opens EXACTLY ONCE per turn. `userText` is the just-sent
// user message; `history` is the prior transcript, so the subscription posts the
// full running conversation (stateless). `null` when no turn is pending.
export const KhalaChatPendingTurn = S.Struct({
  id: S.String,
  userText: S.String,
  history: S.Array(KhalaChatTurn),
})
export type KhalaChatPendingTurn = typeof KhalaChatPendingTurn.Type

export const KhalaChatModel = ts('KhalaChat', {
  composerDraft: S.String,
  status: KhalaChatStatus,
  errorReason: S.NullOr(S.String),
  transcript: S.Array(KhalaChatTurn),
  // The in-flight assistant reply, accumulating as SSE deltas arrive. `null`
  // when no reply is streaming. On completion it is committed to the transcript
  // and reset to null.
  streamingReply: S.NullOr(S.String),
  // The turn the streaming subscription should drive, or null when none is in
  // flight. Carries the per-turn id so the SSE stream opens exactly once.
  pendingTurn: S.NullOr(KhalaChatPendingTurn),
  // Local composer display state. The draft is still plain text; preview renders
  // it through the shared Markdown element, and expanded just changes composer
  // height. Neither is persisted into any public projection.
  composerPreview: S.Boolean,
  composerExpanded: S.Boolean,
  // Whether the "What is Khala?" info popup overlay is open.
  infoOpen: S.Boolean,
})
export type KhalaChatModel = typeof KhalaChatModel.Type

// INIT --------------------------------------------------------------------

export const initKhalaChatModel = (): KhalaChatModel =>
  KhalaChatModel({
    composerDraft: '',
    status: 'idle',
    errorReason: null,
    transcript: [],
    streamingReply: null,
    pendingTurn: null,
    composerPreview: false,
    composerExpanded: false,
    infoOpen: false,
  })

// STREAM WIRE -------------------------------------------------------------

// The narrow SSE wire the chat route emits (see
// `workers/api/src/khala-chat-routes.ts`):
//   event: delta  data: { "text": "…" }
//   event: done   data: { "done": true }
//   event: error  data: { "error": "…" }
export type KhalaChatStreamEvent =
  | Readonly<{ kind: 'delta'; text: string }>
  | Readonly<{ kind: 'done' }>
  | Readonly<{ kind: 'error'; reason: string }>

const DeltaPayload = S.Struct({ text: S.String })

// Parse one decoded SSE block (its `event` name + JSON `data` payload) into a
// typed stream event. Unknown events / malformed payloads yield `undefined` so
// the consumer simply skips them (forward-compatible, never throws).
export const parseKhalaChatStreamEvent = (
  event: string,
  data: unknown,
): KhalaChatStreamEvent | undefined => {
  if (event === 'delta') {
    return S.decodeUnknownOption(DeltaPayload)(data).pipe(
      Option.match({
        onNone: () => undefined,
        onSome: (payload): KhalaChatStreamEvent => ({
          kind: 'delta',
          text: payload.text,
        }),
      }),
    )
  }
  if (event === 'done') {
    return { kind: 'done' }
  }
  if (event === 'error') {
    return { kind: 'error', reason: 'stream_failed' }
  }
  return undefined
}
