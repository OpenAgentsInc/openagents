import { describe, expect, test } from 'vitest'

import {
  ArtanisMindGatewayCandidates,
  artanisMindComplete,
} from './artanis-mind'

const geminiOk = JSON.stringify({
  candidates: [{ content: { parts: [{ text: 'verify capability and heartbeat' }] } }],
})

describe('artanis cloud mind', () => {
  test('serves via the AI gateway when a gateway resolves', async () => {
    const fetchImpl: typeof fetch = async input => {
      const url = String(input)
      if (url.includes('gateway.ai.cloudflare.com')) {
        return new Response(geminiOk, { status: 200 })
      }
      throw new Error('direct path must not be reached')
    }
    const result = await artanisMindComplete({
      apiKey: 'k',
      fetchImpl,
      prompt: 'p',
      system: 's',
    })
    expect('error' in result).toBe(false)
    if ('error' in result) return
    expect(result.servedVia).toBe('cloudflare_ai_gateway')
    expect(result.gatewayId).toBe(ArtanisMindGatewayCandidates[0])
    expect(result.text).toContain('verify')
  })

  test('falls back to direct Google when every gateway candidate fails', async () => {
    const fetchImpl: typeof fetch = async input => {
      const url = String(input)
      if (url.includes('gateway.ai.cloudflare.com')) {
        return new Response('{"error":[{"code":2009}]}', { status: 401 })
      }
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
  })

  test('reports typed unavailability with attempt evidence when all paths fail', async () => {
    const fetchImpl: typeof fetch = async () =>
      new Response('nope', { status: 500 })
    const result = await artanisMindComplete({
      apiKey: 'k',
      fetchImpl,
      gatewayId: 'only-one',
      prompt: 'p',
      system: 's',
    })
    expect('error' in result).toBe(true)
    if (!('error' in result)) return
    expect(result.error).toBe('artanis_mind_unavailable')
    expect(result.attempts.length).toBe(2)
  })
})
