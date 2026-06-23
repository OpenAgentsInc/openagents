// MPP / Payment Auth wire protocol — 402 challenge construction + credential
// parsing + STATELESS HMAC verification (EPIC #6049, defect B).
//
// The mechanism (draft-httpauth-payment-00, the authoritative Payment Auth core
// spec, mirrored at docs/reference/mpp/paymentauth/specs/core/):
//   - A request with NO payment credential gets `402 Payment Required` with one
//     `WWW-Authenticate: Payment ...` header per accepted method, and an
//     `application/problem+json` body
//     (`type: https://paymentauth.org/problems/payment-required`).
//   - Each challenge carries `id`, `realm`, `method`, `intent`, `request`
//     (base64url JCS JSON of the method request), `expires`, and `opaque`
//     (base64url JCS JSON of our server correlation data). The `id` is the
//     HMAC-SHA256 binding over those fields (see mpp-canonical.ts §5.1.3), so the
//     server verifies the retry statelessly — no per-challenge storage.
//   - The client pays, then RETRIES with `Authorization: Payment <base64url>`
//     where the decoded JSON is `{ challenge, payload, source? }`. `challenge`
//     ECHOES the issued challenge; `payload` carries the method proof (the SPT
//     for card; for our crypto rail the deposit reference lives in `opaque`).
//   - The server: recomputes the HMAC id from the echoed challenge fields and
//     constant-time compares; checks expiry; checks the request binds to what we
//     serve (amount/currency); recovers correlation from `opaque`; settles.
//
// This module owns the wire shape + the stateless verification of the binding.
// Settlement (Stripe REST) and the Khala completion live in the endpoint handler.

import { currentEpochMillis } from '../../runtime-primitives'
import {
  type ChallengeBindingSlots,
  computeChallengeId,
  constantTimeEqual,
  decodeJcsBase64UrlRecord,
  jcsBase64Url,
} from './mpp-canonical'

// ---- Challenge model ----

// The method `request` JSON (base64url JCS in the challenge). Mirrors the
// charge-intent request schema: amount is a STRING in minor units per the
// stripe/usdc charge specs; currency is the ISO/asset code. recipient + network
// describe the crypto deposit target.
export type MppRequestParams = Readonly<{
  amount: string
  currency: string
  description?: string | undefined
  recipient?: string | undefined
  network?: string | undefined
  // methodDetails.networkId carries our Stripe Business Network Profile id for
  // the stripe/SPT method (the `profile_…`).
  methodDetails?: Record<string, unknown> | undefined
}>

// Server correlation data carried in `opaque` (base64url JCS). Flat string map
// per the core spec (§5.1.2). We stash the crypto deposit PaymentIntent id +
// amount + network so we can recover settlement state statelessly on retry.
export type MppOpaque = Readonly<{
  // The Stripe crypto deposit PaymentIntent id backing a crypto challenge.
  pi?: string | undefined
  amount?: string | undefined
  network?: string | undefined
  model?: string | undefined
}>

// One accepted payment method on the 402 challenge (server-side shape before
// HMAC id computation; the id is filled by buildChallengeHeader).
export type MppChallenge = Readonly<{
  id: string
  realm: string
  // Method name: a crypto network ('base'/'solana'/'tempo') or 'stripe' (SPT).
  method: string
  intent: 'charge'
  // Base64url JCS of MppRequestParams.
  request: string
  // RFC 3339 expiry.
  expires: string
  // Base64url JCS of MppOpaque.
  opaque?: string | undefined
  // Decoded convenience mirrors for the inline problem body (NOT in the HMAC).
  amountCents: number
  currency: string
  network?: string | undefined
  recipient?: string | undefined
  // The Stripe PaymentIntent id (crypto rail) recovered from opaque.
  paymentIntentId?: string | undefined
}>

