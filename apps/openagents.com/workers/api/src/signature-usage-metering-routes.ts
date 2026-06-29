// Public read-only metering surface for signature monetization
// (EPIC #5523 / DE-6 #5529; promise marketplace.signature_monetization.v1, red).
//
// INERT by default. The route is wired into the live Worker but reads from an
// injected metering store that the Worker leaves EMPTY unless the surface flag
// is explicitly armed (SIGNATURE_USAGE_METERING_ENABLED). Either way the
// response is honest: `inert: true`, `promiseState: 'red'`, the settlement
// blocker still open, with NO billing, fulfillment, or live-revenue claim.
// Read-only (GET only).

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  type SignatureUsageMeteringStore,
  emptySignatureUsageMeteringStore,
  projectSignatureUsageMetering,
} from './signature-usage-metering'

export const SignatureUsageMeteringEndpoint =
  '/api/public/markets/signature-monetization/metering'

// Parse the SIGNATURE_USAGE_METERING_ENABLED flag. Default OFF: anything other
// than an explicit truthy token leaves the surface inert (empty store).
export const isSignatureUsageMeteringEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

export type SignatureUsageMeteringDeps = Readonly<{
  // Whether the metering surface is armed. When false (default) the Worker
  // passes the empty store, so the projection is inert.
  enabled: boolean
  // The metering store. The Worker passes the empty store while INERT.
  store?: SignatureUsageMeteringStore
}>

const resolveStore = (
  deps: SignatureUsageMeteringDeps,
): SignatureUsageMeteringStore =>
  deps.enabled && deps.store !== undefined
    ? deps.store
    : emptySignatureUsageMeteringStore

/**
 * GET the signature usage-metering projection. Read-only, no-store JSON.
 */
export const handleSignatureUsageMeteringApi = (
  request: Request,
  deps: SignatureUsageMeteringDeps,
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.succeed(
    noStoreJsonResponse(projectSignatureUsageMetering(resolveStore(deps))),
  )
}
