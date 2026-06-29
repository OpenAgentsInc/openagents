// Stub/echo provider adapter for the inference gateway (#5476).
//
// Ships so the `/v1/chat/completions` route works end-to-end in tests and on the
// (inert, flagged-off) Worker without a real provider. It echoes the last user
// message and reports a deterministic `usage` object derived from a trivial
// whitespace token count, so the route + metering-hook seams are exercisable.
// Phase-2 replaces dispatch to this adapter with the real Fireworks (#5479),
// Vertex (#5480), and passthrough (#5481) adapters; nothing here implies a real
// provider call or any token cost.

import { Effect } from 'effect'

import {
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamChunk,
  type InferenceUsage,
} from './provider-adapter'

export const STUB_ECHO_ADAPTER_ID = 'stub-echo'

const countTokens = (text: string): number =>
  text.trim().length === 0 ? 0 : text.trim().split(/\s+/).length

const lastUserContent = (request: InferenceRequest): string => {
  const reversed = [...request.messages].reverse()
  const lastUser = reversed.find(message => message.role === 'user')
  return lastUser?.content ?? ''
}

const usageFor = (request: InferenceRequest, reply: string): InferenceUsage => {
  const promptTokens = request.messages.reduce(
    (total, message) => total + countTokens(message.content),
    0,
  )
  const completionTokens = countTokens(reply)
  return {
    promptTokens,
    completionTokens,
    totalTokens: promptTokens + completionTokens,
  }
}

export const stubEchoAdapter: InferenceProviderAdapter = {
  id: STUB_ECHO_ADAPTER_ID,
  complete: (request: InferenceRequest) =>
    Effect.sync((): InferenceResult => {
      const reply = lastUserContent(request)
      return {
        content: reply,
        finishReason: 'stop',
        servedModel: request.model,
        usage: usageFor(request, reply),
      }
    }),
  stream: (request: InferenceRequest) =>
    Effect.sync((): ReadonlyArray<InferenceStreamChunk> => {
      const reply = lastUserContent(request)
      const usage = usageFor(request, reply)
      // Two frames: one content delta, then a terminal frame carrying the
      // receipt-first usage so the metering hook can settle from real counts.
      const contentChunk: InferenceStreamChunk = { contentDelta: reply }
      const terminalChunk: InferenceStreamChunk = {
        contentDelta: '',
        finishReason: 'stop',
        servedModel: request.model,
        usage,
      }
      return [contentChunk, terminalChunk]
    }),
}
