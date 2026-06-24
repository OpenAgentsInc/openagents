import { describe, expect, test } from 'vitest'

import {
  buyModeRelayPublisherForEnv,
  makeLiveBuyModeRelayPublisher,
} from './buy-mode-live-publisher'

class MemoryRelaySocket {
  private readonly messageHandlers: Array<(event: { data?: unknown }) => void> =
    []

  readonly sent: string[] = []

  addEventListener = (
    type: 'close' | 'error' | 'message',
    handler: (event: { data?: unknown }) => void,
  ) => {
    if (type === 'message') {
      this.messageHandlers.push(handler)
    }
  }

  close = () => {}

  send = (data: string) => {
    this.sent.push(data)
    const parsed = JSON.parse(data) as ['EVENT', { id: string }]
    queueMicrotask(() => {
      for (const handler of this.messageHandlers) {
        handler({ data: JSON.stringify(['OK', parsed[1].id, true, '']) })
      }
    })
  }
}

class AuthRequiredRelaySocket {
  private readonly messageHandlers: Array<(event: { data?: unknown }) => void> =
    []

  private authenticated = false

  readonly sent: string[] = []

  addEventListener = (
    type: 'close' | 'error' | 'message',
    handler: (event: { data?: unknown }) => void,
  ) => {
    if (type === 'message') {
      this.messageHandlers.push(handler)
      queueMicrotask(() => {
        handler({ data: JSON.stringify(['AUTH', 'challenge.public.test']) })
      })
    }
  }

  close = () => {}

  send = (data: string) => {
    this.sent.push(data)
    const parsed = JSON.parse(data) as [
      'AUTH' | 'EVENT',
      { id: string; kind: number },
    ]
    queueMicrotask(() => {
      const accepted =
        (parsed[0] === 'AUTH' && parsed[1].kind === 22242) ||
        (parsed[0] === 'EVENT' && this.authenticated)

      if (parsed[0] === 'AUTH' && parsed[1].kind === 22242 && accepted) {
        this.authenticated = true
      }

      for (const handler of this.messageHandlers) {
        handler({ data: JSON.stringify(['OK', parsed[1].id, accepted, '']) })
      }
    })
  }
}

const jobRequest = {
  campaignId: 'campaign.test',
  content: 'public prompt',
  providerPubkeys: ['22'.repeat(32)],
  relayUrl: 'wss://relay.openagents.com',
  requestEvent: {
    content: 'public prompt',
    created_at: 1_785_000_000,
    kind: 5050,
    tags: [['p', '22'.repeat(32)]],
  },
}

describe('buy-mode live publisher', () => {
  test('is unconfigured by default', () => {
    expect(buyModeRelayPublisherForEnv({})).toBeUndefined()
    expect(buyModeRelayPublisherForEnv({
      BUY_MODE_MARKET_SECRET_KEY: '',
    })).toBeUndefined()
  })

  test('signs and publishes request templates without exposing private material', async () => {
    const socket = new MemoryRelaySocket()
    const publisher = makeLiveBuyModeRelayPublisher({
      connect: async () => socket,
      marketSecretKeyHex: '11'.repeat(32),
      publishTimeoutMs: 100,
    })
    const receipt = await publisher.publishJobRequest(jobRequest)

    expect(receipt.accepted).toBe(true)
    expect(receipt.requestEventId).toMatch(/^[0-9a-f]{64}$/)
    expect(receipt.relayRef).toMatch(/^relay\.public\.market\./)
    expect(socket.sent).toHaveLength(1)
    expect(JSON.stringify(receipt)).not.toContain('public prompt')
    expect(JSON.stringify(receipt)).not.toContain('11'.repeat(32))
  })

  test('answers relay auth challenges before publishing request templates', async () => {
    const socket = new AuthRequiredRelaySocket()
    const publisher = makeLiveBuyModeRelayPublisher({
      authChallengeWaitMs: 100,
      connect: async () => socket,
      marketSecretKeyHex: '11'.repeat(32),
      publishTimeoutMs: 100,
    })
    const receipt = await publisher.publishJobRequest(jobRequest)
    const sentMessages = socket.sent.map(serialized => {
      const parsed = JSON.parse(serialized) as [
        'AUTH' | 'EVENT',
        { kind: number },
      ]
      return [parsed[0], parsed[1].kind]
    })

    expect(receipt.accepted).toBe(true)
    expect(sentMessages).toEqual([
      ['AUTH', 22242],
      ['EVENT', 5050],
    ])
  })

  test('fails closed for malformed request templates', async () => {
    const publisher = makeLiveBuyModeRelayPublisher({
      marketSecretKeyHex: '11'.repeat(32),
      publishTimeoutMs: 1,
    })
    const receipt = await publisher.publishJobRequest({
      ...jobRequest,
      requestEvent: { kind: 5050 },
    })

    expect(receipt.accepted).toBe(false)
    expect(receipt.relayRef).toContain('request_template_invalid')
  })
})
