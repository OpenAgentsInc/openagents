// The 402-gated, machine-payable Khala endpoint (EPIC #6049, Phase 2/3 + defect
// B Worker-native MPP verification + settlement).
//
// Flagged, default OFF. Wraps the EXISTING Khala completion path
// (`handleChatCompletions`) behind an MPP / Payment-Auth (HTTP 402) paywall,
// implemented per the canonical spec mirrored at
// docs/reference/mpp/paymentauth/specs/:
//
//   1. No payment credential  -> 402 Payment Required with one
//      `WWW-Authenticate: Payment ...` per supported method. Each challenge `id`
//      is HMAC-SHA256 over the canonical challenge fields with a server-held
//      secret (draft-httpauth-payment-00 §5.1.3), and our crypto deposit
//      PaymentIntent id rides in the `opaque` field so we recover it statelessly.
//   2. `Authorization: Payment <base64url>` -> decode {challenge,payload}, verify
//      the HMAC binding + expiry + request-amount/currency FAIL-CLOSED, then
//      settle:
//        - crypto: recover the deposit PaymentIntent id from `opaque`, retrieve
//          it from Stripe, serve only when `succeeded`;
//        - card/SPT: take `payload.spt`, enforce single-use (replay cache),
//          create+confirm a PaymentIntent with the SPT, serve only when
//          `succeeded`.
//      On success: mint USD-origin Khala credits (idempotent `mpp:<pi>`), run the
//      SAME Khala completion + metering + receipt, and return it with a
//      standards-shaped `Payment-Receipt` header.
//
// FAIL-SAFE (the central safety property): the endpoint is INERT unless it is
// fully configured. With no flag (KHALA_MPP_ENABLED), no Stripe key, OR no
// signing secret (KHALA_MPP_SIGNING_SECRET) it returns a clean "not configured"
// 503 and NEVER constructs a charge or issues a challenge. A missing `profile_`
// id only disables the CARD rail; the crypto rail still works.

import { Effect } from 'effect'

import { noStoreJsonResponse } from '../../http/responses'
import {
  currentEpochMillis,
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
  randomUuid,
} from '../../runtime-primitives'
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
  type MppOpaque,
  type ParsedPaymentCredential,
  buildChallenge,
  buildPaymentReceipt,
  buildPaymentRequiredHeaders,
  buildPaymentRequiredProblem,
  parsePaymentCredential,
  verifyCredential,
} from './mpp-protocol'
import { type MppRail, quoteMppCall } from './mpp-pricing'
import { claimSpt, recordSptPaymentIntent } from './mpp-spt-replay'
import {
  type StripeFetch,
  createCryptoDepositPaymentIntent,
  createSptChargePaymentIntent,
  retrievePaymentIntent,
} from './stripe-mpp-client'

// Default model + crypto networks the endpoint quotes for. khala-mini is the
// general pay-per-call tier.
const DEFAULT_MPP_MODEL = 'openagents/khala-mini'
// x402 (Base) + MPP (Solana, Tempo) — all USDC. We advertise all three crypto
// networks; the agent picks one.
const CRYPTO_NETWORKS: ReadonlyArray<string> = ['base', 'solana', 'tempo']
// The protection-space realm for our challenges (core spec §5.1.1).
const MPP_REALM = 'openagents.com'
// Challenge validity window.
const CHALLENGE_TTL_MS = 5 * 60 * 1000

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
  // The challenge-binding signing secret (HMAC key). Absent => inert (never
  // issues a challenge or verifies a credential).
  signingSecret: string | undefined
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
  nowMs?: () => number
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

// Resolve the requested model from the body, defaulting to khala-mini, and
// constrained to Khala models (the MPP endpoint sells Khala). A non-Khala model
// is coerced to the default so the paywall always quotes a Khala price.
const resolveMppModel = (rawBody: Record<string, unknown> | undefined): string => {
  const requested = resolveRequestedModel(
    typeof rawBody?.model === 'string' ? rawBody.model : undefined,
  )
  return isKhalaModel(requested) ? requested : DEFAULT_MPP_MODEL
}

