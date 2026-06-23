// The 402-gated, machine-payable Khala endpoint (EPIC #6049, Phase 2 + 3).
//
// Flagged, default OFF. Wraps the EXISTING Khala completion path
// (`handleChatCompletions`) behind an MPP/x402 paywall:
//
//   1. No payment credential  -> 402 Payment Required + WWW-Authenticate
//      challenge(s) (USDC crypto, and card/SPT when a profile id is configured).
//   2. Valid credential        -> verify settlement via the Stripe REST API
//      (Worker-native, API version 2026-03-04.preview), mint Khala credits into
//      a payer-bound balance (Phase 3, reuses the USD-origin credit-grant seam),
//      then run the SAME Khala completion against that credit and return it with
//      the `openagents` receipt.
//
// FAIL-SAFE (the central safety property): the endpoint is INERT unless it is
// fully configured. With no Stripe key, OR no flag (KHALA_MPP_ENABLED), the
// endpoint returns a clean "not configured" 503 and NEVER constructs a charge.
// A missing `profile_` id only disables the CARD rail; the crypto rail still
// works. So a half-configured deploy can charge nothing it should not.

import { Effect } from 'effect'

import { noStoreJsonResponse } from '../../http/responses'
import {
  type ChatCompletionsDeps,
  handleChatCompletions,
  resolveRequestedModel,
} from '../chat-completions-routes'
import { isKhalaModel } from '../pricing'
import {
  type MppCreditGrantOutcome,
  mintMppCredits,
  mppPayerAccountRef,
} from './mpp-credit-grant'
import {
  type MppChallenge,
  buildPaymentRequiredHeaders,
  buildPaymentRequiredProblem,
  parsePaymentCredential,
} from './mpp-protocol'
import { type MppRail, quoteMppCall } from './mpp-pricing'
import {
  type StripeFetch,
  createCryptoDepositPaymentIntent,
  retrievePaymentIntent,
} from './stripe-mpp-client'

// Default model + crypto networks the endpoint quotes for. khala-mini is the
// general pay-per-call tier.
const DEFAULT_MPP_MODEL = 'openagents/khala-mini'
// x402 (Base) + MPP (Solana, Tempo) — all USDC. We advertise all three crypto
// networks; the agent picks one.
const CRYPTO_NETWORKS: ReadonlyArray<string> = ['base', 'solana', 'tempo']

// Parse the KHALA_MPP_ENABLED flag. Default OFF.
export const isKhalaMppEnabled = (value: string | undefined): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type MppChatCompletionsDeps = Readonly<{
  // The KHALA_MPP_ENABLED flag, parsed. Default OFF => inert.
  enabled: boolean
  // The Stripe secret key (live or test). Absent => inert (never charges).
  stripeSecretKey: string | undefined
  // The network profile id (`profile_…`) for SPT/card. Absent => crypto-only.
  stripeNetworkProfileId?: string | undefined
  // The deps for the underlying Khala completion path (registry, metering hook,
  // etc). The MPP endpoint reuses these so a paid call runs the SAME completion,
  // metering, receipt, and Bitcoin-payout loop as the keyed `/v1/chat/completions`
  // route. NOTE: `enabled` here must be the inference-gateway flag (so the
  // completion runs); `authenticate`/`readAvailableMsat` are REPLACED by the MPP
  // endpoint with the payer-bound account + minted credit.
  completionDeps: Omit<
    ChatCompletionsDeps,
    'authenticate' | 'readAvailableMsat'
  > &
    Readonly<{ enabled: boolean }>
  // Phase 3 credit mint.
  db: D1Database
  // Injectable seams for tests.
  fetch?: StripeFetch
  nowIso?: () => string
  usdCentsToMsat?: (amountCents: number) => number
  newId?: () => string
}>

// The inert "not configured" response. 503: the endpoint exists but is not
// turned on. Carries NO challenge, so no client ever tries to pay.
const notConfigured = (reason: string): Response =>
  noStoreJsonResponse(
    { error: 'mpp_not_configured', reason },
    { status: 503 },
  )

