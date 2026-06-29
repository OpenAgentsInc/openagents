# Provider Placement Policy

Date: 2026-06-06

Status: implemented contract note for issue #330 / `OPENAGENTS-083`.

## Purpose

OpenAgents product surface now has a reusable provider allowlist and placement restriction contract
for scheduler and runner-gateway decisions.

The implementation lives in
`workers/api/src/provider-placement-policy.ts`.

This is a pure policy contract. It does not dispatch work, select a live
provider account, connect secrets, change the runner gateway, or mutate any
production provider state.

## Provider Policy

`OpenAgentsProviderPolicy` records:

- provider ref;
- backend kind;
- provider trust tier;
- provider state;
- maximum workload trust;
- allowed work kinds;
- allowed data classifications;
- allowed classified surfaces;
- provider eligibility refs;
- policy refs;
- caveat refs;
- disabled reason refs; and
- cooldown refs.

Provider states are:

- `available`;
- `draining`;
- `cooldown`;
- `disabled`; and
- `blocked`.

Provider trust tiers are:

- `public`;
- `customer_visible`;
- `reviewed_private`;
- `legal_sensitive`;
- `payment_private`;
- `provider_private`;
- `internal_only`; and
- `blocked`.

## Placement Request

`OpenAgentsProviderPlacementRequest` records:

- the classified data policy envelope from #329;
- requested backend kind;
- required workload trust;
- work kind;
- owner grant refs;
- operator approval refs;
- legal review refs;
- payment policy refs;
- policy exception refs; and
- evidence refs.

Work kinds cover orders, Sites, Site revisions, artifacts, agent API actions,
Forum posts, Forum payments, private repositories, legal-sensitive work,
payment-sensitive actions, and customer asset processing.

## Decision Rules

`evaluateOpenAgentsProviderPlacement(provider, request)` returns an
`allowed` or `denied` decision plus public-safe blocker refs.

Hard blockers cannot be overridden by ordinary policy exception refs:

- provider blocked;
- provider disabled;
- provider draining/cooldown;
- blocked provider trust tier; and
- no-external-provider classification routed to a non-internal provider.

Overrideable blockers include:

- backend mismatch;
- workload trust too low;
- work kind not allowed;
- classified surface not allowed;
- data classification not allowed;
- required provider eligibility missing;
- private repo owner grant missing;
- legal review/operator approval missing; and
- payment policy missing.

Overrideable blockers can be bypassed only when a policy exception ref is
present. Owner grants and operator approvals can satisfy their own normal
requirements, but they are not broad overrides for unrelated placement policy.

Issue #331 defines the policy exception receipt contract that makes those
exception refs reviewable, scoped, expiring, and evidence-only.

## Redaction

`projectOpenAgentsProviderPlacement` emits public, customer, team, and operator
projections.

Public/customer/team projections hide provider-account refs, private provider
refs, policy exception refs, payment policy refs, legal review refs, operator
approval refs, and private workroom refs as appropriate. Operator projections
can show safe internal refs, but still reject raw secrets and raw payloads.

The contract rejects:

- provider grants, provider tokens, provider payloads, and raw auth state;
- private customer data;
- callback tokens, bearer tokens, OAuth material, API keys, and secrets;
- wallet material, invoices, payment proofs, preimages, and payout targets;
- private repo refs;
- raw runner logs;
- raw source archives; and
- raw timestamps.

## Relationship To Existing Repo Placement

`workers/api/src/coding-autopilot-repo-placement.ts` remains the
repo-specific placement policy added in #317.

This issue adds the cross-surface provider allowlist layer that future
scheduler and runner-gateway work can call before dispatching orders, Sites,
artifacts, agent API actions, payment-sensitive actions, legal-sensitive work,
and private repository work.

## Tests

`workers/api/src/provider-placement-policy.test.ts` covers:

- schema/projection decoding;
- public Site placement on an available eligible provider;
- unavailable provider hard denials;
- no-external-provider hard denials;
- explicit policy-exception overrides for overrideable mismatches;
- owner, legal review, and payment policy requirements; and
- projection redaction and unsafe ref rejection.
