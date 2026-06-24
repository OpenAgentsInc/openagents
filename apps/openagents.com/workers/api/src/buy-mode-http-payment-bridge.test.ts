import { describe, expect, test } from 'vitest'

import {
  buyModePaymentBridgeForEnv,
  makeHttpBuyModePaymentBridge,
} from './buy-mode-http-payment-bridge'

describe('buy-mode HTTP payment bridge', () => {
  test('is default-off until URL and token are both configured', () => {
    expect(buyModePaymentBridgeForEnv({})).toBeUndefined()
    expect(buyModePaymentBridgeForEnv({
      BUY_MODE_PAYMENT_BRIDGE_URL: 'https://bridge.example/pay',
    })).toBeUndefined()
    expect(buyModePaymentBridgeForEnv({
      BUY_MODE_PAYMENT_BRIDGE_TOKEN: 'token',
    })).toBeUndefined()
  })

  test('posts invoice payment requests and accepts public-safe receipt refs', async () => {
    const requests: Array<{ body: unknown; headers: Headers }> = []
    const bridge = makeHttpBuyModePaymentBridge({
      endpoint: 'https://bridge.example/pay',
      fetch: async (_request, init) => {
        requests.push({
          body: JSON.parse(String(init?.body)),
          headers: new Headers(init?.headers),
        })
        return new Response(JSON.stringify({
          receipt_ref: 'receipt.public.buy_mode.eval',
          settlement_ref: 'settlement.public.buy_mode.eval',
        }))
      },
      token: 'secret-token',
    })
    const receipt = await bridge.payBolt11({
      amountMsats: 1_250,
      bolt11: 'lnbc1250n1invoice',
      idempotencyRef: 'buy_mode.settle.result',
      providerPubkey: 'b'.repeat(64),
      resultEventId: 'a'.repeat(64),
    })

    expect(receipt).toEqual({
      receiptRef: 'receipt.public.buy_mode.eval',
      settlementRef: 'settlement.public.buy_mode.eval',
    })
    expect(requests).toHaveLength(1)
    expect(requests[0]?.headers.get('authorization')).toBe('Bearer secret-token')
    expect(requests[0]?.body).toMatchObject({
      amount_msats: 1_250,
      idempotency_ref: 'buy_mode.settle.result',
    })
  })

  test('rejects unsafe receipt refs from the external bridge', async () => {
    const bridge = makeHttpBuyModePaymentBridge({
      endpoint: 'https://bridge.example/pay',
      fetch: async () =>
        new Response(JSON.stringify({
          receipt_ref: 'receipt.public.lnbc1250n1invoice',
          settlement_ref: 'settlement.public.buy_mode.eval',
        })),
      token: 'secret-token',
    })

    await expect(bridge.payBolt11({
      amountMsats: 1_250,
      bolt11: 'lnbc1250n1invoice',
      idempotencyRef: 'buy_mode.settle.result',
      providerPubkey: 'b'.repeat(64),
      resultEventId: 'a'.repeat(64),
    })).rejects.toThrow('unsafe receipt refs')
  })
})