// Build the 402 challenge set for a quote. Always includes the crypto rail; adds
// the card/SPT rail only when a network profile id is configured.
const buildChallenges = (
  input: Readonly<{
    model: string
    cryptoPaymentIntentId: string
    cryptoAmountCents: number
    cryptoDeposit: { network: string; address: string } | undefined
    cardEnabled: boolean
    cardAmountCents: number
    challengeIdPrefix: string
  }>,
): ReadonlyArray<MppChallenge> => {
  const challenges: Array<MppChallenge> = [
    {
      amountCents: input.cryptoAmountCents,
      currency: 'usdc',
      id: `${input.challengeIdPrefix}:crypto`,
      intent: 'charge',
      method: input.cryptoDeposit?.network ?? 'base',
      network: input.cryptoDeposit?.network ?? 'base',
      paymentIntentId: input.cryptoPaymentIntentId,
      recipient: input.cryptoDeposit?.address,
    },
  ]
  if (input.cardEnabled) {
    challenges.push({
      amountCents: input.cardAmountCents,
      currency: 'usd',
      id: `${input.challengeIdPrefix}:card`,
      intent: 'charge',
      method: 'stripe',
    })
  }
  return challenges
}

// Resolve the requested model from the body, defaulting to khala-mini, and
// constrained to Khala models (the MPP endpoint sells Khala). A non-Khala model
// is coerced to the default so the paywall always quotes a Khala price.
const resolveMppModel = (rawBody: Record<string, unknown> | undefined): string => {
  const requested = resolveRequestedModel(
    typeof rawBody?.model === 'string' ? rawBody.model : undefined,
  )
  return isKhalaModel(requested) ? requested : DEFAULT_MPP_MODEL
}

// Run the underlying Khala completion as the payer-bound account (credit already
// minted). Replaces auth + balance reader so the completion runs against the
// minted credit; everything else (registry, metering hook, receipt, lane plan)
// is the SAME as the keyed route.
const runPaidCompletion = (
  request: Request,
  deps: MppChatCompletionsDeps,
  grant: MppCreditGrantOutcome,
): Effect.Effect<Response> =>
  handleChatCompletions(request, {
    ...deps.completionDeps,
    authenticate: async () => ({ accountRef: grant.accountRef }),
    enabled: deps.completionDeps.enabled,
    // The minted credit is the available balance; the metering hook decrements
    // it receipt-first on the real completion.
    readAvailableMsat: async () => grant.grantedMsat,
  })

