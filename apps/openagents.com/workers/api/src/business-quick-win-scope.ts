import { Schema as S } from 'effect'

/**
 * Deterministic intake -> quick-win scope router for OpenAgents Business.
 *
 * The promise business.intake_quick_win_offering.v1 is yellow with two open
 * blockers:
 *   - blocker.product_promises.business_quick_win_self_serve_delivery_missing
 *   - blocker.product_promises.business_first_paid_quick_win_receipt_missing
 *
 * Today the move from a recorded `/business` intake to a scoped quick win is
 * done by an operator reading the free-text "what do you need help with" and
 * deciding (a) which menu offering backs it and (b) what "done" looks like.
 * This module automates exactly that first decision: it deterministically routes
 * an intake to a backing offering promiseId and emits a structured scope with a
 * definition-of-done checklist and a `quickWinScopedRef` that feeds
 * buildBusinessQuickWinReceipt's `quick_win_scoped` line.
 *
 * Honesty rules, enforced by construction:
 * - Routing is automated; DELIVERY is not. Every route's `deliveryMode` is
 *   reported straight from the backing offering's registry availability and is
 *   never `self_serve` yet, because no business quick-win offering is
 *   self-serve deliverable end-to-end today. Flipping a route to `self_serve`
 *   (with proof) is the concrete meaning of closing the self-serve blocker.
 * - An intake that matches no offering is routed to operator triage with an
 *   explicit open question rather than being force-fit to an offering.
 * - This module moves no money and asserts no delivery; it only produces the
 *   scope an operator (and later a self-serve loop) acts on.
 */

export const QuickWinOfferingAvailability = S.Literals([
  'available_now',
  'available_soon',
  'roadmap',
])
export type QuickWinOfferingAvailability =
  typeof QuickWinOfferingAvailability.Type

/**
 * How a scoped quick win can be delivered today:
 * - operator_assisted: an operator must drive delivery (backing offering is
 *   shipped or flag-gated but not packaged as a one-click product).
 * - not_deliverable: the backing offering is roadmap; the scope captures intent
 *   only and cannot be delivered now.
 * - self_serve: reserved for a future state — NOT emitted by any current route.
 *   It exists so closing the self-serve blocker is a route-level data change
 *   with a verifier, not a rewrite.
 */
export const QuickWinDeliveryMode = S.Literals([
  'self_serve',
  'operator_assisted',
  'not_deliverable',
])
export type QuickWinDeliveryMode = typeof QuickWinDeliveryMode.Type

export const QuickWinRouteCategory = S.Literals([
  'coding_agent_work',
  'inference_ai',
  'inference_batch',
  'sites_commerce',
  'ecommerce_workspace',
  'legal_workspace',
  'marketing_workspace',
  'compute_training',
  'unmatched_operator_triage',
])
export type QuickWinRouteCategory = typeof QuickWinRouteCategory.Type

type QuickWinRoute = Readonly<{
  category: QuickWinRouteCategory
  offeringPromiseId: string
  availability: QuickWinOfferingAvailability
  deliveryMode: QuickWinDeliveryMode
  // Lowercase keyword stems; an intake matches the first route whose keyword is
  // a substring of the normalized help text.
  keywords: ReadonlyArray<string>
  definitionOfDone: ReadonlyArray<string>
}>

