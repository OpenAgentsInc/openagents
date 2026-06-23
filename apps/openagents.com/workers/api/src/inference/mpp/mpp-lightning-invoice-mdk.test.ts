import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { LightningInvoiceError } from './mpp-lightning-invoice'
import {
  type MdkRoutePost,
  makeMdkLightningInvoiceIssuer,
} from './mpp-lightning-invoice-mdk'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect as Effect.Effect<A, never>)

const HASH =
  '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff'
const INVOICE = `lnbc100n1p${'a'.repeat(40)}`

describe('makeMdkLightningInvoiceIssuer', () => {
  test('posts a SAT create_checkout and reads the raw bolt11 + paymentHash', async () => {
    let seen: Record<string, unknown> | undefined
    const post: MdkRoutePost = async body => {
      seen = body
      return {
        ok: true,
        payload: {
          data: {
            checkout: {
              id: 'mdk_checkout_x',
              invoice: {
                invoice: INVOICE,
                paymentHash: HASH,
                expiresAt: '2099-01-15T12:05:00Z',
              },
            },
          },
        },
        status: 200,
      }
    }
    const issuer = makeMdkLightningInvoiceIssuer(post)
    const invoice = await run(
      issuer({ amountSats: 42, correlationRef: 'ref', description: 'd' }),
    )
    expect(invoice.bolt11).toBe(INVOICE)
    expect(invoice.paymentHash).toBe(HASH)
    expect(invoice.network).toBe('mainnet')
    expect(invoice.invoiceExpiresAt).toBe('2099-01-15T12:05:00.000Z')
    // The create_checkout body is amount-mode SAT.
    const params = (seen as { params: Record<string, unknown> }).params
    expect(seen?.handler).toBe('create_checkout')
    expect(params.currency).toBe('SAT')
    expect(params.amount).toBe(42)
    expect(params.type).toBe('AMOUNT')
  })

  test('maps a 5xx route status to provider_unavailable', async () => {
    const post: MdkRoutePost = async () => ({
      ok: false,
      payload: {},
      status: 503,
    })
    const issuer = makeMdkLightningInvoiceIssuer(post)
    const result = await Effect.runPromise(
      issuer({ amountSats: 1, correlationRef: 'r', description: 'd' }).pipe(
        Effect.map(() => 'ok' as const),
        Effect.catch((e: LightningInvoiceError) => Effect.succeed(e.reason)),
      ),
    )
    expect(result).toBe('provider_unavailable')
  })

  test('maps a 4xx route status to provider_rejected', async () => {
    const post: MdkRoutePost = async () => ({
      ok: false,
      payload: {},
      status: 400,
    })
    const issuer = makeMdkLightningInvoiceIssuer(post)
    const result = await Effect.runPromise(
      issuer({ amountSats: 1, correlationRef: 'r', description: 'd' }).pipe(
        Effect.map(() => 'ok' as const),
        Effect.catch((e: LightningInvoiceError) => Effect.succeed(e.reason)),
      ),
    )
    expect(result).toBe('provider_rejected')
  })

  test('a payload with no usable invoice => malformed_invoice', async () => {
    const post: MdkRoutePost = async () => ({
      ok: true,
      payload: { data: { checkout: { id: 'x' } } },
      status: 200,
    })
    const issuer = makeMdkLightningInvoiceIssuer(post)
    const result = await Effect.runPromise(
      issuer({ amountSats: 1, correlationRef: 'r', description: 'd' }).pipe(
        Effect.map(() => 'ok' as const),
        Effect.catch((e: LightningInvoiceError) => Effect.succeed(e.reason)),
      ),
    )
    expect(result).toBe('malformed_invoice')
  })
})
