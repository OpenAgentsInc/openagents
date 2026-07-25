import { describe, expect, it } from 'vitest'

import { handlePublicNostrChatManifest } from './public-nostr-chat-routes'

describe('public Nostr chat routes', () => {
  it('publishes a public fail-closed deployment manifest', async () => {
    const response = handlePublicNostrChatManifest(
      new Request('https://openagents.com/api/public/nostr-chat/manifest'),
    )
    const body = (await response.json()) as {
      group: { id: string; naddr: string | null }
      readiness: string
      relay: { selfPubkey: string | null; websocketUrl: string }
    }

    expect(response.status).toBe(200)
    expect(response.headers.get('access-control-allow-origin')).toBe('*')
    expect(body.group.id).toBe('openagents-public')
    expect(body.group.naddr).toBeNull()
    expect(body.readiness).toBe('relay-self-required')
    expect(body.relay.selfPubkey).toBeNull()
    expect(body.relay.websocketUrl).toBe('wss://relay.openagents.com')
  })

  it('publishes a group address only with a configured relay self key', async () => {
    const response = handlePublicNostrChatManifest(
      new Request('https://openagents.com/api/public/nostr-chat/manifest'),
      'a'.repeat(64),
    )
    const body = (await response.json()) as {
      group: { naddr: string | null; nostrUri: string | null }
      readiness: string
    }

    expect(body.readiness).toBe('ready')
    expect(body.group.naddr).toMatch(/^naddr1/)
    expect(body.group.nostrUri).toBe(`nostr:${body.group.naddr}`)
  })

  it('rejects mutation methods', () => {
    const response = handlePublicNostrChatManifest(
      new Request('https://openagents.com/api/public/nostr-chat/manifest', {
        method: 'POST',
      }),
    )

    expect(response.status).toBe(405)
  })
})