// Ordered most-specific first: batch before generic inference, vertical packs
// before generic automation. The unmatched route is the explicit fallback and
// is never keyword-matched.
const QUICK_WIN_ROUTES: ReadonlyArray<QuickWinRoute> = [
  {
    category: 'inference_batch',
    offeringPromiseId: 'inference.batch_processing_jobs.v1',
    availability: 'roadmap',
    deliveryMode: 'not_deliverable',
    keywords: ['batch', 'bulk', 'classif', 'extract', 'summari', 'dataset'],
    definitionOfDone: [
      'Agreed input batch and per-row output schema.',
      'Sample run on a small slice reviewed and accepted.',
      'Full batch processed and results handed back with a run manifest.',
    ],
  },
  {
    category: 'inference_ai',
    offeringPromiseId: 'inference.free_tier_taste.v1',
    availability: 'available_now',
    deliveryMode: 'operator_assisted',
    keywords: ['inference', 'model', 'llm', 'gemini', 'deepseek', 'prompt', 'gateway'],
    definitionOfDone: [
      'Target model and call shape agreed.',
      'Free-allowance taste call returns a usable result.',
      'Hand back result plus how to fund metered usage after the taste.',
    ],
  },
  {
    category: 'ecommerce_workspace',
    offeringPromiseId: 'business.ecommerce_workspace_pack.v1',
    availability: 'available_soon',
    deliveryMode: 'operator_assisted',
    keywords: ['ecommerce', 'e-commerce', 'inventory', 'shopify', 'product catalog', 'store'],
    definitionOfDone: [
      'Inventory/catalog source connected to the prefilled workspace.',
      'One inventory-aware campaign drafted and reviewed.',
      'Campaign artifact handed back with evidence.',
    ],
  },
  {
    category: 'legal_workspace',
    offeringPromiseId: 'business.legal_workspace_pack.v1',
    availability: 'available_soon',
    deliveryMode: 'operator_assisted',
    keywords: ['legal', 'contract', 'forms', 'intake form', 'paralegal', 'compliance'],
    definitionOfDone: [
      'Form/intake template selected (review-gated; no legal advice given).',
      'Draft produced and routed for human review.',
      'Reviewed artifact handed back with the review attestation.',
    ],
  },
  {
    category: 'marketing_workspace',
    offeringPromiseId: 'business.marketing_agency_workspace_pack.v1',
    availability: 'available_soon',
    deliveryMode: 'operator_assisted',
    keywords: ['marketing', 'agency', 'campaign', 'white-label', 'white label', 'ad copy', 'newsletter'],
    definitionOfDone: [
      'Brand and audience captured in the white-label workspace.',
      'Landing page plus email sequence drafted and reviewed.',
      'Published artifacts handed back with links.',
    ],
  },
  {
    category: 'sites_commerce',
    offeringPromiseId: 'autopilot_sites.site_build_and_host.v1',
    availability: 'available_soon',
    deliveryMode: 'operator_assisted',
    keywords: ['site', 'website', 'landing page', 'hostname', 'domain', 'email sequence', 'referral'],
    definitionOfDone: [
      'Page content and target URL agreed.',
      'Site built and served at a stable URL (flag-gated).',
      'URL handed back with a render check.',
    ],
  },
  {
    category: 'compute_training',
    offeringPromiseId: 'training.decentralized_training_launch.v1',
    availability: 'available_now',
    deliveryMode: 'operator_assisted',
    keywords: ['training', 'fine-tune', 'fine tune', 'gpu', 'compute', 'sandbox'],
    definitionOfDone: [
      'Training/compute objective and dataset scoped.',
      'Run launched and progress reported.',
      'Result handed back with contributor settlement evidence.',
    ],
  },
  {
    category: 'coding_agent_work',
    offeringPromiseId: 'business.coding_quick_win.v1',
    availability: 'available_now',
    deliveryMode: 'operator_assisted',
    keywords: ['code', 'coding', 'bug', 'test', 'refactor', 'feature', 'repo', 'script', 'slop', 'software'],
    definitionOfDone: [
      'Objective and verification command agreed against the target repo.',
      'Agent produces a reviewable diff with verification evidence.',
      'Diff handed back; outcome accepted against the verification command.',
    ],
  },
]

