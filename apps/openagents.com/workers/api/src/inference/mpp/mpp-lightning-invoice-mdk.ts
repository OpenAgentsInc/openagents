// MDK-backed BOLT11 invoice issuer for the Lightning MPP rail (EPIC #6049).
//
// Mints a fresh BOLT11 invoice for N sats by POSTing a `create_checkout`
// (mode amount, currency SAT) to the SAME MDK route/sidecar the Forum L402 flow
// uses, then reads the RAW `invoice.bolt11` + `invoice.paymentHash` from the
// provider checkout. This is a server-internal issuer (the invoice + paymentHash
// are PUBLIC — they go into the 402 challenge the client must pay); it does NOT
// use the redacting public projection in `hosted-mdk-client.ts`.
//
// INERT NOTE: this issuer is constructed ONLY when an MDK route is configured
// (route URL + secret) and the Lightning flag is on. With no route the route
// wiring passes `mintLightningInvoice: undefined` and the rail is never offered.

import { Duration, Effect } from 'effect'

import { isRecord, nestedUnknown, optionalString } from '../../json-boundary'
import {
  type LightningInvoice,
  type MintLightningInvoice,
  LightningInvoiceError,
  validateLightningInvoice,
} from './mpp-lightning-invoice'

// Hard upper bound on the MDK sidecar `create_checkout` mint round-trip. The MDK
// route is the `self_hosted_mdkd_sidecar` Cloudflare Container, which sleeps
// after 30m of inactivity (`MdkSidecarContainer.sleepAfter = '30m'`). A cold
// container boot blocks `getContainer(...).fetch()` for SECONDS — and
// `Effect.tryPromise` only catches THROWS, never a hang. Without this bound a
// cold/slow mint would block the whole 402 handler (the observed prod hang:
// arming the rail timed out crypto+card too because the Lightning leg runs in
// the same Effect). With it, a slow mint resolves to a typed `provider_unavailable`
// failure and the Lightning rail is silently dropped (honesty gate), while the
// other rails return promptly. 2s is well under any client/edge timeout budget.
export const MDK_LIGHTNING_MINT_TIMEOUT_MS = 2_000

// A POST transport to the MDK route/sidecar. Returns the HTTP ok flag + status +
// parsed JSON payload (no throwing — the issuer maps a non-ok status to a typed
// failure). In production this is the same `fetchMdkSidecarRequest` path the
// hosted MDK route client uses for the `self_hosted_mdkd_sidecar` route kind.
export type MdkRoutePost = (
  body: Readonly<Record<string, unknown>>,
) => Promise<Readonly<{ ok: boolean; status: number; payload: unknown }>>

// Pull the provider checkout object out of the (possibly nested) route payload.
// Mirrors `providerCheckoutFromPayload` in hosted-mdk-client.ts.
const providerCheckout = (
  payload: unknown,
): Record<string, unknown> | undefined => {
  if (!isRecord(payload)) {
    return undefined
  }
  const nested = nestedUnknown(payload, ['data', 'checkout'])
  if (isRecord(nested)) {
    return nested
  }
  const data = nestedUnknown(payload, ['data'])
  if (isRecord(data)) {
    return data
  }
  const json = nestedUnknown(payload, ['json'])
  if (isRecord(json)) {
    return json
  }
  return payload
}

// Read the raw BOLT11 invoice string from the provider checkout.
const rawBolt11 = (checkout: Record<string, unknown>): unknown =>
  nestedUnknown(checkout, ['invoice', 'invoice']) ??
  nestedUnknown(checkout, ['invoice', 'bolt11']) ??
  nestedUnknown(checkout, ['invoice', 'paymentRequest']) ??
  checkout.bolt11 ??
  checkout.bolt11Invoice ??
  checkout.invoice ??
  checkout.paymentRequest

