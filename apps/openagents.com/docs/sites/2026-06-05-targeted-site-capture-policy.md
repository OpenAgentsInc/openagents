# Targeted Site Capture Policy Gate

Implemented: 2026-06-05

Issue: #183 / OPENAGENTS-SITES-OUTREACH-003

## Summary

OpenAgents product surface now has a typed capture-policy gate for targeted Site remake and
outreach campaigns.

Before any future static capture, rendered Browser Run capture, or approved
paid provider fallback treats a prospect as fetchable, it must consume an
explicit capture-policy decision. Only `allowed` and `paid_escalation`
decisions are fetchable. Every other state is non-fetchable until a new policy
event changes the decision.

## Service Boundary

`workers/api/src/targeted-site-capture-policy.ts` exposes:

- `evaluateTargetedSiteCapturePolicy`;
- `recordTargetedSiteCapturePolicyEvent`;
- `isTargetedSiteCaptureFetchable`;
- list helpers by campaign, prospect, and normalized domain;
- customer-safe and operator-safe projection helpers.

The service is intentionally not a route. Future capture workers should call
the service or read its event records before fetching target pages.

## Decision States

Policy decisions are typed:

- `allowed`;
- `disallowed`;
- `blocked`;
- `manual_review`;
- `customer_owned`;
- `suppressed`;
- `paid_escalation`.

The fetchable rule is deliberately narrow:

```text
fetchable = true only when decision is allowed or paid_escalation
```

The D1 schema enforces the same rule, so records cannot mark blocked,
suppressed, customer-owned, disallowed, or manual-review prospects as
fetchable.

## Inputs

The evaluator consumes public-safe refs and signals for:

- robots availability and policy;
- sitemap availability;
- suppression matches;
- contact or request suppression;
- customer-owned domain signals;
- manual review;
- paid provider escalation;
- bot-protection or login walls;
- unsupported schemes and unsafe domains.

Paid provider escalation requires a `paidEscalationRef`, so future capture
workers can distinguish approved paid capture from ordinary fetch permission.

## Audit Records

Migration `0073_targeted_site_capture_policy.sql` adds
`targeted_site_capture_policy_events`.

Each event records:

- campaign id;
- optional prospect id;
- normalized domain;
- source ref;
- decision and reason;
- fetchable boolean;
- robots, sitemap, suppression, customer-authority, paid-escalation, and
  operator refs;
- public-safe metadata;
- idempotency key;
- decision and creation timestamps.

Events are indexed by campaign, prospect, and normalized domain.

## Projection And Redaction

Customer-safe projections include only:

- campaign id;
- prospect id;
- normalized domain;
- source ref;
- decision;
- fetchability;
- decision timestamp.

Operator-safe projections can include typed reason and safe refs for robots,
sitemap, customer authority, and paid escalation. They expose booleans for
suppression and operator-note refs instead of returning those refs directly.

No projection returns raw contact data, suppression notes, provider payloads,
payment or wallet material, or bot-protection bypass instructions.

## Remaining Work

Issue batch #52 should build the consumers:

- static capture must require `isTargetedSiteCaptureFetchable` before fetch;
- Browser Run rendered capture must require the same policy gate;
- provider fallback adapters must create or consume `paid_escalation` policy
  decisions before paid capture begins.
