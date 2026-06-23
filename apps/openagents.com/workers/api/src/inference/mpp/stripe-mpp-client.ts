// Worker-native Stripe REST client for Machine Payments (EPIC #6049, Phase 2).
//
// WHY Worker-native (not the Mppx / Mpp SDK): the `Mppx`/`Mpp` SDKs (and the
// rest of stripe-node's machine-payment helpers) are Node, and the gateway runs
// on Cloudflare Workers. The MPP/x402 server side is a SMALL protocol — a 402
// with a payment challenge, then a credential verify — and the money primitive
// underneath is the standard Stripe PaymentIntents REST API. So we implement the
// two REST calls we need directly against `https://api.stripe.com`, pinned to
// the `2026-03-04.preview` API version the machine-payments feature requires.
//
// This module does exactly two Stripe calls:
//   1. createCryptoDepositPaymentIntent — create a `crypto` deposit-mode
//      PaymentIntent and return the deposit address(es) for the 402 challenge.
//   2. retrievePaymentIntent — read a PaymentIntent to check it has settled
//      (`succeeded`) before serving the paid resource.
//
// NODE-SIDECAR NOTE: the higher-level `Mppx.compose` challenge ENCODING (the
// exact signed `WWW-Authenticate: Payment ...` token format and credential
// binding) is defined by the MPP SDK. We construct a spec-shaped 402 challenge
// here (see mpp-protocol.ts) that an MPP-aware client can act on for the crypto
// deposit-address flow. If full bit-compatible `Mppx`-signed challenge binding
// or SPT settlement proves to need the Node SDK, run a thin Node MPP sidecar
// that fronts this Worker for challenge signing only; the Worker still owns auth,
// the Khala completion, metering, and the receipt. This is documented in the
// PR + the integration plan as a known go-live consideration.
import { Effect } from 'effect'

// The API version the machine-payments / crypto-deposit feature requires
// (Stripe MPP + x402 docs). Pinned here so the Worker-native calls match the
// feature contract regardless of the stripe-node SDK version the repo pins
// elsewhere (`STRIPE_API_VERSION` in stripe-billing.ts is for the card-billing
// surface and is a DIFFERENT, newer version — do not conflate them).
export const STRIPE_MPP_API_VERSION = '2026-03-04.preview'

const STRIPE_API_BASE = 'https://api.stripe.com/v1'

// A genuine transport/Stripe failure (network, 5xx, malformed response). Domain
// outcomes (a PaymentIntent that is not yet settled) are NOT errors — they are
// part of the typed success value. This error channel carries only real
// failures, which the endpoint surfaces as a 5xx / "verify failed".
export class StripeMppError extends Error {
  override readonly name = 'StripeMppError'
  readonly op: string
  readonly detail: string
  constructor(op: string, detail: string) {
    super(`stripe mpp ${op}: ${detail}`)
    this.op = op
    this.detail = detail
  }
}

