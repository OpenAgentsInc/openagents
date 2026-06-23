import { Effect, Fiber } from 'effect'
import { TestClock } from 'effect/testing'
import { describe, expect, test } from 'vitest'

import {
  type MintLightningInvoice,
  LightningInvoiceError,
  makeFallbackLightningInvoiceIssuer,
} from './mpp-lightning-invoice'
import {
  type SparkTreasuryFundingInvoicePost,
  SPARK_LIGHTNING_MINT_TIMEOUT_MS,
  makeSparkLightningInvoiceIssuer,
} from './mpp-lightning-invoice-spark'

const run = <A, E>(effect: Effect.Effect<A, E>): Promise<A> =>
  Effect.runPromise(effect as Effect.Effect<A, never>)

const reasonOf = <A>(
  effect: Effect.Effect<A, LightningInvoiceError>,
): Promise<string> =>
  Effect.runPromise(
    effect.pipe(
      Effect.map(() => 'ok' as const),
      Effect.catch((e: LightningInvoiceError) => Effect.succeed(e.reason)),
    ),
  )

const HASH =
  '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff'
const HASH2 =
  'aabbccddeeff00112233445566778899aabbccddeeff001122334455667788ff'
const INVOICE = `lnbc100n1p${'a'.repeat(40)}`
const MDK_INVOICE = `lnbc200n1p${'b'.repeat(40)}`

describe('makeSparkLightningInvoiceIssuer', () => {
  test('posts a Spark funding-invoice and reads bolt11 + decoded paymentHash', async () => {
    let seen: Record<string, unknown> | undefined
    const post: SparkTreasuryFundingInvoicePost = async body => {
      seen = body
      return {
        ok: true,
        payload: {
          amountSat: 7,
          bolt11Invoice: INVOICE,
          paymentHash: HASH,
          rail: 'spark',
        },
        status: 200,
      }
    }
    const issuer = makeSparkLightningInvoiceIssuer(post)
    const invoice = await run(
      issuer({ amountSats: 7, correlationRef: 'ref', description: 'desc' }),
    )
    expect(invoice.bolt11).toBe(INVOICE)
    expect(invoice.paymentHash).toBe(HASH)
    expect(invoice.network).toBe('mainnet')
    // The funding-invoice body carries amountSat + description (no secrets).
    expect(seen?.amountSat).toBe(7)
    expect(seen?.description).toBe('desc')
  })

  test('maps a 5xx route status to provider_unavailable', async () => {
    const post: SparkTreasuryFundingInvoicePost = async () => ({
      ok: false,
      payload: { error: 'spark_treasury_funding_invoice_failed' },
      status: 502,
    })
    const reason = await reasonOf(
      makeSparkLightningInvoiceIssuer(post)({
        amountSats: 1,
        correlationRef: 'r',
        description: 'd',
      }),
    )
    expect(reason).toBe('provider_unavailable')
  })

  test('maps a 4xx route status to provider_rejected', async () => {
    const post: SparkTreasuryFundingInvoicePost = async () => ({
      ok: false,
      payload: { error: 'amount_sat_must_be_positive_integer' },
      status: 400,
    })
    const reason = await reasonOf(
      makeSparkLightningInvoiceIssuer(post)({
        amountSats: 1,
        correlationRef: 'r',
        description: 'd',
      }),
    )
    expect(reason).toBe('provider_rejected')
  })

  test('a payload missing the paymentHash => malformed_invoice', async () => {
    const post: SparkTreasuryFundingInvoicePost = async () => ({
      ok: true,
      payload: { bolt11Invoice: INVOICE, rail: 'spark' },
      status: 200,
    })
    const reason = await reasonOf(
      makeSparkLightningInvoiceIssuer(post)({
        amountSats: 1,
        correlationRef: 'r',
        description: 'd',
      }),
    )
    expect(reason).toBe('malformed_invoice')
  })

  // ROOT-CAUSE REGRESSION: the Spark treasury is a Cloudflare Container and a
  // cold container / cold breez-sdk-spark build can block for SECONDS.
  // `Effect.tryPromise` catches throws, NOT a hang. The bounded mint must
  // interrupt a hung post and fail typed (`provider_unavailable`).
  test('a HUNG post is bounded by the mint timeout => provider_unavailable (no hang)', async () => {
    let postStarted = false
    const post: SparkTreasuryFundingInvoicePost = () => {
      postStarted = true
      return new Promise(() => {})
    }
    const issuer = makeSparkLightningInvoiceIssuer(post)
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const fiber = yield* Effect.forkChild(
          issuer({ amountSats: 1, correlationRef: 'r', description: 'd' }).pipe(
            Effect.map(() => 'ok' as const),
            Effect.catch((e: LightningInvoiceError) =>
              Effect.succeed(e.reason),
            ),
          ),
        )
        yield* TestClock.adjust(SPARK_LIGHTNING_MINT_TIMEOUT_MS + 1)
        return yield* Fiber.join(fiber)
      }).pipe(Effect.provide(TestClock.layer())),
    )
    expect(postStarted).toBe(true)
    expect(result).toBe('provider_unavailable')
  })
})