export const handleMppChatCompletions = (
  request: Request,
  deps: MppChatCompletionsDeps,
): Effect.Effect<Response> =>
  Effect.gen(function* () {
    // FAIL-SAFE GATE 1: flag off => inert. Never charges.
    if (!deps.enabled) {
      return notConfigured('KHALA_MPP_ENABLED is off')
    }
    // FAIL-SAFE GATE 2: no Stripe key => inert. Never charges.
    if (
      deps.stripeSecretKey === undefined ||
      deps.stripeSecretKey.trim() === ''
    ) {
      return notConfigured('stripe key not configured')
    }

    if (request.method !== 'POST') {
      return noStoreJsonResponse(
        { error: 'method_not_allowed' },
        { status: 405 },
      )
    }

    const stripeDeps = {
      secretKey: deps.stripeSecretKey,
      ...(deps.fetch === undefined ? {} : { fetch: deps.fetch }),
      ...(deps.stripeNetworkProfileId === undefined
        ? {}
        : { networkProfileId: deps.stripeNetworkProfileId }),
    }
    const newId = deps.newId ?? (() => crypto.randomUUID())
    const cardEnabled =
      deps.stripeNetworkProfileId !== undefined &&
      deps.stripeNetworkProfileId.trim() !== ''

    // We need the body for BOTH the 402 quote (model) and the paid retry (the
    // completion). Read it once; reconstruct a fresh Request for the underlying
    // handler so its own `request.json()` works.
    const rawBody = yield* Effect.promise(async () => {
      try {
        return (await request.clone().json()) as Record<string, unknown>
      } catch {
        return undefined
      }
    })

    const credential = parsePaymentCredential(request.headers.get('authorization'))

    // ---- NO CREDENTIAL: return 402 + challenge(s) ----
    if (credential === undefined) {
      const model = resolveMppModel(rawBody)
      const cryptoQuote = quoteMppCall({ model, rail: 'crypto' as MppRail })
      const cardQuote = quoteMppCall({ model, rail: 'card' as MppRail })

      // Create a crypto deposit PaymentIntent to get a deposit address for the
      // challenge. If Stripe fails here, the endpoint is effectively unable to
      // quote — return a clean 503 (never a broken 402).
      const created = yield* createCryptoDepositPaymentIntent(stripeDeps, {
        amountCents: cryptoQuote.amountCents,
        idempotencyKey: `mpp:quote:${newId()}`,
        metadata: { model, product: 'openagents_khala_mpp' },
        networks: CRYPTO_NETWORKS,
      }).pipe(
        Effect.map(intent => ({ ok: true as const, intent })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.detail }),
        ),
      )
      if (!created.ok) {
        return notConfigured(`stripe quote failed: ${created.reason}`)
      }

      const challenges = buildChallenges({
        cardAmountCents: cardQuote.amountCents,
        cardEnabled,
        challengeIdPrefix: created.intent.id,
        cryptoAmountCents: cryptoQuote.amountCents,
        cryptoDeposit: created.intent.deposits[0],
        cryptoPaymentIntentId: created.intent.id,
        model,
      })

      return new Response(
        JSON.stringify(buildPaymentRequiredProblem(challenges)),
        {
          headers: buildPaymentRequiredHeaders(challenges),
          status: 402,
        },
      )
    }

    // ---- CREDENTIAL PRESENT: verify settlement, mint credits, serve ----
    const paymentIntentId = credential.paymentIntentId
    if (paymentIntentId === undefined || paymentIntentId.trim() === '') {
      // A credential we cannot verify Worker-native (e.g. an SPT/card token that
      // needs the Node MPP SDK to settle). We do NOT guess; return a fresh 402
      // so the client retries with a verifiable crypto credential, and flag the
      // sidecar requirement in the body.
      const model = resolveMppModel(rawBody)
      const cryptoQuote = quoteMppCall({ model, rail: 'crypto' as MppRail })
      const created = yield* createCryptoDepositPaymentIntent(stripeDeps, {
        amountCents: cryptoQuote.amountCents,
        idempotencyKey: `mpp:quote:${newId()}`,
        metadata: { model, product: 'openagents_khala_mpp' },
        networks: CRYPTO_NETWORKS,
      }).pipe(
        Effect.map(intent => ({ ok: true as const, intent })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.detail }),
        ),
      )
      if (!created.ok) {
        return notConfigured(`stripe quote failed: ${created.reason}`)
      }
      const challenges = buildChallenges({
        cardAmountCents: cryptoQuote.amountCents,
        cardEnabled,
        challengeIdPrefix: created.intent.id,
        cryptoAmountCents: cryptoQuote.amountCents,
        cryptoDeposit: created.intent.deposits[0],
        cryptoPaymentIntentId: created.intent.id,
        model,
      })
      const problem = {
        ...buildPaymentRequiredProblem(challenges),
        detail:
          'Could not verify the supplied payment credential Worker-native. Retry with a crypto payment credential (Base/Solana/Tempo USDC). Card/SPT settlement may require the Node MPP sidecar.',
      }
      return new Response(JSON.stringify(problem), {
        headers: buildPaymentRequiredHeaders(challenges),
        status: 402,
      })
    }

    const verified = yield* retrievePaymentIntent(
      stripeDeps,
      paymentIntentId,
    ).pipe(
      Effect.map(intent => ({ ok: true as const, intent })),
      Effect.catch(error =>
        Effect.succeed({ ok: false as const, reason: error.detail }),
      ),
    )
    if (!verified.ok) {
      return noStoreJsonResponse(
        { error: 'payment_verification_failed', reason: verified.reason },
        { status: 502 },
      )
    }
    if (!verified.intent.settled) {
      // Paid not yet settled — return 402 again so the client waits/retries. No
      // completion runs; nothing is served unpaid.
      return noStoreJsonResponse(
        {
          error: 'payment_not_settled',
          payment_intent: verified.intent.id,
          status: verified.intent.status,
        },
        { status: 402 },
      )
    }

    // SETTLED. Phase 3: mint Khala credits for the settled amount into a
    // payer-bound balance (idempotent per payment id), then run the SAME Khala
    // completion against that credit.
    const accountRef = mppPayerAccountRef(verified.intent.id)
    const grant = yield* mintMppCredits(
      {
        db: deps.db,
        ...(deps.nowIso === undefined ? {} : { nowIso: deps.nowIso }),
        ...(deps.usdCentsToMsat === undefined
          ? {}
          : { usdCentsToMsat: deps.usdCentsToMsat }),
      },
      {
        accountRef,
        amountCents: verified.intent.amountCents,
        paymentIntentId: verified.intent.id,
      },
    ).pipe(
      Effect.map(outcome => ({ ok: true as const, outcome })),
      Effect.catch(() =>
        Effect.succeed({ ok: false as const } as const),
      ),
    )
    if (!grant.ok) {
      return noStoreJsonResponse(
        { error: 'credit_grant_failed', payment_intent: verified.intent.id },
        { status: 500 },
      )
    }

    return yield* runPaidCompletion(request, deps, grant.outcome)
  })
