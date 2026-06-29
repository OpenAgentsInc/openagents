import { describe, expect, test } from 'vitest'

import { NOT_MEASURED } from './runner'
import { liveSampleStream, parseSseDataLine } from './stream'

describe('gym-oss SSE frame parsing', () => {
  test('parses a content delta frame', () => {
    const frame = parseSseDataLine(
      'data: {"choices":[{"delta":{"content":"Hi"},"finish_reason":null}]}',
    )
    expect(frame?.contentDelta).toBe('Hi')
    expect(frame?.finished).toBe(false)
    expect(frame?.telemetry).toBeNull()
  })

  test('parses the terminal frame carrying openagents.telemetry', () => {
    const frame = parseSseDataLine(
      'data: {"choices":[{"delta":{},"finish_reason":"stop"}],"openagents":{"telemetry":{"ttftMs":120,"totalWallClockMs":1120,"promptTokens":10,"completionTokens":200,"totalTokens":210,"cachedInputTokens":0}}}',
    )
    expect(frame?.finished).toBe(true)
    expect(frame?.telemetry?.ttftMs).toBe(120)
    expect(frame?.telemetry?.completionTokens).toBe(200)
  })

  test('ignores [DONE] and non-data lines', () => {
    expect(parseSseDataLine('data: [DONE]')).toBeNull()
    expect(parseSseDataLine(': keep-alive')).toBeNull()
    expect(parseSseDataLine('')).toBeNull()
  })
})

// A fake streaming Response built from canned SSE text, so the live stream's IO
// wiring is exercised WITHOUT hitting the network.
const sseResponse = (lines: ReadonlyArray<string>): Response => {
  const body = lines.map(line => `${line}\n`).join('')
  const encoder = new TextEncoder()
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.enqueue(encoder.encode(body))
      controller.close()
    },
  })
  return new Response(stream, { status: 200 })
}

describe('gym-oss live sample stream (offline, fake fetch)', () => {
  test('reads server telemetry off the terminal frame and reports OK', async () => {
    const fetchFn = (async () =>
      sseResponse([
        'data: {"choices":[{"delta":{"content":"Hello"},"finish_reason":null}]}',
        'data: {"choices":[{"delta":{"content":" world"},"finish_reason":null}]}',
        'data: {"choices":[{"delta":{},"finish_reason":"stop"}],"openagents":{"telemetry":{"ttftMs":90,"totalWallClockMs":1090,"promptTokens":5,"completionTokens":200,"totalTokens":205,"cachedInputTokens":0}}}',
        'data: [DONE]',
      ])) as unknown as typeof fetch

    const stream = liveSampleStream({ prompt: 'hi', fetchFn })
    const result = await stream({ index: 0 })
    expect(result.status).toBe('ok')
    expect(result.ttftMs).toBe(90)
    expect(result.source.ttft).toBe('server')
    // Derived from server completion tokens (200) over generation window
    // (total - ttft = 1000ms) -> 200 tok/s.
    expect(result.perceivedTps).toBe(200)
  })

  test('an HTTP error resolves as a FAILED sample, never a fabricated latency', async () => {
    const fetchFn = (async () =>
      new Response('nope', { status: 503 })) as unknown as typeof fetch
    const stream = liveSampleStream({ prompt: 'hi', fetchFn })
    const result = await stream({ index: 0 })
    expect(result.status).toBe('failed')
    expect(result.ttftMs).toBe(NOT_MEASURED)
    expect(result.perceivedTps).toBe(NOT_MEASURED)
    expect(result.error).toContain('503')
  })

  test('a thrown fetch resolves as a FAILED sample (does not reject)', async () => {
    const fetchFn = (async () => {
      throw new Error('network down')
    }) as unknown as typeof fetch
    const stream = liveSampleStream({ prompt: 'hi', fetchFn })
    const result = await stream({ index: 0 })
    expect(result.status).toBe('failed')
    expect(result.error).toContain('network down')
  })
})
