// MPP service-discovery document for Khala (EPIC #6049).
//
// Serves the OpenAPI 3.1 discovery document the Machine Payments Protocol
// registries (MPPScan, mpp.dev/services) crawl at `GET /openapi.json` to light
// the Machine Payments badge. The document follows the payment-discovery draft
// (`draft-payment-discovery-00`): a standard OpenAPI 3.1 file with two
// extensions — root-level `x-service-info` and per-operation `x-payment-info`
// (canonical multi-offer form).
//
// HONESTY GATE (the central safety property): the document must reflect REALITY.
// We must NEVER advertise a payable endpoint that does not actually serve — a
// broken/inert paid endpoint gets us delisted and burns agents. So the paid MPP
// path's `x-payment-info` offers + `402` are emitted ONLY when the MPP endpoint
// is actually armed (the SAME `KHALA_MPP_ENABLED` flag the route reads via
// `config.ts`). The card/`stripe` offer additionally requires the network
// profile id (the same condition that arms the card rail in the route). When the
// endpoint is inert, the document omits the paid path entirely, so nothing
// payable is advertised. The document itself is always served (it still carries
// `x-service-info` + the free description), so registries can discover the
// service even before the paid rail is armed.
//
// This module is PURE of the MPP verify/settlement code: it only READS the
// pricing model (`mpp-pricing.ts`) and is handed the parsed flags by the route
// wiring (which reads them from the same `config.ts` seam as the MPP route).

import { Effect } from 'effect'

import { isKhalaModel } from './pricing'
import {
  type MppRail,
  quoteMppCall,
  quoteMppLightningCall,
} from './mpp/mpp-pricing'

// The canonical public origin.
const ORIGIN = 'https://openagents.com'

// The paid MPP path (mirrors the route registered in index.ts).
const MPP_PATH = '/mpp/v1/chat/completions'

// The general pay-per-call tier the discovery offers quote against (same default
// as the MPP route's DEFAULT_MPP_MODEL). The runtime 402 challenge re-quotes for
// the requested model and remains authoritative; discovery is advisory.
const DISCOVERY_MODEL = 'openagents/khala-mini'

// The crypto networks the MPP route advertises (USDC on each). Mirrors
// CRYPTO_NETWORKS in the route. Discovery emits one `charge` offer per network.
const CRYPTO_NETWORKS: ReadonlyArray<string> = ['tempo', 'base', 'solana']

// USDC has 6 decimals. Discovery `amount` is a base-units integer string (per
// the draft: smallest denomination of the currency). 0.01 USDC => "10000".
const USDC_DECIMALS = 6

// Convert a USD/USDC price to a USDC base-units integer string (6 decimals).
// Rounds up to whole base units, never under-quoting. The runtime 402 challenge
// is authoritative; this is the advisory display amount.
const usdcBaseUnits = (priceUsd: number): string =>
  String(Math.max(1, Math.ceil(priceUsd * 10 ** USDC_DECIMALS - 1e-9)))

// A single x-payment-info offer (canonical multi-offer entry; draft offer obj).
export type PaymentOffer = Readonly<{
  intent: 'charge'
  method: string
  // Base-units integer string, or null for dynamic pricing. Always a fixed
  // advisory quote here (the runtime 402 re-quotes the real model).
  amount: string
  currency: string
  description: string
}>

export type MppDiscoveryFlags = Readonly<{
  // KHALA_MPP_ENABLED, parsed. When false the paid endpoint is inert; the
  // document omits the paid path so it advertises nothing payable.
  mppEnabled: boolean
  // STRIPE_MPP_NETWORK_PROFILE_ID presence (the same condition that arms the
  // card rail in the route). When absent the document omits the card offer.
  cardRailEnabled: boolean
  // KHALA_MPP_LIGHTNING_ENABLED + a working invoice issuer (the SAME condition
  // that arms the Lightning rail in the route). When present the Lightning offer
  // is listed FIRST (Bitcoin-first / preferred). HONESTY GATE: omitted when the
  // rail is not actually armed — never advertise a rail we cannot fulfill.
  lightningRailEnabled: boolean
}>

// Build the offer set for the paid MPP path. Always includes one USDC `charge`
// offer per crypto network (the crypto rail is the default-on MPP rail). Adds a
// `stripe`/`usd` card offer ONLY when the network profile id is configured —
// the SAME condition that makes the card rail actually serve.
const buildOffers = (
  flags: MppDiscoveryFlags,
): ReadonlyArray<PaymentOffer> => {
  // The discovery model is a known Khala tier; fall back to the default if not.
  const model = isKhalaModel(DISCOVERY_MODEL)
    ? DISCOVERY_MODEL
    : 'openagents/khala-mini'
  const cryptoQuote = quoteMppCall({ model, rail: 'crypto' as MppRail })
  const cryptoAmount = usdcBaseUnits(cryptoQuote.priceUsd)

  // Bitcoin-first: the Lightning offer (sats, real Bitcoin) is listed FIRST when
  // the rail is armed. amount is in SATS (the smallest denomination of the "sat"
  // currency per draft-lightning-charge-00).
  const lightningOffers: ReadonlyArray<PaymentOffer> = flags.lightningRailEnabled
    ? [
        {
          amount: String(quoteMppLightningCall({ model }).amountSats),
          currency: 'sat',
          description: `Pay-per-call Khala chat completion over Lightning (BOLT11; real Bitcoin). Advisory quote for ${model}; the runtime 402 challenge re-quotes the requested model and is authoritative.`,
          intent: 'charge' as const,
          method: 'lightning',
        },
      ]
    : []

  const cryptoOffers: ReadonlyArray<PaymentOffer> = CRYPTO_NETWORKS.map(
    network => ({
      amount: cryptoAmount,
      currency: 'usdc',
      description: `Pay-per-call Khala chat completion in USDC on ${network} (x402/MPP). Advisory quote for ${model}; the runtime 402 challenge re-quotes the requested model and is authoritative.`,
      intent: 'charge' as const,
      method: network,
    }),
  )

  if (!flags.cardRailEnabled) {
    return [...lightningOffers, ...cryptoOffers]
  }

  const cardQuote = quoteMppCall({ model, rail: 'card' as MppRail })
  const cardOffer: PaymentOffer = {
    // Card/SPT amount in USD cents (smallest denomination of USD).
    amount: String(cardQuote.amountCents),
    currency: 'usd',
    description: `Pay-per-call Khala chat completion via card (Stripe Shared Payment Token). Advisory quote for ${model}; the runtime 402 challenge is authoritative.`,
    intent: 'charge' as const,
    method: 'stripe',
  }

  return [...lightningOffers, ...cryptoOffers, cardOffer]
}

