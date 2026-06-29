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
import { Cause, Duration, Effect } from 'effect'

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
import {
  type SupplyLaneArming,
  khalaBackingPriceModel,
  resolveNamedModelServability,
} from '../model-serving-policy'
import {
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  HYDRALISK_GPT_OSS_120B_MODEL_ID,
  KHALA_MODEL_ID,
  KHALA_MODEL_SLUG,
} from '../pricing'
import {
  type MppCreditGrantOutcome,
  mintLightningCredits,
  mintMppCredits,
  mppLightningPayerAccountRef,
  mppPayerAccountRef,
} from './mpp-credit-grant'
import { type MintLightningInvoice } from './mpp-lightning-invoice'
import { claimLightningPaymentHash } from './mpp-lightning-replay'
import { readPreimage, verifyLightningPreimage } from './mpp-lightning-verify'
import {
  type MppRail,
  quoteMppCall,
  quoteMppLightningCall,
} from './mpp-pricing'
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
import { claimSpt, recordSptPaymentIntent } from './mpp-spt-replay'
import {
  type StripeFetch,
  createCryptoDepositPaymentIntent,
  createSptChargePaymentIntent,
  retrievePaymentIntent,
} from './stripe-mpp-client'

// Default model + crypto networks the endpoint quotes for. Public model
// selection collapses to one Khala id; raw GPT-OSS is internal supply only.
const DEFAULT_MPP_MODEL = KHALA_MODEL_ID
export const SUPPORTED_MPP_MODEL_EXAMPLES: ReadonlyArray<string> = [
  KHALA_MODEL_ID,
]
// x402 (Base) + MPP (Solana, Tempo) — all USDC. We advertise all three crypto
// networks; the agent picks one.
const CRYPTO_NETWORKS: ReadonlyArray<string> = ['base', 'solana', 'tempo']
// The Lightning rail method name (draft-lightning-charge-00). Bitcoin-first: this
// is the PREFERRED rail and is surfaced FIRST in the multi-method 402.
const LIGHTNING_METHOD = 'lightning'
// The protection-space realm for our challenges (core spec §5.1.1).
const MPP_REALM = 'openagents.com'
// Challenge validity window.
const CHALLENGE_TTL_MS = 5 * 60 * 1000
// PER-RAIL ISOLATION GUARD for the Lightning leg of challenge issuance. The
// MDK-backed invoice issuer already caps its own mint round-trip
// (`MDK_LIGHTNING_MINT_TIMEOUT_MS`, ~2s), but the route MUST NOT trust any single
// issuer to be well-behaved: a buggy, mis-wired, or non-MDK issuer could hang,
// reject in an unmapped way, or die with a defect. So the route wraps the WHOLE
// Lightning leg in its own bounded, defect-swallowing guard (`Effect.timeout` +
// `Effect.catchAllCause`) that resolves to "no Lightning challenge" on ANY
// failure. Slightly LARGER than the issuer's internal mint timeout so the inner,
// more specific bound normally wins; this is the outer safety net. The Lightning
// leg runs CONCURRENTLY with the crypto deposit (see `issueChallenge`), so even
// at the full budget it never DELAYS the other rails — a slow/failed Lightning
// rail can only ever drop ITSELF.
//
// BUDGET (#6049): raised to 6.5s so it stays ABOVE the Spark primary mint budget
// (`SPARK_LIGHTNING_MINT_TIMEOUT_MS = 6000`). PROD `wrangler tail` showed the
// real warm `MdkTreasuryContainer` mint subrequest returning 200 in ~3.76–3.95s,
// and the worker-side round-trip lands at/just over 4s — so the outer guard must
// sit well above that to not cut a real mint off before it can surface the
// Lightning rail. The guard still BOUNDS the whole leg: a mint that exceeds 6.5s
// is interrupted and the Lightning rail is dropped (honesty gate, #6149), so the
// 402 can never hang and crypto + card stay fast. This trades ~4–5s of 402
// latency (when Lightning mints) for Lightning visibility; the zero-latency fix
// is a pre-minted Spark invoice pool (future optimization, tracked on #6049).
const LIGHTNING_LEG_GUARD_MS = 6_500

