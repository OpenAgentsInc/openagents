import { describe, expect, it } from 'vite-plus/test'
import {
  createSarahNostrRelayConsumer,
  handleSarahRelayOwnerMessage,
  isSarahNostrRelayPrimaryEnabled,
} from './sarah-nostr-relay-consumer'
import {
  createMemoryRelayPublisher,
  generateSarahNostrSigner,
} from '@openagentsinc/sarah'

describe('sarah-nostr-relay-consumer', () => {
  it('runs relay-primary path with stub agent and memory publisher', async () => {
    const signer = generateSarahNostrSigner()
    const memory = createMemoryRelayPublisher()
    const consumer = createSarahNostrRelayConsumer({
      signer,
      conversation: {
        ownerPubkey: '44'.repeat(32),
        sarahPubkey: signer.getPublicKey(),
        conversation: 'sarah.' + '55'.repeat(12),
      },
      publish: memory.publish,
      runAgent: async ({ onToolActivity }) => {
        onToolActivity({
          entry: 'tool.call',
          payload: { toolName: 'ping' },
        })
        return {
          ok: true,
          text: 'pong',
          usage: { totalTokens: 3, inputTokens: 1, outputTokens: 2 },
        }
      },
    })

    const result = await consumer.handleOwnerMessage({
      turnRef: 'turn.api.1',
      plaintext: 'ping',
    })
    expect(result.status).toBe('answered')
    expect(result.answerEvent?.content).toBe('pong')
    expect(memory.events.some(e => e.kind === 14)).toBe(true)
    expect(memory.events.some(e => e.kind === 44300)).toBe(true)
  })

  it('handleSarahRelayOwnerMessage one-shot helper', async () => {
    const signer = generateSarahNostrSigner()
    const result = await handleSarahRelayOwnerMessage({
      deps: {
        signer,
        conversation: {
          ownerPubkey: '66'.repeat(32),
          sarahPubkey: signer.getPublicKey(),
          conversation: 'sarah.' + '77'.repeat(12),
        },
        runAgent: async () => ({ ok: true, text: 'ok' }),
      },
      turnRef: 'turn.api.2',
      plaintext: 'hi',
    })
    expect(result.status).toBe('answered')
  })

  it('relay primary flag defaults off', () => {
    const prev = process.env.SARAH_NOSTR_RELAY_PRIMARY
    delete process.env.SARAH_NOSTR_RELAY_PRIMARY
    try {
      expect(isSarahNostrRelayPrimaryEnabled()).toBe(false)
      process.env.SARAH_NOSTR_RELAY_PRIMARY = '1'
      expect(isSarahNostrRelayPrimaryEnabled()).toBe(true)
    } finally {
      if (prev === undefined) delete process.env.SARAH_NOSTR_RELAY_PRIMARY
      else process.env.SARAH_NOSTR_RELAY_PRIMARY = prev
    }
  })
})
