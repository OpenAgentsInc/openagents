// Spark-backed BOLT11 invoice issuer for the Lightning MPP rail (EPIC #6049).
//
// PRIMARY RAIL. Owner directive: Spark is the primary rail for all agent/MPP
// payments — it supports OFFLINE RECEIVES, so the payer can settle the 402
// Lightning charge even when our receiving wallet is not actively listening.
// MDK remains an explicit FALLBACK Lightning issuer only (see
// `mpp-lightning-invoice-mdk.ts` + `lightningInvoiceIssuerForEnv`).
//
// Mints a fresh BOLT11 invoice for N sats by POSTing `/spark/funding-invoice`
// to the SAME `MDK_TREASURY` container the Spark payout/balance paths already
// reach (`fetchMdkTreasuryPath` → the `@breeztech/breez-sdk-spark` SDK). The
// container returns the RAW `bolt11Invoice` + (decoded) `paymentHash`; both are
// PUBLIC (they go into the 402 challenge the client pays). The PREIMAGE is never
// touched here — settlement is verified LOCALLY in the Worker as
// sha256(preimage) === paymentHash (draft-lightning-charge-00), and offline
// receipt is confirmable via the container's `GET /spark/received/:paymentHash`.
//
// INERT NOTE: this issuer is constructed ONLY when a Spark treasury route is
// reachable (the `MDK_TREASURY` container binding present) AND the Lightning flag
// is on. With no route the selector falls back to MDK; with neither the rail is
// never offered (honesty gate).

import { Duration, Effect } from 'effect'

import { isRecord, optionalString } from '../../json-boundary'
import {
  type LightningInvoice,
  type MintLightningInvoice,
  LightningInvoiceError,
  validateLightningInvoice,
} from './mpp-lightning-invoice'

// Hard upper bound on the Spark `/spark/funding-invoice` mint round-trip. The
// Spark treasury is the `MDK_TREASURY` Cloudflare Container; a cold container
// boot OR a cold breez-sdk-spark build/sync can block for seconds — and
// `Effect.tryPromise` only catches THROWS, never a hang. Without this bound a
// cold/slow mint would block the Lightning leg. With it, a slow mint resolves to
// a typed `provider_unavailable` failure so the SELECTOR can fall back to MDK and
// still finish under the route's outer per-rail guard (#6149,
// `LIGHTNING_LEG_GUARD_MS`).
//
// BUDGET (#6049): the real Spark `/spark/funding-invoice` mint takes ~1.5–3.1s
// even WARM, so the old 1.2s cap always tripped and the honesty gate dropped
// Lightning from every 402. PROD `wrangler tail` after the first raise showed the
// `MdkTreasuryContainer` mint subrequest itself returning 200 in ~3.76–3.95s
// warm — and the worker-side round-trip (DO dispatch + queue overhead on top of
// the container time) lands right AT/just over a 4s cap, so a 4s budget still
// tripped. This budget is raised to 6s to give the real warm mint genuine
// headroom and reliably surface the Lightning rail. The tradeoff is the 402 now
// takes ~4–5s when Lightning mints; the zero-latency fix is a pre-minted Spark
// invoice pool (future optimization, tracked on #6049). The per-rail isolation
// (#6149) is preserved: a mint that exceeds this budget (or the outer
// `LIGHTNING_LEG_GUARD_MS`) still only DROPS the Lightning rail — it can never
// hang the endpoint, which always returns crypto + card fast.
export const SPARK_LIGHTNING_MINT_TIMEOUT_MS = 6_000

// A POST transport to the Spark treasury route. Returns the HTTP ok flag +
// status + parsed JSON payload (no throwing — the issuer maps a non-ok status to
// a typed failure). In production this is `fetchMdkTreasuryPath(env)` pointed at
// `/spark/funding-invoice` on the `MDK_TREASURY` container.
export type SparkTreasuryFundingInvoicePost = (
  body: Readonly<Record<string, unknown>>,
) => Promise<Readonly<{ ok: boolean; status: number; payload: unknown }>>

// Read the raw BOLT11 invoice string from the Spark funding-invoice payload.
const rawBolt11 = (payload: Record<string, unknown>): unknown =>
  payload.bolt11Invoice ?? payload.bolt11 ?? payload.invoice

// Read the raw payment hash (lowercase hex) from the Spark funding-invoice
// payload. The container decodes it from the minted bolt11 `p` field.
const rawPaymentHash = (payload: Record<string, unknown>): unknown =>
  payload.paymentHash ?? payload.payment_hash

// Build a `MintLightningInvoice` from a `/spark/funding-invoice` POST transport.
// The invoice is minted for the requested sats; the issuer reads the RAW bolt11 +
// paymentHash and surface-validates them (fail-closed). The preimage is NEVER
// touched here.
export const makeSparkLightningInvoiceIssuer = (
  post: SparkTreasuryFundingInvoicePost,
  timeoutMs: number = SPARK_LIGHTNING_MINT_TIMEOUT_MS,
): MintLightningInvoice => input =>
  Effect.gen(function* () {
    const amountSat = Math.max(1, Math.trunc(input.amountSats))
    const result = yield* Effect.tryPromise({
      catch: () => new LightningInvoiceError('provider_unavailable'),
      try: () =>
        post({
          amountSat,
          description: input.description,
        }),
    }).pipe(
      // Bounded mint: a cold container / cold Spark SDK build can block for
      // seconds. `Effect.tryPromise` catches throws, NOT a hang — so cap the
      // wall-clock here. On timeout we fail with the SAME typed
      // `provider_unavailable` reason a transport error produces, so the selector
      // falls back to MDK (or, if MDK is also unavailable, drops the rail).
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

    if (!isRecord(result.payload)) {
      return yield* Effect.fail(new LightningInvoiceError('provider_rejected'))
    }
    const payload = result.payload

    const invoice: LightningInvoice | undefined = validateLightningInvoice({
      bolt11: rawBolt11(payload),
      ...(optionalString(payload.expiresAt) === undefined
        ? {}
        : { invoiceExpiresAt: optionalString(payload.expiresAt) }),
      paymentHash: rawPaymentHash(payload),
    })
    if (invoice === undefined) {
      return yield* Effect.fail(new LightningInvoiceError('malformed_invoice'))
    }
    return invoice
  })