// Form-encode a flat/nested params object the way the Stripe REST API expects
// (e.g. `payment_method_options[crypto][mode]=deposit`). Supports nested
// objects and arrays (`deposit_options[networks][0]=base`).
export const encodeStripeForm = (params: Record<string, unknown>): string => {
  const out: Array<[string, string]> = []
  const walk = (prefix: string, value: unknown): void => {
    if (value === undefined || value === null) {
      return
    }
    if (Array.isArray(value)) {
      value.forEach((item, index) => walk(`${prefix}[${index}]`, item))
      return
    }
    if (typeof value === 'object') {
      for (const [key, child] of Object.entries(
        value as Record<string, unknown>,
      )) {
        walk(`${prefix}[${key}]`, child)
      }
      return
    }
    out.push([prefix, String(value)])
  }
  for (const [key, value] of Object.entries(params)) {
    walk(key, value)
  }
  return out
    .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v)}`)
    .join('&')
}

// The injectable fetch seam so tests run without network. Defaults to global
// fetch. A test passes a fake that returns canned Stripe JSON.
export type StripeFetch = (url: string, init: RequestInit) => Promise<Response>

export type StripeMppClientDeps = Readonly<{
  // The Stripe SECRET key (live or test). NEVER logged.
  secretKey: string
  // Injectable fetch (defaults to global fetch).
  fetch?: StripeFetch
  // Optional network profile id (`profile_…`) for SPT/card settlement. Absent =>
  // the card/SPT rail is not configured (crypto-only).
  networkProfileId?: string | undefined
}>

const authHeaders = (secretKey: string): Record<string, string> => ({
  authorization: `Bearer ${secretKey}`,
  'content-type': 'application/x-www-form-urlencoded',
  // Pin the machine-payments preview API version.
  'stripe-version': STRIPE_MPP_API_VERSION,
})

// A crypto deposit address returned by a deposit-mode crypto PaymentIntent.
export type CryptoDepositAddress = Readonly<{
  network: string
  address: string
}>

export type CreatedCryptoPaymentIntent = Readonly<{
  id: string
  status: string
  amountCents: number
  currency: string
  // Deposit addresses, one per requested network, the agent pays to.
  deposits: ReadonlyArray<CryptoDepositAddress>
}>

// Extract deposit addresses from a PaymentIntent's next_action crypto display
// details. Stripe's deposit-mode crypto response exposes
// `next_action.crypto_display_details.deposit_addresses` as an OBJECT keyed by
// network name, where each value carries the on-chain `address` (and a
// `supported_tokens` list). The network is the KEY; there is no `network` field
// on the value. See the MPP/x402 quickstarts and the deposit-mode stablecoin
// guide (e.g. `deposit_addresses["base"].address`). This is the authoritative
// shape; we add a cheap tolerant fallback for a single top-level address only.
const extractDeposits = (
  intent: Record<string, unknown>,
): ReadonlyArray<CryptoDepositAddress> => {
  const nextAction = (intent.next_action ?? {}) as Record<string, unknown>
  const display = (nextAction.crypto_display_details ?? {}) as Record<
    string,
    unknown
  >
  const depositAddresses = display.deposit_addresses
  if (
    depositAddresses !== null &&
    typeof depositAddresses === 'object' &&
    !Array.isArray(depositAddresses)
  ) {
    const out: Array<CryptoDepositAddress> = []
    for (const [network, value] of Object.entries(
      depositAddresses as Record<string, unknown>,
    )) {
      const entry = (value ?? {}) as Record<string, unknown>
      const address = entry.address
      if (typeof network === 'string' && typeof address === 'string') {
        out.push({ address, network })
      }
    }
    return out
  }
  // Cheap tolerant fallback: a single address + network at the top of display.
  const network = display.network
  const address = display.address ?? display.deposit_address
  if (typeof network === 'string' && typeof address === 'string') {
    return [{ address, network }]
  }
  return []
}

// Create a crypto deposit-mode PaymentIntent for the given amount + networks and
// return the deposit address(es). This is the address the 402 crypto challenge
// points the agent at. Idempotent per `idempotencyKey`.
export const createCryptoDepositPaymentIntent = (
  deps: StripeMppClientDeps,
  input: Readonly<{
    amountCents: number
    currency?: string
    networks: ReadonlyArray<string>
    idempotencyKey: string
    metadata?: Record<string, string>
  }>,
): Effect.Effect<CreatedCryptoPaymentIntent, StripeMppError> =>
  Effect.gen(function* () {
    const doFetch = deps.fetch ?? ((u, i) => fetch(u, i))
    const currency = input.currency ?? 'usd'
    const body = encodeStripeForm({
      amount: input.amountCents,
      confirm: true,
      currency,
      ...(input.metadata === undefined ? {} : { metadata: input.metadata }),
      payment_method_data: { type: 'crypto' },
      // Stripe's official MPP/x402 SDK samples set this alongside
      // payment_method_data[type]=crypto for deposit-mode crypto PaymentIntents.
      payment_method_types: ['crypto'],
      payment_method_options: {
        crypto: {
          deposit_options: { networks: input.networks },
          mode: 'deposit',
        },
      },
    })

    const response = yield* Effect.tryPromise({
      catch: (cause: unknown) =>
        new StripeMppError('create_payment_intent', String(cause)),
      try: () =>
        doFetch(`${STRIPE_API_BASE}/payment_intents`, {
          body,
          headers: {
            ...authHeaders(deps.secretKey),
            'idempotency-key': input.idempotencyKey,
          },
          method: 'POST',
        }),
    })

    const json = yield* Effect.tryPromise({
      catch: (cause: unknown) =>
        new StripeMppError('create_payment_intent_parse', String(cause)),
      try: () => response.json() as Promise<Record<string, unknown>>,
    })

    if (!response.ok) {
      const err = (json.error ?? {}) as Record<string, unknown>
      return yield* Effect.fail(
        new StripeMppError(
          'create_payment_intent',
          `${response.status} ${String(err.message ?? 'stripe error')}`,
        ),
      )
    }

    const id = json.id
    if (typeof id !== 'string') {
      return yield* Effect.fail(
        new StripeMppError(
          'create_payment_intent',
          'response missing payment intent id',
        ),
      )
    }

    return {
      amountCents:
        typeof json.amount === 'number' ? json.amount : input.amountCents,
      currency: typeof json.currency === 'string' ? json.currency : currency,
      deposits: extractDeposits(json),
      id,
      status: typeof json.status === 'string' ? json.status : 'unknown',
    }
  })

export type RetrievedPaymentIntent = Readonly<{
  id: string
  status: string
  amountCents: number
  currency: string
  // True when the payment has settled and the resource may be served.
  settled: boolean
  metadata: Record<string, string>
}>

// Retrieve a PaymentIntent to check it has settled. In deposit mode Stripe
// auto-captures the PaymentIntent after the on-chain funds settle, so a
// `succeeded` status is the only settled state — it means the agent has paid
// and we may serve. Anything else (`requires_payment_method`, `processing`,
// `requires_action`, etc.) is NOT settled — a domain outcome, not an error.
export const retrievePaymentIntent = (
  deps: StripeMppClientDeps,
  paymentIntentId: string,
): Effect.Effect<RetrievedPaymentIntent, StripeMppError> =>
  Effect.gen(function* () {
    const doFetch = deps.fetch ?? ((u, i) => fetch(u, i))
    const response = yield* Effect.tryPromise({
      catch: (cause: unknown) =>
        new StripeMppError('retrieve_payment_intent', String(cause)),
      try: () =>
        doFetch(
          `${STRIPE_API_BASE}/payment_intents/${encodeURIComponent(
            paymentIntentId,
          )}`,
          {
            headers: authHeaders(deps.secretKey),
            method: 'GET',
          },
        ),
    })

    const json = yield* Effect.tryPromise({
      catch: (cause: unknown) =>
        new StripeMppError('retrieve_payment_intent_parse', String(cause)),
      try: () => response.json() as Promise<Record<string, unknown>>,
    })

    if (!response.ok) {
      const err = (json.error ?? {}) as Record<string, unknown>
      return yield* Effect.fail(
        new StripeMppError(
          'retrieve_payment_intent',
          `${response.status} ${String(err.message ?? 'stripe error')}`,
        ),
      )
    }

    const status = typeof json.status === 'string' ? json.status : 'unknown'
    const settled = status === 'succeeded'
    const rawMeta = (json.metadata ?? {}) as Record<string, unknown>
    const metadata: Record<string, string> = {}
    for (const [k, v] of Object.entries(rawMeta)) {
      if (typeof v === 'string') {
        metadata[k] = v
      }
    }

    return {
      amountCents: typeof json.amount === 'number' ? json.amount : 0,
      currency: typeof json.currency === 'string' ? json.currency : 'usd',
      id: typeof json.id === 'string' ? json.id : paymentIntentId,
      metadata,
      settled,
      status,
    }
  })
