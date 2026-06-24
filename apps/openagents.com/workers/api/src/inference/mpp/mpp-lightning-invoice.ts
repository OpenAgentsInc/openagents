// Lightning rail BOLT11 invoice issuer for the MPP / Payment-Auth endpoint
// (EPIC #6049, Lightning charge intent — draft-lightning-charge-00).
//
// THE FEASIBILITY: the Worker already mints BOLT11 invoices through the
// self-hosted mdkd sidecar (the `MDK_SIDECAR` Durable Object container) — the
// SAME path the Forum L402 flow uses (`fetchMdkSidecarRequest` +
// `create_checkout`). The Lightning charge protocol needs the RAW bolt11
// invoice string and RAW paymentHash to build the 402 challenge, and it
// verifies the preimage LOCALLY with WebCrypto (sha256), so no node call is
// needed at verification time. The public hosted-mdk client
// (`hosted-mdk-client.ts`) deliberately REDACTS bolt11/paymentHash for public
// projection surfaces; this module is a server-internal issuer that reads the
// raw invoice fields, which is correct: the bolt11 invoice and its paymentHash
// are PUBLIC (they go into the 402 challenge the client must pay); only the
// PREIMAGE is the bearer secret, and the preimage never touches this module.
//
// INERT NOTE: the Lightning rail only calls this when KHALA_MPP_LIGHTNING_ENABLED
// is on AND a working MDK sidecar/route is configured. With either absent the
// rail is not advertised and never issues a challenge.

import { Effect } from 'effect'

import { normalizeIsoTimestamp } from '../../runtime-primitives'

// A minted BOLT11 invoice for N sats. All fields are PUBLIC (the invoice + its
// payment hash are what the client pays). The preimage is NEVER here.
export type LightningInvoice = Readonly<{
  // The full BOLT11-encoded payment request string (e.g. "lnbc100n1...").
  bolt11: string
  // The payment hash embedded in the invoice, lowercase hex (64 chars).
  paymentHash: string
  // The Lightning network the invoice is on, per the BOLT11 prefix.
  network: 'mainnet' | 'regtest' | 'signet'
  // RFC 3339 expiry derived from the invoice's BOLT11 expiry, when the provider
  // reports it. The challenge expiry is the EARLIER of this and the challenge
  // TTL (spec §"Challenge Expiry and Invoice Expiry").
  invoiceExpiresAt?: string | undefined
}>

export type LightningInvoiceFailure =
  | 'not_configured' // no sidecar/route wired (rail should not have been offered)
  | 'provider_unavailable' // the sidecar/route call failed
  | 'provider_rejected' // the provider returned a non-invoice / malformed payload
  | 'malformed_invoice' // the returned bolt11/paymentHash did not validate

export class LightningInvoiceError extends Error {
  override readonly name = 'LightningInvoiceError'
  readonly reason: LightningInvoiceFailure
  constructor(reason: LightningInvoiceFailure) {
    super(`lightning invoice issuance failed: ${reason}`)
    this.reason = reason
  }
}

// The injectable issuer seam. The pure MPP protocol + route code depends on
// this function (mirrors the existing `fetch?: StripeFetch` seam); index.ts
// wires the real sidecar-backed implementation; tests inject a fake.
export type MintLightningInvoice = (
  input: Readonly<{
    abortSignal?: AbortSignal | undefined
    amountSats: number
    description: string
    // A public-safe correlation ref (the model + a request nonce) for the
    // provider's invoice memo/metadata. NEVER carries secret material.
    correlationRef: string
  }>,
) => Effect.Effect<LightningInvoice, LightningInvoiceError>

// ---- BOLT11 surface validation (no full decode; we trust the issuer) ----

// Lowercase-hex 32-byte payment hash.
const PAYMENT_HASH_PATTERN = /^[0-9a-f]{64}$/
// BOLT11 human-readable prefix + bech32 body. We only need the prefix to read
// the network and a sanity check on the body; we never decode the amount here
// (the provider issued it for the amount we requested; the client decodes and
// verifies the invoice independently before paying, per the spec).
const BOLT11_PATTERN = /^(lnbc|lntb|lntbs|lnbcrt)[0-9a-z]+$/i

// Read the Lightning network from the BOLT11 human-readable prefix.
//   lnbc  -> mainnet
//   lntb  -> signet (testnet HRP; treated as signet for our networks set)
//   lntbs -> signet
//   lnbcrt-> regtest
export const networkFromBolt11 = (
  bolt11: string,
): LightningInvoice['network'] | undefined => {
  const lower = bolt11.toLowerCase()
  if (lower.startsWith('lnbcrt')) {
    return 'regtest'
  }
  if (lower.startsWith('lntbs') || lower.startsWith('lntb')) {
    return 'signet'
  }
  if (lower.startsWith('lnbc')) {
    return 'mainnet'
  }
  return undefined
}

// Validate a freshly-issued invoice's surface (shape only). Returns the typed
// invoice or undefined when malformed (the caller fails closed).
export const validateLightningInvoice = (
  input: Readonly<{
    bolt11: unknown
    paymentHash: unknown
    invoiceExpiresAt?: unknown
  }>,
): LightningInvoice | undefined => {
  if (
    typeof input.bolt11 !== 'string' ||
    !BOLT11_PATTERN.test(input.bolt11.trim())
  ) {
    return undefined
  }
  const bolt11 = input.bolt11.trim()
  const paymentHash =
    typeof input.paymentHash === 'string'
      ? input.paymentHash.trim().toLowerCase()
      : undefined
  if (paymentHash === undefined || !PAYMENT_HASH_PATTERN.test(paymentHash)) {
    return undefined
  }
  const network = networkFromBolt11(bolt11)
  if (network === undefined) {
    return undefined
  }
  const invoiceExpiresAt =
    typeof input.invoiceExpiresAt === 'string' &&
    !Number.isNaN(Date.parse(input.invoiceExpiresAt))
      ? normalizeIsoTimestamp(input.invoiceExpiresAt)
      : undefined
  return {
    bolt11,
    network,
    paymentHash,
    ...(invoiceExpiresAt === undefined ? {} : { invoiceExpiresAt }),
  }
}

// Compose a PRIMARY issuer with a FALLBACK issuer (EPIC #6049, owner directive:
// Spark primary, MDK fallback). Tries `primary` first; on ANY typed
// `LightningInvoiceError` (including its bounded mint timeout, which surfaces as
// `provider_unavailable`), tries `fallback`. The combined issuer is itself the
// SAME `MintLightningInvoice` seam the route already drops on failure — so if
// BOTH fail it fails with the fallback's error and the route drops only the
// Lightning rail (per-rail isolation, #6149). Each leg keeps its OWN bounded mint
// timeout, and both legs are tuned (Spark + MDK) to fit under the route's outer
// per-rail guard so a primary timeout plus a fallback attempt can never hang the
// endpoint. When only one issuer is present it is returned as-is; when neither is
// present `undefined` is returned (the rail is never offered — honesty gate).
export const makeFallbackLightningInvoiceIssuer = (
  primary: MintLightningInvoice | undefined,
  fallback: MintLightningInvoice | undefined,
): MintLightningInvoice | undefined => {
  if (primary === undefined) {
    return fallback
  }
  if (fallback === undefined) {
    return primary
  }
  return input => primary(input).pipe(Effect.catch(() => fallback(input)))
}