// Build the spec-shaped challenge set for a quote. Always includes the crypto
// rail (one challenge per supported network, all bound to the SAME crypto
// deposit PaymentIntent recovered from `opaque`); adds the card/SPT rail only
// when a network profile id is configured. Each challenge's `id` is the HMAC
// binding computed inside `buildChallenge`.
const buildChallenges = (
  signingSecret: string,
  input: Readonly<{
    model: string
    expires: string
    cryptoPaymentIntentId: string
    cryptoAmountCents: number
    cryptoDeposit: { network: string; address: string } | undefined
    cardEnabled: boolean
    cardAmountCents: number
    cardNetworkProfileId: string | undefined
  }>,
) =>
  Effect.gen(function* () {
    const challenges: Array<MppChallenge> = []
    // The opaque correlation data carries the crypto deposit PaymentIntent id
    // so the retry recovers it statelessly. One opaque per crypto rail (all
    // networks share the same deposit-mode PaymentIntent).
    const cryptoNetwork = input.cryptoDeposit?.network ?? CRYPTO_NETWORKS[0]!
    const cryptoOpaque: MppOpaque = {
      amount: String(input.cryptoAmountCents),
      model: input.model,
      network: cryptoNetwork,
      pi: input.cryptoPaymentIntentId,
    }
    // Crypto challenge (single network = the deposit-mode network Stripe
    // returned; the deposit-mode PaymentIntent backs the settlement).
    const crypto = yield* Effect.promise(() =>
      buildChallenge(signingSecret, {
        amountCents: input.cryptoAmountCents,
        currency: 'usdc',
        expires: input.expires,
        method: cryptoNetwork,
        network: cryptoNetwork,
        opaque: cryptoOpaque,
        paymentIntentId: input.cryptoPaymentIntentId,
        realm: MPP_REALM,
        recipient: input.cryptoDeposit?.address,
        request: {
          amount: String(input.cryptoAmountCents),
          currency: 'usdc',
          description: `OpenAgents Khala ${input.model} pay-per-call`,
          network: cryptoNetwork,
          recipient: input.cryptoDeposit?.address,
        },
      }),
    )
    challenges.push(crypto)

    if (input.cardEnabled && input.cardNetworkProfileId !== undefined) {
      const card = yield* Effect.promise(() =>
        buildChallenge(signingSecret, {
          amountCents: input.cardAmountCents,
          currency: 'usd',
          expires: input.expires,
          method: 'stripe',
          realm: MPP_REALM,
          request: {
            amount: String(input.cardAmountCents),
            currency: 'usd',
            description: `OpenAgents Khala ${input.model} pay-per-call`,
            methodDetails: {
              networkId: input.cardNetworkProfileId,
              paymentMethodTypes: ['card', 'link'],
            },
          },
        }),
      )
      challenges.push(card)
    }
    return challenges as ReadonlyArray<MppChallenge>
  })

// Issue a fresh 402 (no credential, or a credential we refused). Creates a
// crypto deposit PaymentIntent for the quote (the address the crypto challenge
// points at) and builds the bound challenge set. If Stripe cannot quote, returns
// a clean 503 (never a broken 402).
const issueChallenge = (
  deps: MppChatCompletionsDeps,
  signingSecret: string,
  stripeDeps: Parameters<typeof createCryptoDepositPaymentIntent>[0],
  cardEnabled: boolean,
  rawBody: Record<string, unknown> | undefined,
  detail?: string,
) =>
  Effect.gen(function* () {
    const newId = deps.newId ?? randomUuid
    const nowMs = (deps.nowMs ?? currentEpochMillis)()
    const model = resolveMppModel(rawBody)
    const cryptoQuote = quoteMppCall({ model, rail: 'crypto' as MppRail })
    const cardQuote = quoteMppCall({ model, rail: 'card' as MppRail })

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

    const challenges = yield* buildChallenges(signingSecret, {
      cardAmountCents: cardQuote.amountCents,
      cardEnabled,
      cardNetworkProfileId: deps.stripeNetworkProfileId,
      cryptoAmountCents: cryptoQuote.amountCents,
      cryptoDeposit: created.intent.deposits[0],
      cryptoPaymentIntentId: created.intent.id,
      expires: epochMillisToIsoTimestamp(nowMs + CHALLENGE_TTL_MS),
      model,
    })

    return new Response(
      JSON.stringify(buildPaymentRequiredProblem(challenges, detail)),
      {
        headers: buildPaymentRequiredHeaders(challenges),
        status: 402,
      },
    )
  })

