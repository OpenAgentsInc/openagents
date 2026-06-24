import { describe, expect, test } from 'vitest'

import {
  buyModeEvalBridgeForEnv,
  makeLiveBuyModeEvalBridge,
} from './buy-mode-live-eval-bridge'
import type { BuyModeRelaySocket } from './buy-mode-live-publisher'

class MemoryRelaySocket implements BuyModeRelaySocket {
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
    queueMicrotask(() => {
      for (const handler of this.messageHandlers) {
        handler({
          data: JSON.stringify([
            'EVENT',
            'buy-mode-eval',
            {
              content: 'verified output',
              id: 'a'.repeat(64),
              kind: 6050,
              pubkey: 'b'.repeat(64),
              tags: [
                ['e', 'request.event'],
                ['amount', '1250', 'lnbc1250n1invoice'],
              ],
            },
          ]),
        })
      }
    })
  }
}

const input = {
  dispatchedJob: {
    amountMsats: 1_250,
    bolt11Ref: null,
    campaignId: 'campaign.test',
    contentDigestRef: null,
    createdAt: '2026-06-24T00:00:00.000Z',
    idempotencyKeyHash: 'hash',
    jobId: 'job.test',
    providerPubkey: null,
    receiptRef: null,
    requestEventId: 'request.event',
    resultEventId: null,
    state: 'issued' as const,
    updatedAt: '2026-06-24T00:00:00.000Z',
  },
  job: {
    amountMsats: 1_250,
    roleIndex: 0,
    sampleId: 'sample',
    workerId: 'b'.repeat(64),
  },
  relayUrl: 'wss://relay.openagents.com',
  requestEventId: 'request.event',
}

describe('buy-mode live eval bridge', () => {
  test('is default-off unless relay mode is configured', () => {
    expect(buyModeEvalBridgeForEnv({})).toBeUndefined()
    expect(buyModeEvalBridgeForEnv({ BUY_MODE_EVAL_BRIDGE: 'disabled' }))
      .toBeUndefined()
    expect(buyModeEvalBridgeForEnv({ BUY_MODE_EVAL_BRIDGE: 'relay' }))
      .toBeDefined()
  })

  test('waits for a matching result event and returns private settlement payload', async () => {
    const socket = new MemoryRelaySocket()
    const bridge = makeLiveBuyModeEvalBridge({
      connect: async relayUrl => {
        expect(relayUrl).toBe('wss://relay.openagents.com')
        return socket
      },
      timeoutMs: 100,
    })
    const result = await bridge.dispatchEval(input)

    expect(result.verdict).toEqual({
      class: 'exact_trace_replay',
      passed: true,
    })
    expect(result.settledMsats).toBe(1_250)
    expect(result.settlement?.resultEventId).toBe('a'.repeat(64))
    expect(socket.sent[0]).toContain('request.event')
    expect(JSON.stringify({
      settledMsats: result.settledMsats,
      verdict: result.verdict,
    })).not.toContain('lnbc')
  })
})
