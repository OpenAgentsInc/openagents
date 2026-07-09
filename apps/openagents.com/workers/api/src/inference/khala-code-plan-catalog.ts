// Frozen Khala Code plan compatibility catalog (promise
// khala_code.free_paid_plans.v1).
//
// WHY THIS EXISTS. The legacy stable plan IDs, historical entitlement reads,
// and public-safe receipt links remain dereferenceable after the Khala Code app
// retirement. This catalog is a tombstone/compatibility projection only. It
// cannot be armed and is not authority for a future OpenAgents plan.
//
// HONEST AND ACCURATE TO THE CODE. Every clause maps to a real seam:
//   - "paid = capture opt-out" -> inference-privacy-entitlement.ts
//     (`captureDefault = freeTier.free && !paidPrivacy`, fail-closed-to-
//     private) and the paid-privacy entitlement receipt machinery in
//     inference-privacy-receipt-routes.ts.
//   - "paid plan is retired and NOT purchasable" -> the production feature
//     flag reader always returns false. Historical purchase/entitlement code
//     remains only so old receipts and status reads retain meaning.
//   - "free-plan desktop capture is NOT live" -> the Khala Code desktop
//     default path is Codex wrapper mode whose raw events / ATIF traces are
//     owner-private delegation observability, never free-plan capture; the
//     consented desktop capture pipeline is its own planned promise
//     (khala_code.free_plan_trace_capture.v1). Hosted-API free-tier capture
//     terms live in free-tier-data-sharing-disclosure.ts.
//
// This module ships NO purchase behavior, NO capture behavior, and NO
// authority. It is catalog text + bounded policy facts only.

// The stable promise id this catalog is tracked under in docs/promises/ and
// the product-promise registry (apps/openagents.com/workers/api/src/
// product-promises.ts). Reports about the plan structure route through it.
export const KHALA_CODE_PLAN_PROMISE_ID =
  'khala_code.free_paid_plans.v1' as const

export const KHALA_CODE_PLAN_CATALOG_SCHEMA_VERSION =
  'openagents.khala_code.plan_catalog.v1' as const

// Catalog version. Bump when plan terms change (not on unrelated copy edits).
export const KHALA_CODE_PLAN_CATALOG_VERSION = '2026-07-09.1' as const

export const KHALA_CODE_FREE_PLAN_ID = 'khala_code.plan.free.v1' as const
export const KHALA_CODE_PAID_PLAN_ID = 'khala_code.plan.paid.v1' as const

// Historical feature-flag key retained for compatibility. The retired product
// cannot be rearmed: isKhalaCodePaidPlansEnabled always returns false.
export const KHALA_CODE_PAID_PLANS_ENABLED_ENV_KEY =
  'KHALA_CODE_PAID_PLANS_ENABLED' as const

export const KHALA_CODE_PLAN_PURCHASE_ROUTE =
  '/v1/khala-code/plans/purchases' as const

import type { PublicProjectionStalenessContract } from '../public-projection-staleness'

// Fail closed permanently after the 2026-07-09 product retirement. Keep the
// argument so production call sites do not accidentally grow a second path.
export const isKhalaCodePaidPlansEnabled = (_value: unknown): boolean => false

export const KHALA_CODE_PLAN_CATALOG_SUMMARY: string =
  'Retired Khala Code plan catalog. The former Free (pay with data) and Paid ' +
  '(private data) plan IDs are preserved only for historical compatibility, ' +
  'entitlement reads, and receipt integrity. No Khala Code plan is purchasable, ' +
  'and KHALA_CODE_PAID_PLANS_ENABLED cannot rearm the withdrawn product. A ' +
  'future OpenAgents plan requires a new promise and authority path.'

export type KhalaCodePlanKind = 'free' | 'paid'

export type KhalaCodePlan = Readonly<{
  planId: string
  kind: KhalaCodePlanKind
  label: string
  tagline: string
  priceLabel: string
  isDefault: boolean
  captureExcluded: boolean
  terms: ReadonlyArray<string>
  purchase?: Readonly<{
    armed: boolean
    envFlag: typeof KHALA_CODE_PAID_PLANS_ENABLED_ENV_KEY
    route: typeof KHALA_CODE_PLAN_PURCHASE_ROUTE
  }>
}>

