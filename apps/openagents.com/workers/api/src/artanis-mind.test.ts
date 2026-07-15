import { describe, expect, test } from 'vitest'

import {
  ArtanisMindEscalatedMaxOutputTokens,
  artanisMindComplete,
} from './artanis-mind'

const geminiOk = JSON.stringify({
  candidates: [{ content: { parts: [{ text: 'verify capability and heartbeat' }] } }],
})

// A response cut off at the output cap: Gemini returns partial text with
// finishReason MAX_TOKENS. The cinder-atlas truncation artifacts ('mer',
// 're-ex', 'r.') on topic 7ba5d586 were exactly this.
const geminiTruncated = JSON.stringify({
  candidates: [
    {
      content: { parts: [{ text: 'To make sparkPayoutTargetReady true you r' }] },
      finishReason: 'MAX_TOKENS',
    },
  ],
})

describe('artanis cloud mind', () => {
  test('lowers image bytes as Gemini inlineData alongside the user text', async () => {
    let requestBody: Record<string, unknown> | undefined
    const fetchImpl: typeof fetch = async (_input, init) => {
      requestBody = JSON.parse(String(init?.body)) as Record<string, unknown>
      return new Response(geminiOk, { status: 200 })
    }
    const result = await artanisMindComplete({
      apiKey: 'k',
      fetchImpl,
      images: [{ dataBase64: 'AQID', mediaType: 'image/png' }],
      prompt: 'What color?',
      system: 's',
    })
    expect('error' in result).toBe(false)
    const contents = requestBody?.contents as Array<{
      parts: Array<Record<string, unknown>>
    }>
    expect(contents[0]?.parts).toEqual([
      { text: 'What color?' },
      { inlineData: { data: 'AQID', mimeType: 'image/png' } },
    ])
  })

  test('serves directly through Google AI Studio', async () => {
    const seen: string[] = []
    const fetchImpl: typeof fetch = async input => {
      const url = String(input)
      seen.push(url)
      return new Response(geminiOk, { status: 200 })
    }
    const result = await artanisMindComplete({
      apiKey: 'k',
      fetchImpl,
      prompt: 'p',
      system: 's',
    })
    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.servedVia).toBe('google_direct')
    expect(result.gatewayId).toBeNull()
    expect(result.text).toContain('verify')
    expect(seen).toEqual([
      expect.stringContaining('generativelanguage.googleapis.com'),
    ])
  })

  test('never returns truncated MAX_TOKENS text; escalates the cap once and uses the complete answer', async () => {
    const seenCaps: number[] = []
    const fetchImpl: typeof fetch = async (_input, init) => {
      const body = JSON.parse(String(init?.body)) as {
        generationConfig?: { maxOutputTokens?: number }
      }
      const cap = body.generationConfig?.maxOutputTokens ?? -1
      seenCaps.push(cap)
      // The low first cap truncates; the escalated retry completes.
      return new Response(
        cap < ArtanisMindEscalatedMaxOutputTokens ? geminiTruncated : geminiOk,
        { status: 200 },
      )
    }
    const result = await artanisMindComplete({
      apiKey: 'k',
      fetchImpl,
      maxOutputTokens: 1024,
      prompt: 'p',
      system: 's',
    })
    expect('error' in result).toBe(false)
    if ('error' in result) return
    // The complete answer, never the cut-off fragment.
    expect(result.text).toBe('verify capability and heartbeat')
    expect(result.text.endsWith('you r')).toBe(false)
    // The escalated cap was actually requested.
    expect(seenCaps).toContain(ArtanisMindEscalatedMaxOutputTokens)
  })

  test('reports unavailability (never partial text) when truncation persists after escalation', async () => {
    const fetchImpl: typeof fetch = async () =>
      new Response(geminiTruncated, { status: 200 })
    const result = await artanisMindComplete({
      apiKey: 'k',
      fetchImpl,
      prompt: 'p',
      system: 's',
    })
    // Truncated-forever must surface as typed unavailability, not as the
    // partial 'you r' fragment leaking into a posted reply.
    expect('error' in result).toBe(true)
    if (!('error' in result)) return
    expect(result.error).toBe('artanis_mind_unavailable')
    expect(
      result.attempts.some(attempt => attempt.detail.includes('truncated')),
    ).toBe(true)
  })

  test('reports typed unavailability with attempt evidence when all paths fail', async () => {
    const fetchImpl: typeof fetch = async () =>
      new Response('nope', { status: 500 })
    const result = await artanisMindComplete({
      apiKey: 'k',
      fetchImpl,
      prompt: 'p',
      system: 's',
    })
    expect('error' in result).toBe(true)
    if (!('error' in result)) return
    expect(result.error).toBe('artanis_mind_unavailable')
    expect(result.attempts.length).toBe(1)
  })
})
