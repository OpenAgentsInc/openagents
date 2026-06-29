# Payment Limit Policy Classifier

Issue: #289 / OPENAGENTS-H-001

Date: 2026-06-06

## Purpose

OpenAgents needs a shared policy layer before credits, Stripe, Lightning, MDK,
and L402 recovery are connected to live endpoints. The first rule is that
payment can recover only economic limits. Payment must never bypass safety,
abuse, private authority, hidden data, owner grants, provider-capacity outages,
or manual review gates.

The implementation in `workers/api/src/payment-limit-policy.ts` creates a
typed classifier and projection boundary that future billing, Forum paid
actions, Site checkout, agent API, and runner routes can reuse.

## Limit Classes

| Class | Decision | Payment recovery |
| --- | --- | --- |
| `safety` | `blocked` | No. Safety and moderation policy cannot be bought around. |
| `abuse` | `blocked` | No. Abuse controls remain hard limits. |
| `private_authority` | `blocked` | No. Payment cannot replace owner consent, scoped grants, auth, or private-data authority. |
| `provider_capacity` | `manual_review` | No automatic recovery. Operators must decide whether capacity exists. |
| `manual_review` | `manual_review` | No automatic recovery. The route is intentionally waiting on a human/operator decision. |
| `free_beta_allowance` | `allow` when allowance exists, otherwise `recoverable` or `manual_review` | Free allowance can allow the action; otherwise the action may move to credits or L402 when configured. |
| `credits` | `allow` when credit balance exists, otherwise `recoverable` or `manual_review` | Credit balance can allow the action; missing credits can point to recoverable products. |
| `economic_usage` | `recoverable` when credits or L402/MDK are configured | Yes. This is the normal paid recovery path. |
| `l402_mdk_recoverable` | `recoverable` when credits or L402/MDK are configured | Yes. This marks a route that is intended to return a payment challenge once the credential service exists. |

## Decision Shape

The classifier returns:

- `decisionStatus`: `allow`, `blocked`, `manual_review`, or `recoverable`.
- `recoveryActions`: `free_beta`, `credit_balance`, `l402_mdk`, or
  `manual_review`.
- `reasonRefs` and `statusRefs` for route-specific messages without exposing
  private provider details.
- `requiredProductRefs` and `requiredEndpointRefs` so future catalog and L402
  work can attach stable product IDs and endpoint bindings.
- `spendCapCaveatRefs` so agents can understand bounded payment recovery before
  paying.
- `entitlementScopeRefs` for the later entitlement ledger.

The classifier does not charge money, mint credentials, create checkout
intents, satisfy L402 challenges, or grant endpoint authority. It only answers
whether a limit is recoverable and what kind of recovery should be offered.

## Projection Boundary

Customer, agent, and public projections remove operator cost refs and all
private account refs. Operator projections may include safe internal cost refs,
but still strip provider account refs and secret-shaped material.

The redaction guard rejects refs that look like:

- bearer tokens, cookies, OAuth material, API keys, GitHub tokens, provider
  grants, callback tokens, wallet material, mnemonics, raw invoices, raw
  preimages, raw payment material, raw prompts, raw runner logs, source
  archives, customer emails, or secret-shaped strings;
- runner gateway payloads that already fail the runner private-material guard.

This keeps payment-policy projections usable by public proof, customer order
pages, agent API responses, and operator dashboards without leaking payment,
provider, customer, or source material.

## Integration Notes

- Forum paid actions should call this policy before returning a future L402
  challenge. If the limit is safety, abuse, private authority, locked topic, or
  moderation-related, the route should return the existing non-payment denial.
- Site checkout should use this policy to distinguish checkout products from
  owner/review gates. A buyer can pay for a product or paid action, not for
  approval to bypass a customer or operator decision.
- Runner routes should treat provider capacity as manual review or failover
  policy input, not as a paid recovery path. Economic usage can become
  recoverable only after the runner has a product catalog, spend caps, and
  receipt-backed entitlement records.
- Agent API routes should expose recoverable limits with product and endpoint
  refs once #290 through #292 exist. Until then, this slice is contract-only.

## Verification

- `bun run --cwd workers/api test -- src/payment-limit-policy.test.ts`
- `bun run --cwd workers/api typecheck`
