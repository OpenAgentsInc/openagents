// Khala Code plan catalog (promise khala_code.free_paid_plans.v1, claim
// issue #7966).
//
// WHY THIS EXISTS. Episode 245 launches Khala Code with a two-plan structure
// on the whiteboard: Free (pay with data) and Paid (private data). The promise
// record is `planned`: no plan can be selected or purchased inside Khala Code
// today. This module is the SINGLE SOURCE OF TRUTH for the plan structure so
// the same honest, code-accurate terms render at every surface — the public
// agent-readable endpoint (GET /api/public/khala-code/plans) and the Khala
// Code desktop plan surface — instead of scattering copy that can drift past
// what the code actually does.
//
// HONEST AND ACCURATE TO THE CODE. Every clause maps to a real seam:
//   - "paid = capture opt-out" -> inference-privacy-entitlement.ts
//     (`captureDefault = freeTier.free && !paidPrivacy`, fail-closed-to-
//     private) and the paid-privacy entitlement receipt machinery in
//     inference-privacy-receipt-routes.ts.
//   - "paid plan is NOT purchasable today" -> the purchase seam
//     (khala-code-plan-routes.ts) is flag-gated by
//     KHALA_CODE_PAID_PLANS_ENABLED, DEFAULT OFF, fail-closed. When armed it
//     requires a Stripe Checkout or Spark/MPP Lightning payment before granting
//     the existing paid-privacy entitlement; blocker
//     blocker.product_promises.khala_code_paid_plan_not_purchasable stays.
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
export const KHALA_CODE_PLAN_CATALOG_VERSION = '2026-07-04.1' as const

export const KHALA_CODE_FREE_PLAN_ID = 'khala_code.plan.free.v1' as const
export const KHALA_CODE_PAID_PLAN_ID = 'khala_code.plan.paid.v1' as const

// Purchase-seam feature flag. DEFAULT OFF: while unarmed, the purchase route
// fails closed (503 khala_code_paid_plans_not_enabled) and the catalog reports
// the paid plan as not purchasable. Arming is an owner decision recorded via
// NEEDS_OWNER + the promise registry, never an agent decision.
export const KHALA_CODE_PAID_PLANS_ENABLED_ENV_KEY =
  'KHALA_CODE_PAID_PLANS_ENABLED' as const

export const KHALA_CODE_PLAN_PURCHASE_ROUTE =
  '/v1/khala-code/plans/purchases' as const

import type { PublicProjectionStalenessContract } from '../public-projection-staleness'

const ON_TOKENS = new Set(['1', 'on', 'true', 'yes'])

// Fail-closed flag read. Absent / non-string / any non-on value => disabled.
export const isKhalaCodePaidPlansEnabled = (value: unknown): boolean =>
  typeof value === 'string' && ON_TOKENS.has(value.trim().toLowerCase())

export const KHALA_CODE_PLAN_CATALOG_SUMMARY: string =
  'Khala Code is launching with a two-plan structure: Free (pay with data) ' +
  'and Paid (private data: capture opt-out). This catalog is the honest ' +
  'current state: the free plan is the default for everyone, free-plan ' +
  'desktop trace capture is NOT live, and the paid plan is NOT yet ' +
  'purchasable — its purchase seam is flag-gated off by default, and when ' +
  'armed it requires a real payment before any paid-privacy entitlement is ' +
  'granted.'

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
  'Free is the default plan: every Khala Code user without a paid-privacy entitlement is on it today; there is nothing to select or activate.',
  'Pay-with-data is launch-anchored design intent, not live behavior: Khala Code desktop coding sessions are NOT captured for training today.',
  'The Khala Code default path is Codex wrapper mode; its raw events and ATIF traces are owner-private delegation observability, never free-plan capture.',
  'The consented desktop capture pipeline (disclosure, consent, plan-scoped wiring, scrubbing) is its own planned promise: khala_code.free_plan_trace_capture.v1.',
  'Hosted Khala API free-tier capture terms are separate and disclosed at GET /api/public/free-tier-data-sharing (data.free_tier_capture_disclosure.v1).',
  'No payout is granted by capture: the data-market reward marker is inert and owner-gated.',
]

const PAID_PLAN_TERMS: ReadonlyArray<string> = [
  'Private data: the paid plan’s substance is the capture opt-out — a paid-privacy entitlement excludes the account from capture, fail-closed (when in doubt, not captured).',
  'The entitlement machinery is live on the hosted API (inference_privacy_entitlements, privacy.khala_paid_capture_optout.v1); the Khala Code plan purchase seam reuses it rather than duplicating truth.',
  'NOT yet purchasable: the purchase seam is flag-gated by KHALA_CODE_PAID_PLANS_ENABLED, default OFF, fail-closed, and grants no entitlement while unarmed.',
  'When armed, card purchases use Stripe Checkout and crypto purchases use the Spark/MPP Lightning invoice rail; only successful payment grants an idempotent paid-privacy entitlement receipt at /api/public/inference/privacy-receipts/{receiptRef}.',
  'Arming the seam, Stripe price id, Lightning sats price, live credentials, and public plan copy are owner decisions (blocker.product_promises.khala_code_paid_plan_not_purchasable).',
]

// Build the catalog. `paidPlanPurchaseArmed` is the fail-closed read of
// KHALA_CODE_PAID_PLANS_ENABLED so the catalog always reports the real
// purchasability state instead of a hardcoded hope; `generatedAt` and
// `staleness` are the serving route's freshness contract.
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
      priceLabel: options.paidPlanPurchaseArmed
        ? 'Purchase seam armed (owner-gated rollout)'
        : 'Not yet purchasable',
      isDefault: false,
      captureExcluded: true,
      terms: PAID_PLAN_TERMS,
      purchase: {
        armed: options.paidPlanPurchaseArmed,
        envFlag: KHALA_CODE_PAID_PLANS_ENABLED_ENV_KEY,
        route: KHALA_CODE_PLAN_PURCHASE_ROUTE,
      },
    },
  ],
  blockerRefs: [
    'blocker.product_promises.khala_code_paid_plan_not_purchasable',
  ],
  authorityBoundary:
    'Catalog text only. This projection grants no capture, billing, payout, ' +
    'or settlement authority; capture behavior stays governed by the ' +
    'disclosure and privacy-entitlement records, and no plan is presented ' +
    'as purchasable unless the owner-gated purchase seam is actually armed.',
  relatedPromiseIds: [
    KHALA_CODE_PLAN_PROMISE_ID,
    'khala_code.free_plan_trace_capture.v1',
    'privacy.khala_paid_capture_optout.v1',
    'data.free_tier_capture_disclosure.v1',
  ],
})