export type KhalaCodePlanCatalog = Readonly<{
  schemaVersion: typeof KHALA_CODE_PLAN_CATALOG_SCHEMA_VERSION
  catalogVersion: typeof KHALA_CODE_PLAN_CATALOG_VERSION
  promiseId: typeof KHALA_CODE_PLAN_PROMISE_ID
  summary: string
  plans: ReadonlyArray<KhalaCodePlan>
  blockerRefs: ReadonlyArray<string>
  authorityBoundary: string
  relatedPromiseIds: ReadonlyArray<string>
  // Public-projection freshness contract: the catalog is static text plus one
  // deployment-config input (the fail-closed purchase-flag read), recomputed
  // on every read (live_at_read). Both are supplied by the serving route.
  generatedAt: string
  staleness: PublicProjectionStalenessContract
}>

const FREE_PLAN_TERMS: ReadonlyArray<string> = [
  'Historical compatibility only: the former free plan cannot be selected or activated.',
  'Pay-with-data was design intent, not live behavior: deprecated Khala Code desktop coding sessions are NOT captured for training today; this catalog creates no capture authority.',
  'The Khala Code default path is Codex wrapper mode; its raw events and ATIF traces are owner-private delegation observability, never free-plan capture.',
  'The consented desktop capture pipeline (disclosure, consent, plan-scoped wiring, scrubbing) is its own planned promise: khala_code.free_plan_trace_capture.v1.',
  'Hosted Khala API free-tier capture terms are separate and disclosed at GET /api/public/free-tier-data-sharing (data.free_tier_capture_disclosure.v1).',
  'No payout is granted by capture: the data-market reward marker is inert and owner-gated.',
]

const PAID_PLAN_TERMS: ReadonlyArray<string> = [
  'Private data: the paid plan’s substance is the capture opt-out — a paid-privacy entitlement excludes the account from capture, fail-closed (when in doubt, not captured).',
  'The entitlement machinery is live on the hosted API (inference_privacy_entitlements, privacy.khala_paid_capture_optout.v1); the Khala Code plan purchase seam reuses it rather than duplicating truth.',
  'RETIRED and not purchasable: KHALA_CODE_PAID_PLANS_ENABLED is ignored and production purchase requests fail closed.',
  'Historical entitlement and privacy-receipt reads remain stable; they do not authorize a new payment or app plan.',
  'A future OpenAgents plan needs a new owner-approved promise, pricing, consent/privacy, API authority, and receipt chain.',
]

// Build the frozen catalog. `paidPlanPurchaseArmed` is retained in the input
// shape for compatibility but intentionally ignored.
export const khalaCodePlanCatalog = (
  options: Readonly<{
    generatedAt: string
    paidPlanPurchaseArmed: boolean
    staleness: PublicProjectionStalenessContract
  }>,
): KhalaCodePlanCatalog => ({
  generatedAt: options.generatedAt,
  staleness: options.staleness,
  schemaVersion: KHALA_CODE_PLAN_CATALOG_SCHEMA_VERSION,
  catalogVersion: KHALA_CODE_PLAN_CATALOG_VERSION,
  promiseId: KHALA_CODE_PLAN_PROMISE_ID,
  summary: KHALA_CODE_PLAN_CATALOG_SUMMARY,
  plans: [
    {
      planId: KHALA_CODE_FREE_PLAN_ID,
      kind: 'free',
      label: 'Free',
      tagline: 'Pay with data',
      priceLabel: 'Free',
      isDefault: true,
      captureExcluded: false,
      terms: FREE_PLAN_TERMS,
    },
    {
      planId: KHALA_CODE_PAID_PLAN_ID,
      kind: 'paid',
      label: 'Paid',
      tagline: 'Private data',
      priceLabel: 'Retired — not purchasable',
      isDefault: false,
      captureExcluded: true,
      terms: PAID_PLAN_TERMS,
      purchase: {
        armed: false,
        envFlag: KHALA_CODE_PAID_PLANS_ENABLED_ENV_KEY,
        route: KHALA_CODE_PLAN_PURCHASE_ROUTE,
      },
    },
  ],
  blockerRefs: [
    'blocker.product_promises.khala_code_product_retired',
    'blocker.product_promises.khala_code_paid_plan_not_purchasable',
  ],
  authorityBoundary:
    'Catalog text only. This projection grants no capture, billing, payout, ' +
    'or settlement authority; capture behavior stays governed by the ' +
    'disclosure and privacy-entitlement records, and no plan is presented ' +
    'as purchasable. The legacy env flag cannot rearm the withdrawn product.',
  relatedPromiseIds: [
    KHALA_CODE_PLAN_PROMISE_ID,
    'khala_code.free_plan_trace_capture.v1',
    'privacy.khala_paid_capture_optout.v1',
    'data.free_tier_capture_disclosure.v1',
  ],
})