// Parse the KHALA_MPP_ENABLED flag. Default OFF.
export const isKhalaMppEnabled = (value: string | undefined): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

// Parse the KHALA_MPP_LIGHTNING_ENABLED flag. Default OFF. The Lightning rail is
// additionally gated on a configured invoice issuer being present (the MDK
// wallet binding) — see the deps below.
export const isKhalaMppLightningEnabled = (
  value: string | undefined,
): boolean => {
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
  // The KHALA_MPP_LIGHTNING_ENABLED flag, parsed. Default OFF => no Lightning
  // rail offered. HONESTY GATE: the Lightning rail is offered ONLY when this is
  // on AND `mintLightningInvoice` is present (a working invoice issuer) — we
  // never advertise a rail we cannot fulfill.
  lightningEnabled?: boolean | undefined
  // The injectable BOLT11 invoice issuer (the MDK sidecar/route-backed minter in
  // production; a fake in tests). Absent => no Lightning rail, even if the flag
  // is on. Real Bitcoin inbound: the verify path is LOCAL (sha256(preimage)).
  mintLightningInvoice?: MintLightningInvoice | undefined
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
  noStoreJsonResponse({ error: 'mpp_not_configured', reason }, { status: 503 })

export const isMppSellableModel = (model: string): boolean => {
  const normalized = model.trim().toLowerCase()
  return normalized === KHALA_MODEL_ID
}

const isDirectGptOssModel = (model: string): boolean => {
  const normalized = model.trim().toLowerCase()
  return (
    normalized === HYDRALISK_GPT_OSS_20B_MODEL_ID ||
    normalized === HYDRALISK_GPT_OSS_120B_MODEL_ID
  )
}

const modelDescription = (model: string): string =>
  `OpenAgents Khala ${model.trim().toLowerCase()} pay-per-call`

const mppProductForModel = (_model: string | undefined): string =>
  'openagents_khala_mpp'

const priceModelForMppModel = (
  model: string,
  laneArming: SupplyLaneArming | undefined,
): string | undefined =>
  laneArming !== undefined && model === KHALA_MODEL_ID
    ? khalaBackingPriceModel(laneArming)
    : undefined

type MppModelResolution =
  | Readonly<{ ok: true; model: string }>
  | Readonly<{ ok: false; response: Response }>

// Resolve the requested model from the body, defaulting to the single public
// Khala model id, and reject unsupported ids BEFORE issuing a payment challenge.
// This avoids silently charging for a different model than the buyer requested.
const resolveMppModel = (
  rawBody: Record<string, unknown> | undefined,
  laneArming?: SupplyLaneArming | undefined,
): MppModelResolution => {
  const rawModel =
    typeof rawBody?.model === 'string' ? rawBody.model : undefined
  const trimmed = rawModel?.trim()
  const model =
    trimmed === undefined || trimmed === ''
      ? DEFAULT_MPP_MODEL
      : resolveRequestedModel(trimmed).trim().toLowerCase()

  if (trimmed?.trim().toLowerCase() === KHALA_MODEL_SLUG) {
    return {
      ok: false,
      response: noStoreJsonResponse(
        {
          detail:
            'Use the external OpenAI-compatible model id openagents/khala on MPP.',
          error: 'mpp_model_not_supported',
          model: KHALA_MODEL_SLUG,
          supported_models: SUPPORTED_MPP_MODEL_EXAMPLES,
          supported_model_pattern: KHALA_MODEL_ID,
        },
        { status: 400 },
      ),
    }
  }

  if (isDirectGptOssModel(model)) {
    return {
      ok: false,
      response: noStoreJsonResponse(
        {
          detail:
            'Raw GPT-OSS model ids are internal Hydralisk supply and are consumed only through openagents/khala.',
          error: 'model_not_public',
          model,
          supported_models: SUPPORTED_MPP_MODEL_EXAMPLES,
        },
        { status: 403 },
      ),
    }
  }

  if (isMppSellableModel(model)) {
    if (
      laneArming !== undefined &&
      resolveNamedModelServability(model, laneArming) === false
    ) {
      return {
        ok: false,
        response: noStoreJsonResponse(
          { error: 'model_unavailable', model },
          { status: 400 },
        ),
      }
    }
    return { model, ok: true }
  }

  return {
    ok: false,
    response: noStoreJsonResponse(
      {
        error: 'mpp_model_not_supported',
        model,
        supported_models: SUPPORTED_MPP_MODEL_EXAMPLES,
        supported_model_pattern: KHALA_MODEL_ID,
      },
      { status: 400 },
    ),
  }
}

// Build a Lightning charge challenge (draft-lightning-charge-00). amount is in
// SATS, currency "sat"; methodDetails carries the BOLT11 invoice + paymentHash +
// network (all PUBLIC). The opaque carries the paymentHash (`ph`) and the
// invoice expiry (`invExp`) so the retry recovers the settlement target + the
// invoice-expiry signal STATELESSLY. The HMAC `id` is computed inside
// `buildChallenge`. The preimage NEVER appears here.
const buildLightningChallenge = (
  signingSecret: string,
  input: Readonly<{
    model: string
    expires: string
    amountSats: number
    bolt11: string
    paymentHash: string
    network: string
    invoiceExpiresAt: string | undefined
  }>,
) =>
  Effect.promise(() =>
    buildChallenge(signingSecret, {
      // amountCents is the decoded mirror field; for Lightning it carries the
      // SAT amount (sat-native, not cents). Documented at the type.
      amountCents: input.amountSats,
      currency: 'sat',
      expires: input.expires,
      method: LIGHTNING_METHOD,
      network: input.network,
      opaque: {
        amount: String(input.amountSats),
        ...(input.invoiceExpiresAt === undefined
          ? {}
          : { invExp: input.invoiceExpiresAt }),
        model: input.model,
        network: input.network,
        ph: input.paymentHash,
      },
      realm: MPP_REALM,
      request: {
        amount: String(input.amountSats),
        currency: 'sat',
        description: modelDescription(input.model),
        methodDetails: {
          invoice: input.bolt11,
          network: input.network,
          paymentHash: input.paymentHash,
        },
        network: input.network,
      },
    }),
  )

// Build the spec-shaped challenge set for a quote. When a pre-built Lightning
// challenge is supplied it is FIRST (Bitcoin-first / preferred rail). Always
// includes the crypto rail (one challenge per supported network, all bound to
// the SAME crypto deposit PaymentIntent recovered from `opaque`); adds the
// card/SPT rail only when a network profile id is configured. Each challenge's
// `id` is the HMAC binding computed inside `buildChallenge`.
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
    // Pre-built Lightning challenge, prepended FIRST when present.
    lightningChallenge: MppChallenge | undefined
  }>,
) =>
  Effect.gen(function* () {
    const challenges: Array<MppChallenge> = []
    // Bitcoin-first: the Lightning challenge is offered FIRST when armed.
    if (input.lightningChallenge !== undefined) {
      challenges.push(input.lightningChallenge)
    }
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
          description: modelDescription(input.model),
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
          opaque: {
            amount: String(input.cardAmountCents),
            model: input.model,
            network: 'stripe',
          },
          realm: MPP_REALM,
          request: {
            amount: String(input.cardAmountCents),
            currency: 'usd',
            description: modelDescription(input.model),
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

// The Lightning rail is active only when the flag is on AND an invoice issuer is
// wired (HONESTY GATE: never advertise a rail we cannot fulfill).
const lightningRailActive = (
  deps: MppChatCompletionsDeps,
): MintLightningInvoice | undefined =>
  deps.lightningEnabled === true && deps.mintLightningInvoice !== undefined
    ? deps.mintLightningInvoice
    : undefined

// Mint a Lightning invoice + build the (Bitcoin-first) Lightning challenge for
// this quote. Best-effort and FULLY ISOLATED: if the invoice issuer fails,
// times out, or dies with a defect we return undefined and the 402 still carries
// the other rails — we just do not advertise a Lightning rail we could not
// actually mint an invoice for (honesty gate). The effective challenge expiry is
// the EARLIER of the challenge TTL and the invoice expiry.
//
// SAFETY (the central fix): this leg can NEVER hang or break the 402. The whole
// computation is bounded by `LIGHTNING_LEG_GUARD_MS` and ANY failure cause —
// typed `LightningInvoiceError`, a hang/timeout, OR an unexpected defect from a
// mis-wired issuer — is swallowed back to `undefined` (drop the rail). The
// observed prod hang was a cold MDK-sidecar container blocking the entire
// (sequential) issuance Effect; here a hung mint is interrupted and dropped.
const buildLightningChallengeForQuote = (
  deps: MppChatCompletionsDeps,
  signingSecret: string,
  mint: MintLightningInvoice,
  input: Readonly<{
    model: string
    priceModel?: string
    newId: () => string
    challengeExpiresAt: string
  }>,
): Effect.Effect<MppChallenge | undefined, never> =>
  Effect.gen(function* () {
    const abortController = new AbortController()
    const nowMs = deps.nowMs ?? currentEpochMillis
    const legStartMs = nowMs()
    return yield* Effect.gen(function* () {
      const lightningQuote = quoteMppLightningCall({
        model: input.model,
        ...(input.priceModel === undefined
          ? {}
          : { priceModel: input.priceModel }),
      })
      const invoice = yield* mint({
        abortSignal: abortController.signal,
        amountSats: lightningQuote.amountSats,
        correlationRef: `mpp:lightning:${input.model}:${input.newId()}`,
        description: modelDescription(input.model),
      }).pipe(
        // DIAGNOSTIC (#6049): observe WHY a Lightning leg drops in prod tail. The
        // mint cause is the actual drop reason (typed `provider_unavailable` /
        // `provider_rejected` / `malformed_invoice`, an interrupt, or the inner
        // mint timeout). Logged with elapsed ms; carries NO invoice/secret content.
        Effect.tapCause((cause: Cause.Cause<unknown>) =>
          Effect.logWarning('mpp_lightning_leg_dropped', {
            elapsedMs: nowMs() - legStartMs,
            model: input.model,
            reason: Cause.pretty(cause).slice(0, 240),
          }),
        ),
      )
      // Effective expiry = earlier of challenge TTL and invoice BOLT11 expiry
      // (spec §"Challenge Expiry and Invoice Expiry"). The challenge `expires`
      // auth-param MUST NOT be later than the invoice expiry.
      const challengeExpires =
        invoice.invoiceExpiresAt !== undefined &&
        Date.parse(invoice.invoiceExpiresAt) <
          Date.parse(input.challengeExpiresAt)
          ? invoice.invoiceExpiresAt
          : input.challengeExpiresAt
      const challenge = yield* buildLightningChallenge(signingSecret, {
        amountSats: lightningQuote.amountSats,
        bolt11: invoice.bolt11,
        expires: challengeExpires,
        invoiceExpiresAt: invoice.invoiceExpiresAt,
        model: input.model,
        network: invoice.network,
        paymentHash: invoice.paymentHash,
      })
      // DIAGNOSTIC (#6049): a Lightning leg that actually minted + surfaced, with
      // elapsed ms. NO invoice/secret content.
      yield* Effect.logInfo('mpp_lightning_leg_surfaced', {
        elapsedMs: nowMs() - legStartMs,
        model: input.model,
      })
      return challenge
    }).pipe(Effect.ensuring(Effect.sync(() => abortController.abort())))
  }).pipe(
    // Outer per-rail safety net: bound the WHOLE leg, then swallow ANY cause
    // (typed error, the timeout's `TimeoutException`, or an interrupt/defect)
    // back to "no Lightning challenge". This is what makes a Lightning failure
    // ALWAYS safe regardless of issuer behavior. `catchCause` (not `catch`)
    // recovers from BOTH recoverable errors and unrecoverable defects.
    Effect.timeout(Duration.millis(LIGHTNING_LEG_GUARD_MS)),
    // DIAGNOSTIC (#6049): if the OUTER guard fires (the whole leg exceeded
    // `LIGHTNING_LEG_GUARD_MS`), record it distinctly from an inner mint failure.
    Effect.tapCause((cause: Cause.Cause<unknown>) =>
      Effect.logWarning('mpp_lightning_leg_guard_fired', {
        reason: Cause.pretty(cause).slice(0, 240),
      }),
    ),
    Effect.catchCause(() => Effect.as(Effect.void, undefined)),
  )

// Issue a fresh 402 (no credential, or a credential we refused). Creates a
// crypto deposit PaymentIntent for the quote (the address the crypto challenge
// points at) and builds the bound challenge set. When the Lightning rail is
// armed the Lightning challenge is offered FIRST (Bitcoin-first). If Stripe
// cannot quote, returns a clean 503 (never a broken 402).
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
    const resolution = resolveMppModel(rawBody, deps.completionDeps.laneArming)
    if (!resolution.ok) {
      return resolution.response
    }
    const model = resolution.model
    const priceModel = priceModelForMppModel(
      model,
      deps.completionDeps.laneArming,
    )
    const cryptoQuote = quoteMppCall({
      model,
      ...(priceModel === undefined ? {} : { priceModel }),
      rail: 'crypto' as MppRail,
    })
    const cardQuote = quoteMppCall({
      model,
      ...(priceModel === undefined ? {} : { priceModel }),
      rail: 'card' as MppRail,
    })
    const challengeExpiresAt = epochMillisToIsoTimestamp(
      nowMs + CHALLENGE_TTL_MS,
    )

    // PER-RAIL ISOLATION: the Lightning leg ("Bitcoin-first" in PRESENTATION
    // order) and the crypto deposit creation run CONCURRENTLY, so a slow/cold
    // Lightning mint can never delay the crypto/card rails beyond its own bounded
    // guard. The Lightning leg is fully self-contained — it resolves to
    // `undefined` (drop the rail) on ANY failure/timeout/defect and never fails
    // this Effect (`Effect.Effect<…, never>`). Presentation order still puts
    // Lightning FIRST, but only when it actually minted (see `buildChallenges`).
    const mint = lightningRailActive(deps)
    const [lightningChallenge, created] = yield* Effect.all(
      [
        mint === undefined
          ? Effect.as(Effect.void, undefined)
          : buildLightningChallengeForQuote(deps, signingSecret, mint, {
              challengeExpiresAt,
              model,
              newId,
              ...(priceModel === undefined ? {} : { priceModel }),
            }),
        createCryptoDepositPaymentIntent(stripeDeps, {
          amountCents: cryptoQuote.amountCents,
          idempotencyKey: `mpp:quote:${newId()}`,
          metadata: { model, product: mppProductForModel(model) },
          networks: CRYPTO_NETWORKS,
        }).pipe(
          Effect.map(intent => ({ ok: true as const, intent })),
          Effect.catch(error =>
            Effect.succeed({ ok: false as const, reason: error.detail }),
          ),
        ),
      ] as const,
      { concurrency: 'unbounded' },
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
      expires: challengeExpiresAt,
      lightningChallenge,
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

// Settle + serve the LIGHTNING rail once a credential passed the generic HMAC
// verification. Lightning settlement is LOCAL (draft-lightning-charge-00
// §Verification): no node call. We:
//   1. recover the bound paymentHash from the verified `opaque` (`ph`);
//   2. enforce the invoice-expiry signal (`invExp`) — the EARLIER-of expiry —
//      on top of the challenge expiry the generic verify already checked;
//   3. verify sha256(payload.preimage) == paymentHash, fail-closed;
//   4. atomically CONSUME-ONCE on the paymentHash (claim before serve);
//   5. mint BITCOIN-ORIGIN credit (idempotent per paymentHash, NOT usd_credit),
//      run the SAME Khala completion, attach a Payment-Receipt whose `reference`
//      is the paymentHash (NEVER the preimage — bearer secret).
// Any failure => fresh 402 (never serves unpaid). The preimage is never logged,
// persisted, or returned.
const settleAndServeLightning = (
  request: Request,
  deps: MppChatCompletionsDeps,
  verified: Extract<Awaited<ReturnType<typeof verifyCredential>>, { ok: true }>,
  credential: ParsedPaymentCredential,
) =>
  Effect.gen(function* () {
    const nowMs = (deps.nowMs ?? currentEpochMillis)()
    const paymentHash = verified.opaque?.ph
    if (paymentHash === undefined || paymentHash.trim() === '') {
      return noStoreJsonResponse(
        { error: 'payment_verification_failed', reason: 'missing_correlation' },
        { status: 402 },
      )
    }

    // (2) Invoice expiry: the EARLIER-of expiry. The generic verify already
    // rejected a past challenge `expires`; this additionally rejects a past
    // invoice expiry (spec §"Challenge Expiry and Invoice Expiry").
    const invExp = verified.opaque?.invExp
    if (invExp !== undefined && invExp !== '') {
      const invExpMs = Date.parse(invExp)
      if (Number.isNaN(invExpMs) || invExpMs <= nowMs) {
        return noStoreJsonResponse(
          { error: 'payment_verification_failed', reason: 'expired_invoice' },
          { status: 402 },
        )
      }
    }

    // (3) Local preimage verification — sha256(preimage) == paymentHash.
    const preimage = readPreimage(credential.payload)
    const preimageResult = yield* Effect.promise(() =>
      verifyLightningPreimage(preimage, paymentHash),
    )
    if (!preimageResult.ok) {
      // malformed-credential vs invalid-preimage (spec problem types). Either
      // way => fresh 402 (never serve). The preimage value is never echoed.
      return noStoreJsonResponse(
        {
          error: 'payment_verification_failed',
          reason:
            preimageResult.reason === 'malformed'
              ? 'malformed_preimage'
              : 'invalid_preimage',
        },
        { status: 402 },
      )
    }

    // (4) Atomic consume-once on the paymentHash (claim BEFORE serve). A replay
    // collides on the PRIMARY KEY and is refused before a second free serve.
    const claimed = yield* claimLightningPaymentHash(deps.db, {
      challengeId: credential.challenge.id,
      paymentHash,
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
        { error: 'payment_verification_failed', reason: 'preimage_replayed' },
        { status: 402 },
      )
    }

    // (5) SETTLED (real Bitcoin). Mint BITCOIN-ORIGIN credit (NOT usd_credit),
    // idempotent per paymentHash, then run the SAME Khala completion.
    const amountSats = Number(verified.request.amount)
    if (!Number.isFinite(amountSats) || amountSats <= 0) {
      return noStoreJsonResponse(
        { error: 'payment_verification_failed', reason: 'amount_invalid' },
        { status: 402 },
      )
    }
    const accountRef = mppLightningPayerAccountRef(paymentHash)
    const grant = yield* mintLightningCredits(
      {
        db: deps.db,
        ...(deps.nowIso === undefined ? {} : { nowIso: deps.nowIso }),
      },
      { accountRef, amountSats, paymentHash },
    ).pipe(
      Effect.map(outcome => ({ ok: true as const, outcome })),
      Effect.catch(() => Effect.succeed({ ok: false as const } as const)),
    )
    if (!grant.ok) {
      return noStoreJsonResponse(
        { error: 'credit_grant_failed', reference: paymentHash },
        { status: 500 },
      )
    }

    // Receipt `reference` = the paymentHash (public), NEVER the preimage.
    return yield* runPaidCompletion(request, deps, grant.outcome, {
      method: LIGHTNING_METHOD,
      reference: paymentHash,
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
  verified: Extract<Awaited<ReturnType<typeof verifyCredential>>, { ok: true }>,
  credential: ParsedPaymentCredential,
) =>
  Effect.gen(function* () {
    // Resolve the settled PaymentIntent for the rail.
    let settledPi: Readonly<{ id: string; amountCents: number }> | undefined

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
        metadata: {
          model: verified.opaque?.model ?? '',
          product: mppProductForModel(verified.opaque?.model),
        },
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
      settledPi = {
        amountCents: charged.intent.amountCents,
        id: charged.intent.id,
      }
    } else {
      // ---- crypto rail ----
      // Recover our deposit PaymentIntent id from the bound `opaque`.
      const paymentIntentId = verified.opaque?.pi
      if (paymentIntentId === undefined || paymentIntentId.trim() === '') {
        return noStoreJsonResponse(
          {
            error: 'payment_verification_failed',
            reason: 'missing_correlation',
          },
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
    if (deps.signingSecret === undefined || deps.signingSecret.trim() === '') {
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
    const resolution = resolveMppModel(rawBody, deps.completionDeps.laneArming)
    if (!resolution.ok) {
      return resolution.response
    }
    const model = resolution.model
    const priceModel = priceModelForMppModel(
      model,
      deps.completionDeps.laneArming,
    )
    const cryptoQuote = quoteMppCall({
      model,
      ...(priceModel === undefined ? {} : { priceModel }),
      rail: 'crypto' as MppRail,
    })
    const cardQuote = quoteMppCall({
      model,
      ...(priceModel === undefined ? {} : { priceModel }),
      rail: 'card' as MppRail,
    })
    const lightningActive = lightningRailActive(deps) !== undefined
    const lightningQuote = quoteMppLightningCall({
      model,
      ...(priceModel === undefined ? {} : { priceModel }),
    })
    // Lightning is offered FIRST when armed (Bitcoin-first). Verification only
    // ACCEPTS a method we actually offered, so a `lightning` credential is
    // refused unless the rail is armed.
    const allowedMethods = [
      ...(lightningActive ? [LIGHTNING_METHOD] : []),
      ...CRYPTO_NETWORKS,
      ...(cardEnabled ? ['stripe'] : []),
    ]

    const verified = yield* Effect.promise(() =>
      verifyCredential(signingSecret, credential, {
        allowedMethods,
        expectedCurrencyForMethod: method =>
          method === LIGHTNING_METHOD
            ? 'sat'
            : method === 'stripe'
              ? 'usd'
              : 'usdc',
        // Per-method floor: Lightning is sat-native (sats floor), the USDC/card
        // rails are cent-native (cents floor). Verification separately binds the
        // exact amount via the HMAC over `request`; this just floors a downward
        // tamper in the method's native unit.
        expectedMinAmountCents: method =>
          method === LIGHTNING_METHOD
            ? lightningQuote.amountSats
            : Math.min(cryptoQuote.amountCents, cardQuote.amountCents),
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

    if (
      verified.opaque?.model !== undefined &&
      verified.opaque.model.trim().toLowerCase() !== model
    ) {
      const detail =
        'Payment credential was issued for a different model. Retry against a fresh challenge.'
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
    // Lightning settles LOCALLY (no Stripe); the USDC/card rails settle via
    // Stripe. Dispatch on the verified method.
    if (verified.method === LIGHTNING_METHOD) {
      return yield* settleAndServeLightning(request, deps, verified, credential)
    }
    return yield* settleAndServe(
      request,
      deps,
      stripeDeps,
      verified,
      credential,
    )
  })
