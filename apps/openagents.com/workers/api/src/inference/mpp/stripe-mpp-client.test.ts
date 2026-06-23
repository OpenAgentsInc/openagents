import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  STRIPE_MPP_API_VERSION,
  createCryptoDepositPaymentIntent,
  encodeStripeForm,
  retrievePaymentIntent,
} from './stripe-mpp-client'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect as Effect.Effect<A, never>)

describe('stripe mpp client — form encoding', () => {
  test('form-encodes nested params the way the Stripe REST API expects', () => {
    const encoded = encodeStripeForm({
      amount: 1,
      currency: 'usd',
      payment_method_options: {
        crypto: { deposit_options: { networks: ['base', 'solana'] }, mode: 'deposit' },
      },
      payment_method_types: ['crypto'],
    })
    expect(encoded).toContain('amount=1')
    expect(encoded).toContain('currency=usd')
    expect(encoded).toContain(
      encodeURIComponent('payment_method_options[crypto][mode]') + '=deposit',
    )
    expect(encoded).toContain(
      encodeURIComponent(
        'payment_method_options[crypto][deposit_options][networks][0]',
      ) + '=base',
    )
    expect(encoded).toContain(
      encodeURIComponent('payment_method_types[0]') + '=crypto',
    )
  })
})

describe('stripe mpp client — create crypto deposit PaymentIntent', () => {
  test('pins the 2026-03-04.preview API version and returns deposit addresses', async () => {
    const seen: Array<{ url: string; init: RequestInit }> = []
    const fakeFetch = async (url: string, init: RequestInit): Promise<Response> => {
      seen.push({ init, url })
      return new Response(
        JSON.stringify({
          amount: 1,
          currency: 'usd',
          id: 'pi_test_1',
          next_action: {
            crypto_display_details: {
              addresses: [{ address: '0xabc', network: 'base' }],
            },
          },
          status: 'requires_payment',
        }),
        { headers: { 'content-type': 'application/json' }, status: 200 },
      )
    }

    const created = await run(
      createCryptoDepositPaymentIntent(
        { fetch: fakeFetch, secretKey: 'sk_test_x' },
        {
          amountCents: 1,
          idempotencyKey: 'mpp:quote:abc',
          networks: ['base', 'solana', 'tempo'],
        },
      ),
    )

    expect(created.id).toBe('pi_test_1')
    expect(created.deposits).toEqual([{ address: '0xabc', network: 'base' }])
    // API version pinned + auth header set + idempotency key forwarded.
    const headers = new Headers(seen[0]?.init.headers)
    expect(headers.get('stripe-version')).toBe(STRIPE_MPP_API_VERSION)
    expect(headers.get('authorization')).toBe('Bearer sk_test_x')
    expect(headers.get('idempotency-key')).toBe('mpp:quote:abc')
    // Body requests crypto deposit mode on the requested networks.
    const body = String(seen[0]?.init.body)
    expect(body).toContain(encodeURIComponent('payment_method_types[0]') + '=crypto')
  })

  test('fails on a Stripe error response', async () => {
    const fakeFetch = async (): Promise<Response> =>
      new Response(JSON.stringify({ error: { message: 'bad request' } }), {
        status: 400,
      })
    const result = await Effect.runPromise(
      createCryptoDepositPaymentIntent(
        { fetch: fakeFetch, secretKey: 'sk_test_x' },
        { amountCents: 1, idempotencyKey: 'k', networks: ['base'] },
      ).pipe(
        Effect.map(() => 'ok' as const),
        Effect.catch(error => Effect.succeed(error.detail)),
      ),
    )
    expect(result).toContain('bad request')
  })
})

describe('stripe mpp client — retrieve / verify settlement', () => {
  test('marks a succeeded PaymentIntent settled', async () => {
    const fakeFetch = async (): Promise<Response> =>
      new Response(
        JSON.stringify({
          amount: 5,
          currency: 'usd',
          id: 'pi_1',
          metadata: { model: 'openagents/khala-mini' },
          status: 'succeeded',
        }),
        { status: 200 },
      )
    const verified = await run(
      retrievePaymentIntent({ fetch: fakeFetch, secretKey: 'sk_test_x' }, 'pi_1'),
    )
    expect(verified.settled).toBe(true)
    expect(verified.amountCents).toBe(5)
    expect(verified.metadata.model).toBe('openagents/khala-mini')
  })

  test('does NOT mark an unsettled PaymentIntent settled', async () => {
    const fakeFetch = async (): Promise<Response> =>
      new Response(
        JSON.stringify({ amount: 5, id: 'pi_2', status: 'requires_payment_method' }),
        { status: 200 },
      )
    const verified = await run(
      retrievePaymentIntent({ fetch: fakeFetch, secretKey: 'sk_test_x' }, 'pi_2'),
    )
    expect(verified.settled).toBe(false)
    expect(verified.status).toBe('requires_payment_method')
  })
})
