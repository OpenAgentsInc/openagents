// MPP / x402 wire protocol — 402 challenge construction + credential parsing
// (EPIC #6049, Phase 2). PURE (no IO, no Stripe, no env).
//
// The mechanism (Stripe machine-payments docs, authoritative):
//   - A request with NO payment credential gets `402 Payment Required` with one
//     `WWW-Authenticate: Payment ...` header per accepted method, and a
//     `application/problem+json` body
//     (`type: https://paymentauth.org/problems/payment-required`).
//   - The client reads the challenges, picks a method, pays, and RETRIES with an
//     `Authorization: Payment <token>` credential.
//   - The server parses the credential, verifies it (Stripe REST, see
//     stripe-mpp-client.ts), and serves.
//
// This module owns the wire shape only; verification + the Khala completion live
// in the endpoint handler.

// One accepted payment method on the 402 challenge.
export type MppChallenge = Readonly<{
  // Challenge id (binds the retry to this quote).
  id: string
  // Method name: 'tempo'/'solana'/'base' (crypto) or 'stripe' (card/SPT).
  method: string
  // 'charge' for a pay-per-call charge.
  intent: 'charge'
  // Price in minor units (cents) for this method.
  amountCents: number
  currency: string
  // For crypto methods: the network the deposit address lives on.
  network?: string | undefined
  // For crypto methods: the deposit address the agent pays to.
  recipient?: string | undefined
  // The Stripe PaymentIntent id backing this challenge (crypto). The retry
  // references it (or the agent's payment proof references the deposit address);
  // the server verifies settlement of this PaymentIntent.
  paymentIntentId?: string | undefined
}>

// The problem+json body of a 402 response (RFC 9457 + paymentauth.org type).
export type PaymentRequiredProblem = Readonly<{
  type: string
  title: string
  status: 402
  detail: string
  challengeId: string
  // Non-standard but useful: a structured echo of the challenges so a simple
  // client that does not parse WWW-Authenticate can still act on the price +
  // deposit address. Mirrors the directory examples that surface this inline.
  challenges: ReadonlyArray<MppChallenge>
}>

const PAYMENT_REQUIRED_TYPE =
  'https://paymentauth.org/problems/payment-required'

// Render the `WWW-Authenticate: Payment ...` header value for one challenge. The
// quoted-param shape matches the Stripe docs example:
//   WWW-Authenticate: Payment id="chal_…", method="tempo", intent="charge", …
export const renderChallengeHeader = (challenge: MppChallenge): string => {
  const params: Array<[string, string]> = [
    ['id', challenge.id],
    ['method', challenge.method],
    ['intent', challenge.intent],
    ['amount', String(challenge.amountCents)],
    ['currency', challenge.currency],
  ]
  if (challenge.network !== undefined) {
    params.push(['network', challenge.network])
  }
  if (challenge.recipient !== undefined) {
    params.push(['recipient', challenge.recipient])
  }
  if (challenge.paymentIntentId !== undefined) {
    params.push(['payment_intent', challenge.paymentIntentId])
  }
  const rendered = params
    .map(([k, v]) => `${k}="${v.replace(/"/g, '\\"')}"`)
    .join(', ')
  return `Payment ${rendered}`
}

// Build the full set of 402 response headers (one WWW-Authenticate per method +
// problem content type + no-store).
export const buildPaymentRequiredHeaders = (
  challenges: ReadonlyArray<MppChallenge>,
): Headers => {
  const headers = new Headers({
    'cache-control': 'no-store',
    'content-type': 'application/problem+json',
  })
  for (const challenge of challenges) {
    headers.append('www-authenticate', renderChallengeHeader(challenge))
  }
  return headers
}

// Build the problem+json body for the 402.
export const buildPaymentRequiredProblem = (
  challenges: ReadonlyArray<MppChallenge>,
): PaymentRequiredProblem => ({
  challengeId: challenges[0]?.id ?? '',
  challenges,
  detail: 'Payment is required to use this endpoint.',
  status: 402,
  title: 'Payment Required',
  type: PAYMENT_REQUIRED_TYPE,
})

// A parsed inbound payment credential from `Authorization: Payment …`. The
// retry carries the challenge id it is answering and a reference the server uses
// to verify settlement — for the crypto rail, the Stripe PaymentIntent id.
export type ParsedPaymentCredential = Readonly<{
  scheme: 'Payment'
  // The challenge id this credential answers, when present.
  challengeId?: string | undefined
  method?: string | undefined
  // The Stripe PaymentIntent id to verify (crypto rail).
  paymentIntentId?: string | undefined
  // The shared payment token (card/SPT rail), when present.
  sharedPaymentToken?: string | undefined
  // The raw credential value (for a Node-sidecar verify path if needed).
  raw: string
}>

// Parse a quoted-param list like: id="chal_1", payment_intent="pi_123" → map.
const parseQuotedParams = (value: string): Record<string, string> => {
  const out: Record<string, string> = {}
  // Match key="value" (value may contain escaped quotes).
  const re = /(\w+)="((?:[^"\\]|\\.)*)"/g
  let match: RegExpExecArray | null
  while ((match = re.exec(value)) !== null) {
    out[match[1]!] = match[2]!.replace(/\\"/g, '"')
  }
  return out
}

// Parse the `Authorization` header. Returns undefined for a missing or
// non-Payment scheme (so the endpoint returns a fresh 402). The Payment value is
// either quoted params (`Payment id="…", payment_intent="…"`) or a base64url
// JSON token (the Mppx-signed shape). We extract the fields we need to verify;
// the raw value is retained for a Node-sidecar verify path.
export const parsePaymentCredential = (
  authorization: string | null,
): ParsedPaymentCredential | undefined => {
  if (authorization === null) {
    return undefined
  }
  const trimmed = authorization.trim()
  if (!/^Payment\s+/i.test(trimmed)) {
    return undefined
  }
  const raw = trimmed.replace(/^Payment\s+/i, '')

  // Quoted-param form.
  if (raw.includes('="')) {
    const params = parseQuotedParams(raw)
    return {
      challengeId: params.id,
      method: params.method,
      paymentIntentId: params.payment_intent ?? params.paymentIntent,
      raw,
      scheme: 'Payment',
      sharedPaymentToken: params.shared_payment_token ?? params.spt,
    }
  }

  // base64url JSON token form (Mppx-signed). Decode best-effort to extract the
  // settlement reference; if it does not decode, the raw value still rides
  // through for a sidecar verify.
  try {
    const normalized = raw.replace(/-/g, '+').replace(/_/g, '/')
    const decoded = atob(normalized)
    const parsed = JSON.parse(decoded) as Record<string, unknown>
    const payload = (parsed.payload ?? parsed) as Record<string, unknown>
    const pi =
      payload.payment_intent ??
      payload.paymentIntent ??
      payload.payment_intent_id
    const spt = payload.shared_payment_token ?? payload.spt
    return {
      challengeId:
        typeof parsed.challengeId === 'string' ? parsed.challengeId : undefined,
      method: typeof parsed.method === 'string' ? parsed.method : undefined,
      paymentIntentId: typeof pi === 'string' ? pi : undefined,
      raw,
      scheme: 'Payment',
      sharedPaymentToken: typeof spt === 'string' ? spt : undefined,
    }
  } catch {
    return { raw, scheme: 'Payment' }
  }
}