const UNMATCHED_ROUTE: QuickWinRoute = {
  category: 'unmatched_operator_triage',
  offeringPromiseId: 'business.intake_quick_win_offering.v1',
  availability: 'available_soon',
  deliveryMode: 'operator_assisted',
  keywords: [],
  definitionOfDone: [
    'Operator reads the raw request and confirms which offering (if any) backs it.',
    'If nothing on the menu fits, capture it as an open question and tell the customer plainly.',
  ],
}

export const QuickWinScope = S.Struct({
  scopeKind: S.Literal('business_quick_win_scope'),
  // The /business intake signup this scope derives from
  // (BusinessSignupRecord.id).
  signupId: S.String,
  // Normalized free-text request the route was derived from (empty when the
  // intake left "what do you need help with" blank).
  requestedHelp: S.String,
  category: QuickWinRouteCategory,
  offeringPromiseId: S.String,
  availability: QuickWinOfferingAvailability,
  deliveryMode: QuickWinDeliveryMode,
  definitionOfDone: S.Array(S.String),
  // True whenever delivery still needs an operator (today: always true).
  needsOperator: S.Boolean,
  // True when no menu offering matched and the intake was routed to triage.
  unmatched: S.Boolean,
  // Deterministic, dereferenceable reference for the quick_win_scoped lifecycle
  // line; pass this as buildBusinessQuickWinReceipt's quickWinScopedRef.
  quickWinScopedRef: S.String,
})
export type QuickWinScope = typeof QuickWinScope.Type

export class QuickWinScopeInvariantError extends S.TaggedErrorClass<QuickWinScopeInvariantError>()(
  'QuickWinScopeInvariantError',
  { reason: S.String },
) {}

export type QuickWinScopeInput = Readonly<{
  // BusinessSignupRecord.id — required; a scope must derive from a recorded
  // intake.
  signupId: string
  // BusinessSignupRecord.helpWith — may be null/empty.
  helpWith: string | null
}>

const normalizeHelpText = (value: string | null): string =>
  (value ?? '').replace(/\s+/g, ' ').trim()

const selectRoute = (normalizedLower: string): QuickWinRoute => {
  if (normalizedLower === '') {
    return UNMATCHED_ROUTE
  }
  for (const route of QUICK_WIN_ROUTES) {
    if (route.keywords.some(keyword => normalizedLower.includes(keyword))) {
      return route
    }
  }
  return UNMATCHED_ROUTE
}

/**
 * Route a recorded `/business` intake to a structured quick-win scope.
 * Deterministic and pure: identical input yields an identical scope.
 */
export const scopeQuickWinFromIntake = (
  input: QuickWinScopeInput,
): QuickWinScope => {
  const signupId = input.signupId.trim()
  if (signupId === '') {
    throw new QuickWinScopeInvariantError({
      reason: 'signupId is required: a quick-win scope must derive from a recorded /business intake.',
    })
  }

  const requestedHelp = normalizeHelpText(input.helpWith)
  const route = selectRoute(requestedHelp.toLowerCase())
  const unmatched = route.category === 'unmatched_operator_triage'

  return {
    scopeKind: 'business_quick_win_scope',
    signupId,
    requestedHelp,
    category: route.category,
    offeringPromiseId: route.offeringPromiseId,
    availability: route.availability,
    deliveryMode: route.deliveryMode,
    definitionOfDone: route.definitionOfDone,
    needsOperator: route.deliveryMode !== 'self_serve',
    unmatched,
    quickWinScopedRef: `quick-win-scope:${signupId}:${route.category}`,
  }
}

/**
 * Public projection: surfaces the routing decision and definition-of-done
 * without leaking the raw customer request text.
 */
export const publicQuickWinScopeProjection = (scope: QuickWinScope) => ({
  scopeKind: scope.scopeKind,
  category: scope.category,
  offeringPromiseId: scope.offeringPromiseId,
  availability: scope.availability,
  deliveryMode: scope.deliveryMode,
  definitionOfDone: scope.definitionOfDone,
  needsOperator: scope.needsOperator,
  unmatched: scope.unmatched,
})
