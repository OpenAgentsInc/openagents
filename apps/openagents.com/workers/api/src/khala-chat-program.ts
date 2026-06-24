// Generic public Khala chat program: the stateless turn assembly + the
// streaming inference seam for the `/khala` chat demo.
//
// This is the GENERIC Khala chat — NOT the Autopilot Concierge / onboarding
// intake interview. It mirrors the onboarding streaming pattern
// (`autopilot-onboarding-program.ts` + `autopilot-onboarding-routes.ts`): it
// reaches Khala over the SAME provider-adapter registry + overflow dispatch the
// gateway uses (no external HTTP hop, no auth/credit gate), model
// `openagents/khala-mini`. The difference is the system prompt and the turn
// shape:
//   - System prompt = the Khala identity contract (`KHALA_IDENTITY_SYSTEM_PROMPT`
//     from `inference/khala-identity.ts`, first-person plural "we are Khala",
//     NEVER naming an underlying provider) + a light instruction that Khala is a
//     general assistant that may discuss its own capabilities, the
//     OpenAI-compatible API, and the public model ids — but NOT the
//     onboarding/concierge intake program.
//   - Stateless: the client sends the running message list each turn. There is
//     NO server session row, no durable resume, no persistence.
//
// The streaming inference seam reuses the onboarding `OnboardingStreamSource`
// shape (deltas async-iterable + final()) and the shared
// `dispatchOnboardingStreamSource` provider-adapter bridge, so the streaming +
// metering seam is not reinvented here.

import { Effect, Schema as S } from 'effect'

import type { OnboardingStreamSource } from './autopilot-onboarding-program'
import { OnboardingInferenceError } from './autopilot-onboarding-program'
import {
  type InferenceMessage,
  type InferenceRequest,
} from './inference/provider-adapter'
import { KHALA_IDENTITY_SYSTEM_PROMPT } from './inference/khala-identity'

// The live public Khala model for the generic chat demo (same model id the
// onboarding program uses; the cheapest-viable router lane).
export const KHALA_CHAT_MODEL = 'khala'

// Bounds: a single message body and the whole running conversation are capped so
// a public, unauthenticated demo cannot push an unbounded prompt through the
// shared inference pool. These are the cheap abuse guards the route applies
// before opening any stream.
export const KHALA_CHAT_MAX_MESSAGE_CHARS = 8_000
export const KHALA_CHAT_MAX_MESSAGES = 40
export const KHALA_CHAT_MAX_TOTAL_CHARS = 24_000

// A single chat turn from the client. Only user/assistant roles cross the
// boundary; the system prompt is rebuilt server-side every turn and never
// supplied by the client (so the identity contract can never be overridden).
export const KhalaChatMessage = S.Struct({
  role: S.Literals(['user', 'assistant']),
  content: S.String,
})
export type KhalaChatMessage = typeof KhalaChatMessage.Type

// The request body: the running conversation. The newest user message is the
// last element; prior assistant/user turns precede it. Stateless — the client
// re-sends the whole list each turn.
export const KhalaChatRequest = S.Struct({
  messages: S.Array(KhalaChatMessage),
})
export type KhalaChatRequest = typeof KhalaChatRequest.Type

// The generic instruction layered ON TOP of the identity contract. It tells
// Khala it is the OpenAgents inference orchestrator / a general assistant that
// may discuss its own capabilities, the OpenAI-compatible API, the public model
// ids, the base URL, and SSE streaming. It deliberately does NOT introduce an
// intake interview, an output spec, or any concierge scoping behavior.
export const KHALA_CHAT_INSTRUCTION = [
  'You are answering in a public chat demo on the OpenAgents website.',
  'You are the OpenAgents inference orchestrator: one OpenAI-compatible endpoint over a network of agents. You are a general-purpose assistant — answer whatever the user asks directly and helpfully.',
  'You may explain your own capabilities when asked: you are reachable as an OpenAI-compatible Chat Completions API at the base URL https://openagents.com/api/v1, the public model id is openagents/khala, and streaming works over standard Server-Sent Events.',
  'Do not run an intake interview, do not ask a fixed script of onboarding questions, and do not collect a business profile. Just have a normal, helpful conversation.',
].join(' ')