// Read the raw payment hash (lowercase hex) from the provider checkout.
const rawPaymentHash = (checkout: Record<string, unknown>): unknown =>
  nestedUnknown(checkout, ['invoice', 'paymentHash']) ??
  nestedUnknown(checkout, ['invoice', 'payment_hash']) ??
  checkout.paymentHash ??
  checkout.payment_hash

// Read the raw invoice expiry (RFC 3339) from the provider checkout.
const rawInvoiceExpiry = (
  checkout: Record<string, unknown>,
): string | undefined =>
  optionalString(nestedUnknown(checkout, ['invoice', 'expiresAt'])) ??
  optionalString(nestedUnknown(checkout, ['invoice', 'expiry'])) ??
  optionalString(checkout.expiresAt)

// Build a `MintLightningInvoice` from a `create_checkout` POST transport. The
// checkout is created in `amount` mode with `currency: 'SAT'` for the requested
// sats; the issuer reads the RAW bolt11 + paymentHash and surface-validates them
// (fail-closed). The preimage is NEVER touched here.
// Mint timeout for when MDK runs as the FALLBACK leg behind the Spark primary
// (EPIC #6049). The fallback only runs if Spark fails FAST (e.g. a non-ok status
// or a thrown transport error), so it does not normally stack on top of the full
// Spark budget. It is bounded so a slow/cold MDK sidecar still cannot hang the
// endpoint: a fallback that exceeds this budget (or the route's outer per-rail
// guard `LIGHTNING_LEG_GUARD_MS`, #6149) just DROPS the Lightning rail. Raised to
// 3s alongside the Spark primary budget so a real warm MDK mint can also surface
// Lightning; the zero-latency fix remains a pre-minted invoice pool (future
// optimization, tracked on #6049). Used by the selector when both issuers are
// present.
export const MDK_LIGHTNING_FALLBACK_MINT_TIMEOUT_MS = 3_000

export const makeMdkLightningInvoiceIssuer = (
  post: MdkRoutePost,
  timeoutMs: number = MDK_LIGHTNING_MINT_TIMEOUT_MS,
): MintLightningInvoice => input =>
  Effect.gen(function* () {
    const amountSats = Math.max(1, Math.trunc(input.amountSats))
    const result = yield* Effect.tryPromise({
      catch: () => new LightningInvoiceError('provider_unavailable'),
      try: () =>
        post({
          handler: 'create_checkout',
          params: {
            amount: amountSats,
            currency: 'SAT',
            metadata: { correlation_ref: input.correlationRef },
            title: 'openagents_khala_mpp_lightning',
            type: 'AMOUNT',
          },
        }),
    }).pipe(
      // Bounded mint: a cold/slow MDK sidecar container can block for seconds.
      // `Effect.tryPromise` catches throws, NOT a hang — so we cap the wall-clock
      // here. On timeout the in-flight effect is interrupted and we fail with the
      // SAME typed `provider_unavailable` reason a transport error produces, so
      // the caller drops the Lightning rail and serves the others promptly.
      Effect.timeoutOrElse({
        duration: Duration.millis(timeoutMs),
        orElse: () =>
          Effect.fail(new LightningInvoiceError('provider_unavailable')),
      }),
    )
    if (!result.ok) {
      return yield* Effect.fail(
        new LightningInvoiceError(
          result.status >= 500 ? 'provider_unavailable' : 'provider_rejected',
        ),
      )
    }

    const checkout = providerCheckout(result.payload)
    if (checkout === undefined) {
      return yield* Effect.fail(
        new LightningInvoiceError('provider_rejected'),
      )
    }

    const invoice: LightningInvoice | undefined = validateLightningInvoice({
      bolt11: rawBolt11(checkout),
      ...(rawInvoiceExpiry(checkout) === undefined
        ? {}
        : { invoiceExpiresAt: rawInvoiceExpiry(checkout) }),
      paymentHash: rawPaymentHash(checkout),
    })
    if (invoice === undefined) {
      return yield* Effect.fail(
        new LightningInvoiceError('malformed_invoice'),
      )
    }
    return invoice
  })