// The OpenAI chat-completions request-body schema (model + messages), per the
// draft's Input Schema guidance so agents can construct a valid request.
const CHAT_COMPLETIONS_REQUEST_SCHEMA = {
  type: 'object',
  properties: {
    model: {
      type: 'string',
      description:
        'A Khala model id, e.g. "openagents/khala-mini" or "openagents/khala-code".',
    },
    messages: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          role: { type: 'string' },
          content: { type: 'string' },
        },
        required: ['role', 'content'],
      },
    },
  },
  required: ['model', 'messages'],
} as const

// The paid MPP path operation object. Carries `x-payment-info` (multi-offer) and
// the mandatory `402` response. Only built when the endpoint is armed.
const buildPaidPath = (flags: MppDiscoveryFlags) => ({
  post: {
    summary: 'Khala chat completions (pay-per-call via Machine Payments)',
    description:
      'OpenAI-compatible Chat Completions, paid per request via the Machine Payments Protocol (MPP) / x402. A request with no payment credential returns 402 with a payment challenge; a verified credential runs the completion and returns it with a usage receipt.',
    operationId: 'mppChatCompletions',
    'x-payment-info': {
      offers: buildOffers(flags),
    },
    requestBody: {
      required: true,
      content: {
        'application/json': {
          schema: CHAT_COMPLETIONS_REQUEST_SCHEMA,
        },
      },
    },
    responses: {
      '200': {
        description:
          'Successful chat completion (OpenAI Chat Completions shape plus an `openagents` disclosure block).',
      },
      '402': {
        description: 'Payment Required',
      },
    },
  },
})

// Build the OpenAPI 3.1 discovery document. PURE — given the parsed flags it is
// fully deterministic. When the MPP endpoint is inert (flag off), the paid path
// is OMITTED so the document advertises nothing payable; only the free
// description + `x-service-info` remain.
export const buildMppDiscoveryDocument = (
  flags: MppDiscoveryFlags,
): Record<string, unknown> => {
  const paths: Record<string, unknown> = {
    // The free, keyed Khala endpoint is always described (no payment offers, no
    // 402 advertised — it is keyed/credit-metered, not MPP-payable). This lets a
    // registry see the service surface even before the paid rail is armed.
    '/v1/chat/completions': {
      post: {
        summary: 'Khala chat completions (keyed / credit-metered)',
        description:
          'OpenAI-compatible Chat Completions. Authenticated with an OpenAgents agent key and metered receipt-first against the account balance. For pay-per-call machine payments without a key, see the MPP endpoint when armed.',
        operationId: 'chatCompletions',
        requestBody: {
          required: true,
          content: {
            'application/json': {
              schema: CHAT_COMPLETIONS_REQUEST_SCHEMA,
            },
          },
        },
        responses: {
          '200': {
            description:
              'Successful chat completion (OpenAI Chat Completions shape plus an `openagents` disclosure block).',
          },
        },
      },
    },
  }

  // HONESTY GATE: only advertise the payable MPP path when the endpoint is
  // actually armed. Inert => omit it entirely (no offers, no 402).
  if (flags.mppEnabled) {
    paths[MPP_PATH] = buildPaidPath(flags)
  }

  return {
    openapi: '3.1.0',
    info: {
      title: 'OpenAgents Khala Inference API',
      version: '1.0.0',
      description:
        'OpenAI-compatible LLM inference, pay-per-call. Machine-payable (MPP / x402) when the paid endpoint is armed.',
    },
    'x-service-info': {
      categories: ['ai'],
      docs: {
        homepage: ORIGIN,
        llms: `${ORIGIN}/llms.txt`,
        apiReference: `${ORIGIN}/agents.md`,
      },
    },
    paths,
  }
}

// Render the discovery document as an HTTPS, cacheable JSON response per the
// draft: `Content-Type: application/json`, `Cache-Control: public, max-age=300`.
// GET (and HEAD) only; anything else is 405. CORS-open so browser-based clients
// can read it cross-origin (the draft RECOMMENDS this for browser clients).
export const renderMppDiscoveryDocument = (
  request: Request,
  flags: MppDiscoveryFlags,
): Effect.Effect<Response> =>
  Effect.sync(() => {
    if (request.method !== 'GET' && request.method !== 'HEAD') {
      return new Response(JSON.stringify({ error: 'method_not_allowed' }), {
        headers: new Headers({
          allow: 'GET, HEAD',
          'cache-control': 'no-store',
          'content-type': 'application/json',
        }),
        status: 405,
      })
    }
    const body = JSON.stringify(buildMppDiscoveryDocument(flags))
    const headers = new Headers({
      'access-control-allow-origin': '*',
      'cache-control': 'public, max-age=300',
      'content-type': 'application/json',
    })
    return new Response(request.method === 'HEAD' ? null : body, {
      headers,
      status: 200,
    })
  })