// STREAMING INFERENCE SEAM ------------------------------------------------

// The streaming inference seam. Given the assembled message list, return a
// source whose `deltas` async-iterable yields assistant TEXT increments as the
// upstream produces them. Reuses the onboarding `OnboardingStreamSource` shape
// and tagged error so the shared `dispatchOnboardingStreamSource` bridge and the
// production overflow dispatch apply unchanged. Tests inject a deterministic
// stub.
export type KhalaChatStreamSource = OnboardingStreamSource

export type KhalaChatStreamClient = (
  request: InferenceRequest,
) => Effect.Effect<KhalaChatStreamSource, OnboardingInferenceError>

// VALIDATION --------------------------------------------------------------

export class KhalaChatValidationError extends S.TaggedErrorClass<KhalaChatValidationError>()(
  'KhalaChatValidationError',
  {
    reason: S.String,
  },
) {}

// Validate the running conversation under the demo's bounds: at least one
// message, the last message is from the user, the message count and per-message
// and total character budgets are respected, and no message is empty. Returns
// the (unchanged) messages on success so the caller can build the request.
export const validateKhalaChatRequest = (
  request: KhalaChatRequest,
): Effect.Effect<ReadonlyArray<KhalaChatMessage>, KhalaChatValidationError> => {
  const messages = request.messages

  if (messages.length === 0) {
    return Effect.fail(
      new KhalaChatValidationError({ reason: 'messages must not be empty' }),
    )
  }

  if (messages.length > KHALA_CHAT_MAX_MESSAGES) {
    return Effect.fail(
      new KhalaChatValidationError({
        reason: `too many messages (max ${KHALA_CHAT_MAX_MESSAGES})`,
      }),
    )
  }

  const last = messages[messages.length - 1]
  if (last === undefined || last.role !== 'user') {
    return Effect.fail(
      new KhalaChatValidationError({
        reason: 'the last message must be a user message',
      }),
    )
  }

  const emptyMessage = messages.some(message => message.content.trim() === '')
  if (emptyMessage) {
    return Effect.fail(
      new KhalaChatValidationError({ reason: 'messages must not be empty' }),
    )
  }

  const oversizeMessage = messages.some(
    message => message.content.length > KHALA_CHAT_MAX_MESSAGE_CHARS,
  )
  if (oversizeMessage) {
    return Effect.fail(
      new KhalaChatValidationError({
        reason: `a message exceeds the ${KHALA_CHAT_MAX_MESSAGE_CHARS}-character limit`,
      }),
    )
  }

  const totalChars = messages.reduce(
    (sum, message) => sum + message.content.length,
    0,
  )
  if (totalChars > KHALA_CHAT_MAX_TOTAL_CHARS) {
    return Effect.fail(
      new KhalaChatValidationError({
        reason: `the conversation exceeds the ${KHALA_CHAT_MAX_TOTAL_CHARS}-character limit`,
      }),
    )
  }

  return Effect.succeed(messages)
}

// MESSAGE ASSEMBLY --------------------------------------------------------

// Build the inference message list for a turn: the Khala identity contract +
// the generic chat instruction as a leading system message, then the running
// user/assistant conversation. The identity prompt is FIRST so the identity
// contract binds the whole turn (the gateway guard backstops it regardless).
// The client never supplies the system message, so the identity contract can
// never be overridden by a crafted conversation.
export const buildKhalaChatMessages = (
  messages: ReadonlyArray<KhalaChatMessage>,
): ReadonlyArray<InferenceMessage> => [
  {
    role: 'system',
    content: `${KHALA_IDENTITY_SYSTEM_PROMPT} ${KHALA_CHAT_INSTRUCTION}`,
  },
  ...messages.map(message => ({ role: message.role, content: message.content })),
]

// Build the streaming inference request for a turn. `stream: true` so the
// provider-adapter opens an incremental stream; `passthroughParams` is empty
// (the demo does not forward arbitrary OpenAI params).
export const buildKhalaChatRequest = (
  messages: ReadonlyArray<KhalaChatMessage>,
): InferenceRequest => ({
  model: KHALA_CHAT_MODEL,
  messages: buildKhalaChatMessages(messages),
  stream: true,
  passthroughParams: {},
})
