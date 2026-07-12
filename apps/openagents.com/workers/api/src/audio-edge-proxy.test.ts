import { describe, expect, test } from 'vitest'
import { AUDIO_EDGE_STREAM_PATH, makeAudioEdgeProxy } from './audio-edge-proxy'

const request = (grant = 'g'.repeat(64)) => new Request(`https://openagents.com${AUDIO_EDGE_STREAM_PATH}`, { headers: { Upgrade: 'websocket', 'x-openagents-audio-grant': grant } })
describe('least-privilege audio WebSocket edge', () => {
  test('forwards only the bounded app grant and host-only Google identity token', async () => {
    let captured: { url: string; init?: RequestInit } | undefined
    const proxy = makeAudioEdgeProxy({ identityToken: async () => 'identity-token', fetchUpstream: async (url, init) => { captured = { url: String(url), init }; return { status: 101 } as Response } })
    const response = await proxy(request(), { OPENAGENTS_AUDIO_CLOUD_RUN_URL: 'https://audio.run.app' })
    expect(response.status).toBe(101); expect(captured?.url).toBe('https://audio.run.app/v1/stream')
    expect(new Headers(captured?.init?.headers).get('authorization')).toBe('Bearer identity-token')
    expect(new Headers(captured?.init?.headers).get('x-openagents-audio-grant')).toBe('g'.repeat(64))
    expect(JSON.stringify(captured)).not.toContain('private')
  })
  test('fails closed for non-upgrade, query credentials, bad grants, missing identity, and upstream refusal', async () => {
    const proxy = makeAudioEdgeProxy({ identityToken: async () => undefined, fetchUpstream: fetch })
    expect((await proxy(new Request('https://openagents.com/v1/stream'), {})).status).toBe(426)
    expect((await proxy(request('short'), {})).status).toBe(400)
    expect((await proxy(new Request('https://openagents.com/v1/stream?token=x', { headers: { Upgrade: 'websocket', 'x-openagents-audio-grant': 'g'.repeat(64) } }), {})).status).toBe(400)
    expect((await proxy(request(), { OPENAGENTS_AUDIO_CLOUD_RUN_URL: 'https://audio.run.app' })).status).toBe(503)
    const refused = makeAudioEdgeProxy({ identityToken: async () => 'token', fetchUpstream: async () => new Response(null, { status: 403 }) })
    expect((await refused(request(), { OPENAGENTS_AUDIO_CLOUD_RUN_URL: 'https://audio.run.app' })).status).toBe(502)
  })
})