// Run the underlying Khala completion as the payer-bound account (credit already
// minted), then attach the standards-shaped `Payment-Receipt` header on success.
const runPaidCompletion = (
  request: Request,
  deps: MppChatCompletionsDeps,
  grant: MppCreditGrantOutcome,
  receipt: Readonly<{ method: string; reference: string }>,
) =>
  Effect.gen(function* () {
    const nowIso = (deps.nowIso ?? currentIsoTimestamp)()
    const response = yield* handleChatCompletions(request, {
      ...deps.completionDeps,
      authenticate: async () => ({ accountRef: grant.accountRef }),
      enabled: deps.completionDeps.enabled,
      // The minted credit is the available balance; the metering hook decrements
      // it receipt-first on the real completion.
      readAvailableMsat: async () => grant.grantedMsat,
    })
    // Attach the Payment-Receipt only on a 2xx (success). Core spec: receipts are
    // issued only on success; error responses carry none.
    if (response.status < 200 || response.status >= 300) {
      return response
    }
    const headers = new Headers(response.headers)
    headers.set(
      'payment-receipt',
      buildPaymentReceipt({
        method: receipt.method,
        reference: receipt.reference,
        timestamp: nowIso,
      }),
    )
    headers.set('cache-control', 'private')
    return new Response(response.body, {
      headers,
      status: response.status,
      statusText: response.statusText,
    })
  })

