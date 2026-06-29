# Data Classification and Trust v1

Issue #219 adds the first Omni data classification and trust model.

The model gives workrooms, and later artifacts, sources, provider routes,
projections, and API responses, an explicit privacy/trust boundary that
placement policy can enforce.

## Classifications

The initial classifications are:

- `public`
- `customer`
- `team`
- `operator`
- `private`
- `legal_sensitive`
- `provider_private`
- `payment_private`
- `secret_bearing`

Workrooms now persist:

- `dataClassification`
- `trustTier`
- `classificationCaveatRef`

The D1 columns are `data_classification`, `trust_tier`, and
`classification_caveat_ref`.

## Trust Tiers

The initial trust tiers are:

- `verified`
- `reviewed`
- `unverified`
- `blocked`

Blocked records cannot project to any normal audience. Secret-bearing records
only project to the private/internal audience.

## Projection Enforcement

The classification helper enforces audience projection:

- Public audiences can see only public records.
- Customers can see public and customer records.
- Team audiences can see public, team, and operator records.
- Operators can see public, customer, team, operator, private,
  legal-sensitive, provider-private, and payment-private records.
- Private/internal audiences can see every classification, including
  secret-bearing records.

Workroom public, customer, and operator projections now include classification
fields and enforce these rules.

## Downgrade Rules

The helper supports safe transition checks:

- Moving to a more restrictive classification is allowed.
- Downgrading legal-sensitive, payment-private, or provider-private data
  requires redaction evidence.
- Secret-bearing data cannot be downgraded by this policy.

## Current Scope

This issue wires classification onto workrooms first. Evidence bundles, proof
bundles, route scorecards, and future API response envelopes should consume the
same helper rather than defining separate classification semantics.

## v2 Extension

Issue #329 / `OPENAGENTS-082` extends this helper into the reusable OpenAgents product surface data
policy envelope documented at
`../2026-06-06-openagents-data-classification-policy-v2.md`.

The v2 contract adds the `agent` projection audience,
`deletion_retention_sensitive` classification, classified surfaces for orders,
Sites, artifacts, Forum/payment refs, provider-account refs, customer assets,
and agent API payloads, plus typed allow/redact/omit/deny projection
decisions, export/retention policy refs, provider eligibility refs, and a
stricter unsafe-ref guard.