// The problem+json body of a 402 response (RFC 9457 + paymentauth.org type).
export type PaymentRequiredProblem = Readonly<{
  type: string
  title: string
  status: 402
  detail: string
  challengeId: string
  // Structured echo so a simple client that does not parse WWW-Authenticate can
  // still act on the price + deposit address.
  challenges: ReadonlyArray<
    Readonly<{
      id: string
      method: string
      intent: 'charge'
      amountCents: number
      currency: string
      network?: string | undefined
      recipient?: string | undefined
      paymentIntentId?: string | undefined
    }>
  >
}>

const PAYMENT_REQUIRED_TYPE =
  'https://paymentauth.org/problems/payment-required'

// Build a challenge: encode the method request + opaque to base64url JCS, then
// compute the HMAC id over the binding slots. Async because the HMAC uses
// WebCrypto. Returns the fully-formed challenge with its `id` populated.
export const buildChallenge = async (
  serverSecret: string,
  input: Readonly<{
    realm: string
    method: string
    request: MppRequestParams
    expires: string
    opaque?: MppOpaque | undefined
    // Decoded mirrors for the inline problem body.
    amountCents: number
    currency: string
    network?: string | undefined
    recipient?: string | undefined
    paymentIntentId?: string | undefined
  }>,
): Promise<MppChallenge> => {
  const requestB64 = jcsBase64Url(input.request)
  const opaqueB64 =
    input.opaque === undefined ? undefined : jcsBase64Url(input.opaque)
  const slots: ChallengeBindingSlots = {
    digest: '',
    expires: input.expires,
    intent: 'charge',
    method: input.method,
    opaqueB64Url: opaqueB64 ?? '',
    realm: input.realm,
    requestB64Url: requestB64,
  }
  const id = await computeChallengeId(serverSecret, slots)
  return {
    amountCents: input.amountCents,
    currency: input.currency,
    expires: input.expires,
    id,
    intent: 'charge',
    method: input.method,
    network: input.network,
    opaque: opaqueB64,
    paymentIntentId: input.paymentIntentId,
    realm: input.realm,
    recipient: input.recipient,
    request: requestB64,
  }
}

// Render the `WWW-Authenticate: Payment ...` header for one challenge. Emits the
// spec parameters (id, realm, method, intent, request, expires, opaque).
export const renderChallengeHeader = (challenge: MppChallenge): string => {
  const params: Array<[string, string]> = [
    ['id', challenge.id],
    ['realm', challenge.realm],
    ['method', challenge.method],
    ['intent', challenge.intent],
    ['request', challenge.request],
    ['expires', challenge.expires],
  ]
  if (challenge.opaque !== undefined) {
    params.push(['opaque', challenge.opaque])
  }
  const rendered = params
    .map(([k, v]) => `${k}="${v.replace(/"/g, '\\"')}"`)
    .join(', ')
  return `Payment ${rendered}`
}

// Build the full set of 402 response headers (one WWW-Authenticate per method +
// problem content type + no-store, per the core spec caching rules).
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
  detail = 'Payment is required to use this endpoint.',
): PaymentRequiredProblem => ({
  challengeId: challenges[0]?.id ?? '',
  challenges: challenges.map(c => ({
    amountCents: c.amountCents,
    currency: c.currency,
    id: c.id,
    intent: c.intent,
    method: c.method,
    network: c.network,
    paymentIntentId: c.paymentIntentId,
    recipient: c.recipient,
  })),
  detail,
  status: 402,
  title: 'Payment Required',
  type: PAYMENT_REQUIRED_TYPE,
})

// ---- Credential parsing + verification ----

// The echoed challenge object inside a credential (core spec §6.2).
export type CredentialChallenge = Readonly<{
  id: string
  realm: string
  method: string
  intent: string
  request: string
  expires?: string | undefined
  opaque?: string | undefined
  digest?: string | undefined
}>

// A decoded inbound payment credential from `Authorization: Payment <b64url>`.
export type ParsedPaymentCredential = Readonly<{
  scheme: 'Payment'
  challenge: CredentialChallenge
  payload: Record<string, unknown>
  source?: string | undefined
  raw: string
}>

