# Coding On Autopilot Repo Placement

Date: 2026-06-06

Status: implemented contract note for GitHub issue #317 / `OPENAGENTS-070`.

## Purpose

Coding on Autopilot needs a placement policy before scheduler or runner code
chooses a backend for repository work. The policy combines repo trust tier,
Omni data classification, runner backend, workload trust, customer grant refs,
provider grant refs, and operator approval refs.

The implementation lives in
`workers/api/src/coding-autopilot-repo-placement.ts`.

## Repo Trust Tiers

The v1 trust tiers are:

- `public`;
- `private`;
- `sensitive`;
- `infra`;
- `legal_sensitive`;
- `payment_sensitive`;
- `regulated`;
- `unknown`.

## Placement Decisions

The policy returns one of:

- `eligible`;
- `needs_customer_grant`;
- `needs_operator_approval`;
- `needs_provider_grant`;
- `blocked`.

`legal_sensitive`, `payment_sensitive`, and `regulated` contexts are SHC-only
in this first policy. Private-like contexts require a customer repo grant.
Sensitive, infra, legal, payment, and regulated contexts require operator
approval. Payment-sensitive and payment-private contexts require a provider
grant. Unknown or secret-bearing/provider-private contexts are blocked.

## Public Proof Rule

`publicClaimAllowed` is true only when the placement is eligible, the repo trust
tier is `public`, and the data classification is `public`. Private,
sensitive, infra, legal, payment, regulated, unknown, provider-private, and
secret-bearing contexts cannot become public proof claims through this policy.

## Projection Rules

Public projection hides workroom refs, customer grants, provider grants,
operator approvals, and private repo refs.

Customer projection can show safe customer-grant and workroom refs but hides
provider grants and operator approvals.

Team projection can show operator approval refs but hides provider grants.

Operator projection can see all safe refs, but raw provider tokens, private repo
URLs, customer emails, runner payloads, source archives, wallet/payment
material, and secrets are rejected before projection.

## Tests

`workers/api/src/coding-autopilot-repo-placement.test.ts` covers:

- public repo eligibility and public-claim allowance;
- private repo customer-grant gating;
- legal/payment backend restrictions;
- payment provider-grant gating;
- unknown and secret-bearing blocking;
- unsafe repo, grant, provider, runner, and customer ref rejection.