// Settle + serve once a credential has passed stateless verification. The method
// chooses the settlement rail. On a settled payment: mint credit (idempotent),
// run the completion, attach the receipt. Anything not settled => 402 (never
// serves unpaid).
const settleAndServe = (
  request: Request,
  deps: MppChatCompletionsDeps,
  stripeDeps: Parameters<typeof retrievePaymentIntent>[0],
  verified: Extract<
    Awaited<ReturnType<typeof verifyCredential>>,
    { ok: true }
  >,
  credential: ParsedPaymentCredential,
) =>
  Effect.gen(function* () {
    // Resolve the settled PaymentIntent for the rail.
    let settledPi:
      | Readonly<{ id: string; amountCents: number }>
      | undefined

    if (verified.method === 'stripe') {
      // ---- card/SPT rail ----
      const spt = credential.payload.spt
      if (typeof spt !== 'string' || !spt.startsWith('spt_')) {
        return noStoreJsonResponse(
          { error: 'payment_verification_failed', reason: 'missing_spt' },
          { status: 402 },
        )
      }
      const profileId = deps.stripeNetworkProfileId
      if (profileId === undefined || profileId.trim() === '') {
        // Card rail not configured — should not happen (we never offered it) but
        // fail-closed.
        return noStoreJsonResponse(
          { error: 'payment_verification_failed', reason: 'card_rail_off' },
          { status: 402 },
        )
      }
      // Single-use SPT: claim BEFORE charging. A replay collides and is refused.
      const claimed = yield* claimSpt(deps.db, {
        challengeId: credential.challenge.id,
        spt,
      }).pipe(
        Effect.map(ok => ({ ok: true as const, claimed: ok })),
        Effect.catch(() => Effect.succeed({ ok: false as const })),
      )
      if (!claimed.ok) {
        return noStoreJsonResponse(
          { error: 'payment_verification_failed', reason: 'replay_store' },
          { status: 502 },
        )
      }
      if (!claimed.claimed) {
        return noStoreJsonResponse(
          { error: 'payment_verification_failed', reason: 'spt_replayed' },
          { status: 402 },
        )
      }
      const charged = yield* createSptChargePaymentIntent(stripeDeps, {
        amountCents: Number(verified.request.amount),
        challengeId: credential.challenge.id,
        metadata: { model: verified.opaque?.model ?? '', product: 'openagents_khala_mpp' },
        networkProfileId: profileId,
        spt,
      }).pipe(
        Effect.map(intent => ({ ok: true as const, intent })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.detail }),
        ),
      )
      if (!charged.ok) {
        return noStoreJsonResponse(
          { error: 'payment_verification_failed', reason: charged.reason },
          { status: 502 },
        )
      }
      if (!charged.intent.settled) {
        return noStoreJsonResponse(
          {
            error: 'payment_not_settled',
            payment_intent: charged.intent.id,
            status: charged.intent.status,
          },
          { status: 402 },
        )
      }
      yield* recordSptPaymentIntent(deps.db, {
        paymentIntentId: charged.intent.id,
        spt,
      }).pipe(Effect.catch(() => Effect.void))
      settledPi = { amountCents: charged.intent.amountCents, id: charged.intent.id }
    } else {
      // ---- crypto rail ----
      // Recover our deposit PaymentIntent id from the bound `opaque`.
      const paymentIntentId = verified.opaque?.pi
      if (paymentIntentId === undefined || paymentIntentId.trim() === '') {
        return noStoreJsonResponse(
          { error: 'payment_verification_failed', reason: 'missing_correlation' },
          { status: 402 },
        )
      }
      const retrieved = yield* retrievePaymentIntent(
        stripeDeps,
        paymentIntentId,
      ).pipe(
        Effect.map(intent => ({ ok: true as const, intent })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.detail }),
        ),
      )
      if (!retrieved.ok) {
        return noStoreJsonResponse(
          { error: 'payment_verification_failed', reason: retrieved.reason },
          { status: 502 },
        )
      }
      if (!retrieved.intent.settled) {
        return noStoreJsonResponse(
          {
            error: 'payment_not_settled',
            payment_intent: retrieved.intent.id,
            status: retrieved.intent.status,
          },
          { status: 402 },
        )
      }
      settledPi = {
        amountCents: retrieved.intent.amountCents,
        id: retrieved.intent.id,
      }
    }

    // SETTLED. Mint Khala credits for the settled amount into a payer-bound
    // balance (idempotent per payment id), then run the SAME Khala completion.
    const accountRef = mppPayerAccountRef(settledPi.id)
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
        amountCents: settledPi.amountCents,
        paymentIntentId: settledPi.id,
      },
    ).pipe(
      Effect.map(outcome => ({ ok: true as const, outcome })),
      Effect.catch(() => Effect.succeed({ ok: false as const } as const)),
    )
    if (!grant.ok) {
      return noStoreJsonResponse(
        { error: 'credit_grant_failed', payment_intent: settledPi.id },
        { status: 500 },
      )
    }

    return yield* runPaidCompletion(request, deps, grant.outcome, {
      method: verified.method,
      reference: settledPi.id,
    })
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
    // FAIL-SAFE GATE 3: no signing secret => inert. Cannot bind/verify a
    // challenge, so we never issue one or accept a credential.
    if (
      deps.signingSecret === undefined ||
      deps.signingSecret.trim() === ''
    ) {
      return notConfigured('signing secret not configured')
    }
    const signingSecret = deps.signingSecret

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
    const cardEnabled =
      deps.stripeNetworkProfileId !== undefined &&
      deps.stripeNetworkProfileId.trim() !== ''

    // We need the body for BOTH the 402 quote (model) and the paid retry (the
    // completion). Read it once; the underlying handler reads its own clone.
    const rawBody = yield* Effect.promise(async () => {
      try {
        return (await request.clone().json()) as Record<string, unknown>
      } catch {
        return undefined
      }
    })

    const credential = parsePaymentCredential(
      request.headers.get('authorization'),
    )

    // ---- NO CREDENTIAL (or malformed): return 402 + challenge(s) ----
    if (credential === undefined) {
      return yield* issueChallenge(
        deps,
        signingSecret,
        stripeDeps,
        cardEnabled,
        rawBody,
      )
    }

    // ---- CREDENTIAL PRESENT: verify the HMAC binding FAIL-CLOSED ----
    const model = resolveMppModel(rawBody)
    const cryptoQuote = quoteMppCall({ model, rail: 'crypto' as MppRail })
    const cardQuote = quoteMppCall({ model, rail: 'card' as MppRail })
    const allowedMethods = cardEnabled
      ? [...CRYPTO_NETWORKS, 'stripe']
      : [...CRYPTO_NETWORKS]

    const verified = yield* Effect.promise(() =>
      verifyCredential(signingSecret, credential, {
        allowedMethods,
        expectedCurrencyForMethod: method =>
          method === 'stripe' ? 'usd' : 'usdc',
        // The card rail floor is the card quote; crypto uses the crypto quote.
        // Use the lower of the two so neither rail under-floors; verification
        // separately binds the exact amount via the HMAC over `request`.
        expectedMinAmountCents: Math.min(
          cryptoQuote.amountCents,
          cardQuote.amountCents,
        ),
        ...(deps.nowMs === undefined ? {} : { nowMs: deps.nowMs() }),
        realm: MPP_REALM,
      }),
    )

    if (!verified.ok) {
      // Verification failed => fresh 402 (never serve). Re-issue a challenge so a
      // well-behaved client can retry with a correctly-bound credential.
      const detail = `Payment credential rejected (${verified.reason}). Retry against a fresh challenge.`
      return yield* issueChallenge(
        deps,
        signingSecret,
        stripeDeps,
        cardEnabled,
        rawBody,
        detail,
      )
    }

    // ---- VERIFIED: settle for the rail, then serve ----
    return yield* settleAndServe(
      request,
      deps,
      stripeDeps,
      verified,
      credential,
    )
  })