const readString = (
  record: Record<string, unknown>,
  key: string,
): string | undefined => {
  const value = record[key]
  return typeof value === 'string' ? value : undefined
}

// Parse the `Authorization` header. Returns undefined for a missing/non-Payment
// scheme (the endpoint then issues a fresh 402). The Payment value is a
// base64url-nopad JSON object `{ challenge, payload, source? }`. We require the
// echoed `challenge` (with at least id/realm/method/intent/request) and a
// `payload` object; anything malformed returns undefined (fail-closed →
// malformed-credential 402).
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
  const raw = trimmed.replace(/^Payment\s+/i, '').trim()
  if (raw === '') {
    return undefined
  }

  const decoded = base64UrlDecodeRecord(raw)
  if (decoded === undefined) {
    return undefined
  }

  const challengeRecord = decoded.challenge
  if (
    challengeRecord === null ||
    typeof challengeRecord !== 'object' ||
    Array.isArray(challengeRecord)
  ) {
    return undefined
  }
  const challenge = challengeRecord as Record<string, unknown>
  const id = readString(challenge, 'id')
  const realm = readString(challenge, 'realm')
  const method = readString(challenge, 'method')
  const intent = readString(challenge, 'intent')
  const request = readString(challenge, 'request')
  if (
    id === undefined ||
    realm === undefined ||
    method === undefined ||
    intent === undefined ||
    request === undefined
  ) {
    return undefined
  }

  const payloadRecord = decoded.payload
  const payload =
    payloadRecord !== null &&
    typeof payloadRecord === 'object' &&
    !Array.isArray(payloadRecord)
      ? (payloadRecord as Record<string, unknown>)
      : undefined
  if (payload === undefined) {
    return undefined
  }

  return {
    challenge: {
      digest: readString(challenge, 'digest'),
      expires: readString(challenge, 'expires'),
      id,
      intent,
      method,
      opaque: readString(challenge, 'opaque'),
      realm,
      request,
    },
    payload,
    raw,
    scheme: 'Payment',
    source: readString(decoded, 'source'),
  }
}

// Decode a base64url-nopad string to a JSON record (top-level credential).
const base64UrlDecodeRecord = (
  value: string,
): Record<string, unknown> | undefined => decodeJcsBase64UrlRecord(value)

// ---- Stateless verification result ----

export type CredentialVerifyFailure =
  | 'invalid-challenge' // HMAC id mismatch / unknown binding
  | 'expired' // past `expires`
  | 'request-mismatch' // amount/currency does not bind to what we serve
  | 'method-mismatch' // method not one we offered

export type CredentialVerifyResult =
  | Readonly<{
      ok: true
      method: string
      // Decoded method request params (amount/currency/etc).
      request: MppRequestParams
      // Decoded server correlation data (crypto: pi/amount/network).
      opaque: MppOpaque | undefined
    }>
  | Readonly<{ ok: false; reason: CredentialVerifyFailure }>

