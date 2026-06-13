// Authorizing policy for the Provider Capacity Marketplace Gate's
// "API-inference gateway resale is allowed only through an explicit policy path
// ... with tests" clause (apps/openagents.com/INVARIANTS.md). This is the
// "future policy" that gate's invariant requires before base API-inference
// resale is permitted. It implements the Model-2 commercial-plan authorization
// (docs/autopilot-coder/2026-06-13-cloud-remote-execution-commercial-plan.md):
//
// - Selling API inference bought on OpenAgents' OWN commercial / API-key
//   accounts (the OpenRouter-style gateway) is AUTHORIZED, but only when the
//   full provider-capacity ref chain is present.
// - Converting a consumer SUBSCRIPTION login into resale is BLOCKED
//   unconditionally — the non-waivable rule, regardless of any refs.
// - Accepted-outcome / agentic-work labor stays allowed (the existing path).
//
// This decides authorization of the mechanism only. It does not compute price
// and does not authorize public marketplace/monetization COPY, which the gate
// keeps separately blocked until the full settlement-receipt chain exists.

export type InferenceMonetizationKind =
  | 'agentic_work'
  | 'api_inference_gateway_resale'
  | 'subscription_capacity_resale'

export type ProviderAccountAuthMode = 'api_key' | 'subscription'

export type InferenceResaleRefs = {
  providerGrantRef: string | null
  routePolicyRef: string | null
  meteringReceiptRef: string | null
  pricingPolicyRef: string | null
  tosBoundaryRef: string | null
  dispatchRef: string | null
  assignmentReceiptRef: string | null
  settlementReceiptRef: string | null
}

// The ref chain that must be present before API-inference gateway resale is
// authorized, each paired with the typed blocker ref emitted when it is absent.
export const INFERENCE_RESALE_REQUIRED_REFS = [
  { field: 'providerGrantRef', missingRef: 'blocker.inference_resale.missing.provider_grant' },
  { field: 'routePolicyRef', missingRef: 'blocker.inference_resale.missing.route_policy' },
  { field: 'meteringReceiptRef', missingRef: 'blocker.inference_resale.missing.metering_receipt' },
  { field: 'pricingPolicyRef', missingRef: 'blocker.inference_resale.missing.pricing_policy' },
  { field: 'tosBoundaryRef', missingRef: 'blocker.inference_resale.missing.tos_boundary' },
  { field: 'dispatchRef', missingRef: 'blocker.inference_resale.missing.dispatch' },
  { field: 'assignmentReceiptRef', missingRef: 'blocker.inference_resale.missing.assignment_receipt' },
  { field: 'settlementReceiptRef', missingRef: 'blocker.inference_resale.missing.settlement_receipt' },
] as const satisfies ReadonlyArray<{ field: keyof InferenceResaleRefs; missingRef: string }>

export const INFERENCE_RESALE_SUBSCRIPTION_FORBIDDEN_REF =
  'blocker.inference_resale.subscription_resale_forbidden'
export const INFERENCE_RESALE_REQUIRES_API_KEY_REF =
  'blocker.inference_resale.requires_api_key_account'

export type InferenceResaleAuthorizationDecision = {
  schema: 'openagents.inference_resale_authorization.v1'
  kind: InferenceMonetizationKind
  authorized: boolean
  blockerRefs: string[]
}

function refPresent(value: string | null | undefined): boolean {
  return typeof value === 'string' && value.trim().length > 0
}

export function authorizeInferenceMonetization(input: {
  kind: InferenceMonetizationKind
  accountAuthMode?: ProviderAccountAuthMode
  refs?: Partial<InferenceResaleRefs>
}): InferenceResaleAuthorizationDecision {
  const base = {
    schema: 'openagents.inference_resale_authorization.v1' as const,
    kind: input.kind,
  }

  // Non-waivable: a consumer subscription login is never resale authorization.
  if (input.kind === 'subscription_capacity_resale') {
    return { ...base, authorized: false, blockerRefs: [INFERENCE_RESALE_SUBSCRIPTION_FORBIDDEN_REF] }
  }

  // Accepted-outcome / agentic-work labor is the existing allowed path.
  if (input.kind === 'agentic_work') {
    return { ...base, authorized: true, blockerRefs: [] }
  }

  // api_inference_gateway_resale (Model 2): OpenAgents' own API-key account
  // only, and only with the full ref chain present.
  const blockerRefs: string[] = []
  if (input.accountAuthMode !== 'api_key') {
    blockerRefs.push(INFERENCE_RESALE_REQUIRES_API_KEY_REF)
  }
  for (const required of INFERENCE_RESALE_REQUIRED_REFS) {
    if (!refPresent(input.refs?.[required.field])) blockerRefs.push(required.missingRef)
  }
  return { ...base, authorized: blockerRefs.length === 0, blockerRefs }
}
