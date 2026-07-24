import { describe, expect, it } from 'vite-plus/test'
import {
  conversationTagFromSarahThread,
  createSarahNostrTurnBridge,
  isSarahOwnerThread,
  resolveSarahNostrBridgeMode,
  tryCreateSarahNostrTurnBridgeFromEnv,
} from './sarah-nostr-turn-bridge'
import {
  generateSarahNostrSigner,
  testSarahNostrCipher,
  verifySignedEvent,
} from '@openagentsinc/sarah'

describe('sarah-nostr-turn-bridge', () => {
  it('maps thread.sarah.<digest> to conversation tag and rejects non-Sarah threads', () => {
    const thread = 'thread.sarah.' + 'ab'.repeat(12)
    expect(isSarahOwnerThread(thread)).toBe(true)
    expect(conversationTagFromSarahThread(thread)).toBe('sarah.' + 'ab'.repeat(12))
    expect(isSarahOwnerThread('thread.other.abc')).toBe(false)
    expect(conversationTagFromSarahThread('thread.other.abc')).toBeNull()
  })

  it('dual-publishes claim, tools, finish, and additive NIP-AM usage', () => {
    const signer = generateSarahNostrSigner()
    const ownerPubkey = '01'.repeat(32)
    const bridge = createSarahNostrTurnBridge({
      signer,
      ownerPubkey,
      conversationTag: 'sarah.' + 'cd'.repeat(12),
      cipher: testSarahNostrCipher(),
    })

    const started = bridge.startTurn('turn.bridge.1')
    expect(started).not.toBeNull()
    expect(started!.durable!.kind).toBe(44300)
    expect(verifySignedEvent(started!.durable!)).toBe(true)

    expect(bridge.startTurn('turn.bridge.1')).toBeNull()

    const tool = bridge.publishToolActivity({
      turnRef: 'turn.bridge.1',
      entry: 'tool.call',
      payload: { toolName: 'status' },
    })
    expect(tool.live?.kind).toBe(24200)

    const metric = bridge.publishUsageMetric({
      turnRef: 'turn.bridge.1',
      totalTokens: 42,
      inputTokens: 20,
      outputTokens: 22,
    })
    expect(metric.kind).toBe(44200)
    expect(verifySignedEvent(metric)).toBe(true)
    expect(metric.content).toContain('"totalTokens":42')
    expect(metric.content).not.toContain('privateKey')

    const finished = bridge.finishTurn({
      turnRef: 'turn.bridge.1',
      entry: 'turn.finished',
    })
    expect(finished.entry).toBe('turn.finished')
  })

  it('tryCreate from env returns null when mode is khala', () => {
    const prevShadow = process.env.SARAH_NOSTR_SHADOW_PUBLISH
    const prevMode = process.env.SARAH_NOSTR_RECORD_MODE
    delete process.env.SARAH_NOSTR_SHADOW_PUBLISH
    delete process.env.SARAH_NOSTR_RECORD_MODE
    try {
      expect(resolveSarahNostrBridgeMode()).toBe('khala')
      expect(
        tryCreateSarahNostrTurnBridgeFromEnv({
          threadId: 'thread.sarah.' + 'ee'.repeat(12),
        }),
      ).toBeNull()
    } finally {
      if (prevShadow !== undefined) process.env.SARAH_NOSTR_SHADOW_PUBLISH = prevShadow
      if (prevMode !== undefined) process.env.SARAH_NOSTR_RECORD_MODE = prevMode
    }
  })

  it('tryCreate from env builds a bridge when legacy SHADOW_PUBLISH=1', () => {
    const prevFlag = process.env.SARAH_NOSTR_SHADOW_PUBLISH
    const prevMode = process.env.SARAH_NOSTR_RECORD_MODE
    const prevOwner = process.env.SARAH_NOSTR_OWNER_PUBKEY
    const prevNodeEnv = process.env.NODE_ENV
    process.env.SARAH_NOSTR_SHADOW_PUBLISH = '1'
    delete process.env.SARAH_NOSTR_RECORD_MODE
    process.env.SARAH_NOSTR_OWNER_PUBKEY = 'ab'.repeat(32)
    process.env.NODE_ENV = 'test'
    delete process.env.SARAH_NOSTR_IDENTITY_SECRET
    try {
      const bridge = tryCreateSarahNostrTurnBridgeFromEnv({
        threadId: 'thread.sarah.' + 'ff'.repeat(12),
      })
      expect(bridge).not.toBeNull()
      expect(bridge!.conversation.ownerPubkey).toBe('ab'.repeat(32))
      expect(bridge!.conversation.conversation).toBe('sarah.' + 'ff'.repeat(12))
    } finally {
      if (prevFlag === undefined) delete process.env.SARAH_NOSTR_SHADOW_PUBLISH
      else process.env.SARAH_NOSTR_SHADOW_PUBLISH = prevFlag
      if (prevMode === undefined) delete process.env.SARAH_NOSTR_RECORD_MODE
      else process.env.SARAH_NOSTR_RECORD_MODE = prevMode
      if (prevOwner === undefined) delete process.env.SARAH_NOSTR_OWNER_PUBKEY
      else process.env.SARAH_NOSTR_OWNER_PUBKEY = prevOwner
      if (prevNodeEnv === undefined) delete process.env.NODE_ENV
      else process.env.NODE_ENV = prevNodeEnv
    }
  })

  it('tryCreate from env builds a bridge when RECORD_MODE=shadow or nostr', () => {
    const prevFlag = process.env.SARAH_NOSTR_SHADOW_PUBLISH
    const prevMode = process.env.SARAH_NOSTR_RECORD_MODE
    const prevOwner = process.env.SARAH_NOSTR_OWNER_PUBKEY
    const prevNodeEnv = process.env.NODE_ENV
    delete process.env.SARAH_NOSTR_SHADOW_PUBLISH
    process.env.SARAH_NOSTR_OWNER_PUBKEY = 'cd'.repeat(32)
    process.env.NODE_ENV = 'test'
    delete process.env.SARAH_NOSTR_IDENTITY_SECRET
    try {
      for (const mode of ['shadow', 'nostr'] as const) {
        process.env.SARAH_NOSTR_RECORD_MODE = mode
        expect(resolveSarahNostrBridgeMode()).toBe(mode)
        const bridge = tryCreateSarahNostrTurnBridgeFromEnv({
          threadId: 'thread.sarah.' + 'aa'.repeat(12),
        })
        expect(bridge).not.toBeNull()
        expect(bridge!.conversation.conversation).toBe('sarah.' + 'aa'.repeat(12))
      }
    } finally {
      if (prevFlag === undefined) delete process.env.SARAH_NOSTR_SHADOW_PUBLISH
      else process.env.SARAH_NOSTR_SHADOW_PUBLISH = prevFlag
      if (prevMode === undefined) delete process.env.SARAH_NOSTR_RECORD_MODE
      else process.env.SARAH_NOSTR_RECORD_MODE = prevMode
      if (prevOwner === undefined) delete process.env.SARAH_NOSTR_OWNER_PUBKEY
      else process.env.SARAH_NOSTR_OWNER_PUBKEY = prevOwner
      if (prevNodeEnv === undefined) delete process.env.NODE_ENV
      else process.env.NODE_ENV = prevNodeEnv
    }
  })
})