// Recompute the HMAC id over the echoed challenge fields and verify the binding
// statelessly, FAIL-CLOSED. Then check expiry and that the request binds to what
// we serve (currency + amount). This is the load-bearing verification: a tampered
// challenge, a forged id, an expired challenge, or a request whose amount/currency
// no longer matches our quote all reject here. Settlement happens AFTER this.
export const verifyCredential = async (
  serverSecret: string,
  credential: ParsedPaymentCredential,
  expectations: Readonly<{
    realm: string
    // The methods we offered (crypto networks + optionally 'stripe').
    allowedMethods: ReadonlyArray<string>
    // The currency we expect for the credential's method.
    expectedCurrencyForMethod: (method: string) => string
    // The minimum amount in minor units we expect (>= floors a downward tamper).
    expectedMinAmountCents: number
    nowMs?: number | undefined
  }>,
): Promise<CredentialVerifyResult> => {
  const echoed = credential.challenge

  // (d-pre) method must be one we offered.
  if (!expectations.allowedMethods.includes(echoed.method)) {
    return { ok: false, reason: 'method-mismatch' }
  }

  // (a) recompute the HMAC id from the echoed challenge fields + server secret
  // and constant-time compare. The realm in the echo must also match ours.
  if (echoed.realm !== expectations.realm) {
    return { ok: false, reason: 'invalid-challenge' }
  }
  const slots: ChallengeBindingSlots = {
    digest: echoed.digest ?? '',
    expires: echoed.expires ?? '',
    intent: echoed.intent,
    method: echoed.method,
    opaqueB64Url: echoed.opaque ?? '',
    realm: echoed.realm,
    requestB64Url: echoed.request,
  }
  const expectedId = await computeChallengeId(serverSecret, slots)
  if (!constantTimeEqual(expectedId, echoed.id)) {
    return { ok: false, reason: 'invalid-challenge' }
  }

  // (b) not expired.
  if (echoed.expires !== undefined && echoed.expires !== '') {
    const expiresMs = Date.parse(echoed.expires)
    const nowMs = expectations.nowMs ?? currentEpochMillis()
    if (Number.isNaN(expiresMs) || expiresMs <= nowMs) {
      return { ok: false, reason: 'expired' }
    }
  }

  // (c) the request params bind to what we're serving (amount/currency). Decode
  // the JCS request and check currency + a non-decreasing amount. Because the id
  // already covered `request`, this is a defense-in-depth read; a mismatch means
  // we would be serving for a different quote than the bound one.
  const requestRecord = decodeJcsBase64UrlRecord(echoed.request)
  if (requestRecord === undefined) {
    return { ok: false, reason: 'invalid-challenge' }
  }
  const amount = readString(requestRecord, 'amount')
  const currency = readString(requestRecord, 'currency')
  if (amount === undefined || currency === undefined) {
    return { ok: false, reason: 'request-mismatch' }
  }
  const amountCents = Number(amount)
  if (
    !Number.isFinite(amountCents) ||
    amountCents < expectations.expectedMinAmountCents
  ) {
    return { ok: false, reason: 'request-mismatch' }
  }
  if (
    currency.toLowerCase() !==
    expectations.expectedCurrencyForMethod(echoed.method).toLowerCase()
  ) {
    return { ok: false, reason: 'request-mismatch' }
  }

  const request: MppRequestParams = {
    amount,
    currency,
    description: readString(requestRecord, 'description'),
    methodDetails:
      requestRecord.methodDetails !== null &&
      typeof requestRecord.methodDetails === 'object' &&
      !Array.isArray(requestRecord.methodDetails)
        ? (requestRecord.methodDetails as Record<string, unknown>)
        : undefined,
    network: readString(requestRecord, 'network'),
    recipient: readString(requestRecord, 'recipient'),
  }

  const opaque =
    echoed.opaque === undefined
      ? undefined
      : decodeOpaque(echoed.opaque)

  return { method: echoed.method, ok: true, opaque, request }
}

// ---- Payment-Receipt (core spec §6.3) ----

// Build the base64url-nopad `Payment-Receipt` header value for a settled
// payment. Issued ONLY on success (2xx); never on error responses.
export const buildPaymentReceipt = (
  input: Readonly<{
    method: string
    reference: string
    timestamp: string
  }>,
): string =>
  jcsBase64Url({
    method: input.method,
    reference: input.reference,
    status: 'success',
    timestamp: input.timestamp,
  })

// Decode the `opaque` correlation record (base64url JCS) into MppOpaque.
const decodeOpaque = (opaqueB64: string): MppOpaque | undefined => {
  const record = decodeJcsBase64UrlRecord(opaqueB64)
  if (record === undefined) {
    return undefined
  }
  return {
    amount: readString(record, 'amount'),
    model: readString(record, 'model'),
    network: readString(record, 'network'),
    pi: readString(record, 'pi'),
  }
}