describe('makeFallbackLightningInvoiceIssuer (Spark primary, MDK fallback)', () => {
  const sparkIssuer = (invoice: string, hash: string): MintLightningInvoice =>
    makeSparkLightningInvoiceIssuer(async () => ({
      ok: true,
      payload: { bolt11Invoice: invoice, paymentHash: hash, rail: 'spark' },
      status: 200,
    }))

  const failingIssuer = (
    reason: LightningInvoiceError['reason'],
  ): MintLightningInvoice => makeSparkLightningInvoiceIssuer(async () => ({
    ok: false,
    payload: {},
    status: reason === 'provider_rejected' ? 400 : 502,
  }))

  const mdkLike = (invoice: string, hash: string): MintLightningInvoice =>
    makeSparkLightningInvoiceIssuer(async () => ({
      ok: true,
      payload: { bolt11Invoice: invoice, paymentHash: hash, rail: 'spark' },
      status: 200,
    }))

  test('Spark available => Spark invoice is used (MDK fallback never called)', async () => {
    let mdkCalled = false
    const mdk: MintLightningInvoice = input => {
      mdkCalled = true
      return mdkLike(MDK_INVOICE, HASH2)(input)
    }
    const issuer = makeFallbackLightningInvoiceIssuer(
      sparkIssuer(INVOICE, HASH),
      mdk,
    )
    const invoice = await run(
      issuer!({ amountSats: 5, correlationRef: 'r', description: 'd' }),
    )
    expect(invoice.bolt11).toBe(INVOICE)
    expect(invoice.paymentHash).toBe(HASH)
    expect(mdkCalled).toBe(false)
  })

  test('Spark unavailable => MDK fallback is used', async () => {
    const issuer = makeFallbackLightningInvoiceIssuer(
      failingIssuer('provider_unavailable'),
      mdkLike(MDK_INVOICE, HASH2),
    )
    const invoice = await run(
      issuer!({ amountSats: 5, correlationRef: 'r', description: 'd' }),
    )
    expect(invoice.bolt11).toBe(MDK_INVOICE)
    expect(invoice.paymentHash).toBe(HASH2)
  })

  test('both unavailable => fails typed (route drops only the Lightning rail)', async () => {
    const issuer = makeFallbackLightningInvoiceIssuer(
      failingIssuer('provider_unavailable'),
      failingIssuer('provider_unavailable'),
    )
    const reason = await reasonOf(
      issuer!({ amountSats: 5, correlationRef: 'r', description: 'd' }),
    )
    expect(reason).toBe('provider_unavailable')
  })

  test('only one issuer present => returned as-is', () => {
    const only = sparkIssuer(INVOICE, HASH)
    expect(makeFallbackLightningInvoiceIssuer(only, undefined)).toBe(only)
    expect(makeFallbackLightningInvoiceIssuer(undefined, only)).toBe(only)
    expect(
      makeFallbackLightningInvoiceIssuer(undefined, undefined),
    ).toBeUndefined()
  })
})
