// Gym — GPT-OSS live streaming sample call (#6167).
//
// The production `SampleStream`: calls the OpenAgents OpenAI-compatible gateway
// at `/api/v1/chat/completions` with `stream: true`, reads the SSE via
// `response.body.getReader()` + `TextDecoder`, parses `data: {json}` frames,
// accumulates content deltas, and reads the server-measured
// `openagents.telemetry` block off the terminal frame. It stamps CLIENT-side
// timing with perf marks (request start, first content byte, end) and hands both
// the client timing and the parsed server telemetry to `reconcileSample`.
//
// AUTH: the browser sends `credentials: 'include'` so the logged-in owner
// session cookie authenticates the call. This lane is billed by the hour, so
// there is no per-call balance gate — but the route is auth/owner-gated and the
// runner enforces a hard in-flight cap (MAX_IN_FLIGHT).
//
// This module does IO; the offline tests drive the PURE runner with a fake
// `SampleStream` instead, so the runner/aggregation never hit the network.

import {
  arrayFromUnknown,
  parseJsonRecord,
  recordFromUnknown,
} from '../../../json-boundary'
import { currentUnixMs } from '../../../time-format'
import {
  GPT_OSS_MODEL_ID,
  parseServerTelemetry,
  reconcileSample,
  type ClientTiming,
  type SampleResult,
  type SampleStream,
  type ServerTelemetry,
} from './runner'

const CHAT_COMPLETIONS_PATH = '/api/v1/chat/completions'

// A monotonic clock for client perf marks. Falls back to the runtime time
// helper if performance is unavailable (it is present in every modern browser).
const now = (): number =>
  typeof performance !== 'undefined' &&
  typeof performance.now === 'function'
    ? performance.now()
    : currentUnixMs()

// Read one chat message pair from the prompt.
const messagesFor = (prompt: string): ReadonlyArray<{ role: string; content: string }> => [
  { role: 'user', content: prompt },
]

// Parse a single SSE `data:` line's JSON payload into the bits we need: the
// content delta (if any) and the server `openagents.telemetry` block (terminal
// frame). Returns null for `[DONE]` / non-JSON / unparseable frames.
type ParsedFrame = Readonly<{
  contentDelta: string
  telemetry: ServerTelemetry | null
  finished: boolean
}>

export const parseSseDataLine = (line: string): ParsedFrame | null => {
  const trimmed = line.trim()
  if (!trimmed.startsWith('data:')) {
    return null
  }
  const payload = trimmed.slice('data:'.length).trim()
  if (payload === '' || payload === '[DONE]') {
    return null
  }
  // Decode through the named JSON boundary (no raw JSON.parse / assertions).
  const record = parseJsonRecord(payload)
  if (record === undefined) {
    return null
  }
  const choices = arrayFromUnknown(record.choices) ?? []
  const first = recordFromUnknown(choices[0])
  const delta = recordFromUnknown(first?.delta) ?? {}
  const content = typeof delta.content === 'string' ? delta.content : ''
  const finished =
    first?.finish_reason !== null && first?.finish_reason !== undefined
  const openagents = recordFromUnknown(record.openagents)
  const telemetry =
    openagents === undefined ? null : parseServerTelemetry(openagents.telemetry)
  return { contentDelta: content, telemetry, finished }
}

// The configurable bits of the live stream (the prompt + an injectable fetch for
// testing the IO wiring without a real network).
export type LiveStreamConfig = Readonly<{
  prompt: string
  model?: string
  fetchFn?: typeof fetch
}>

// Build a production `SampleStream` bound to a prompt. Each invocation fires one
// streaming request, measures it, and resolves with a reconciled SampleResult.
// A network/HTTP failure resolves as a FAILED sample (never rejects, never
// fabricates latency) so one bad sample can't wedge the run.
export const liveSampleStream = (config: LiveStreamConfig): SampleStream => {
  const fetchFn = config.fetchFn ?? fetch
  const model = config.model ?? GPT_OSS_MODEL_ID

  return async ({ index, signal }): Promise<SampleResult> => {
    const startMs = now()
    let firstContentByteMs: number | undefined
    let endMs: number | undefined
    let generationStartMs: number | undefined
    let observedContentDeltas = 0
    let server: ServerTelemetry | null = null

    try {
      const response = await fetchFn(CHAT_COMPLETIONS_PATH, {
        method: 'POST',
        credentials: 'include',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          model,
          messages: messagesFor(config.prompt),
          stream: true,
        }),
        ...(signal === undefined ? {} : { signal }),
      })

      if (!response.ok || response.body === null) {
        return reconcileSample({
          index,
          status: 'failed',
          error: `gateway responded ${response.status}`,
          client: emptyTiming(),
          server: null,
        })
      }

      const reader = response.body.getReader()
      const decoder = new TextDecoder()
      let buffer = ''
      let finished = false

      for (;;) {
        const { done, value } = await reader.read()
        if (done) {
          break
        }
        buffer += decoder.decode(value, { stream: true })
        // SSE frames are separated by blank lines; process complete lines.
        let newlineIndex = buffer.indexOf('\n')
        while (newlineIndex !== -1) {
          const line = buffer.slice(0, newlineIndex)
          buffer = buffer.slice(newlineIndex + 1)
          const frame = parseSseDataLine(line)
          if (frame !== null) {
            if (frame.contentDelta !== '') {
              if (firstContentByteMs === undefined) {
                firstContentByteMs = now() - startMs
                generationStartMs = now()
              }
              observedContentDeltas += 1
            }
            if (frame.telemetry !== null) {
              server = frame.telemetry
            }
            if (frame.finished) {
              finished = true
            }
          }
          newlineIndex = buffer.indexOf('\n')
        }
      }

      endMs = now() - startMs
      const generationMs =
        generationStartMs === undefined ? undefined : now() - generationStartMs

      // A finished stream with zero content deltas is a non-content run: its
      // streaming latency is honestly not_measured (never a fabricated 0).
      if (!finished && observedContentDeltas === 0 && server === null) {
        return reconcileSample({
          index,
          status: 'failed',
          error: 'stream ended without content or telemetry',
          client: emptyTiming(),
          server: null,
        })
      }

      const client: ClientTiming = {
        firstContentByteMs,
        endMs,
        generationMs,
        observedContentDeltas,
      }
      return reconcileSample({ index, status: 'ok', client, server })
    } catch (cause) {
      const error =
        cause instanceof Error ? cause.message : 'streaming request failed'
      return reconcileSample({
        index,
        status: 'failed',
        error,
        client: emptyTiming(),
        server: null,
      })
    }
  }
}

const emptyTiming = (): ClientTiming => ({
  firstContentByteMs: undefined,
  endMs: undefined,
  generationMs: undefined,
  observedContentDeltas: 0,
})
