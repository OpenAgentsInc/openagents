import { describe, expect, test } from 'vitest'

import {
  ArtanisMindModelDefault,
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
    let requestBody: {
      generationConfig?: { thinkingConfig?: Record<string, unknown> }
    } | undefined
    const fetchImpl: typeof fetch = async (input, init) => {
      const url = String(input)
      seen.push(url)
      requestBody = JSON.parse(String(init?.body)) as typeof requestBody
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
    expect(result.model).toBe('gemma-4-31b-it')
    expect(result.text).toContain('verify')
    expect(seen).toEqual([
      expect.stringContaining(
        `/models/${ArtanisMindModelDefault}:generateContent`,
      ),
    ])
    expect(requestBody?.generationConfig?.thinkingConfig).toEqual({
      thinkingLevel: 'minimal',
    })
  })

  test('never exposes Gemma private thought parts as Sarah response text', async () => {
    const fetchImpl: typeof fetch = async () =>
      new Response(JSON.stringify({
        candidates: [{
          content: { parts: [
            { text: 'private scratchpad', thought: true },
            { text: 'Hello — what should we work on?' },
          ] },
        }],
      }), { status: 200 })
    const result = await artanisMindComplete({
      apiKey: 'k',
      fetchImpl,
      prompt: 'Hello',
      system: 'You are Sarah.',
    })
    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.text).toBe('Hello — what should we work on?')
  })

  test('keeps numeric thinking disabled for explicit legacy Gemini calls', async () => {
    let requestBody: {
      generationConfig?: { thinkingConfig?: Record<string, unknown> }
    } | undefined
    const fetchImpl: typeof fetch = async (_input, init) => {
      requestBody = JSON.parse(String(init?.body)) as typeof requestBody
      return new Response(geminiOk, { status: 200 })
    }
    await artanisMindComplete({
      apiKey: 'k',
      fetchImpl,
      model: 'gemini-3.5-flash',
      prompt: 'p',
      system: 's',
    })
    expect(requestBody?.generationConfig?.thinkingConfig).toEqual({
      thinkingBudget: 0,
    })
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
