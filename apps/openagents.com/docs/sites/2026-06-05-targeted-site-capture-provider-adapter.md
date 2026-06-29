# Targeted Site Capture Provider Adapter Boundary

Implemented: 2026-06-05

Issue: #186 / OPENAGENTS-SITES-OUTREACH-006

## Summary

OpenAgents product surface now has a provider-neutral capture adapter boundary for targeted Site
remake campaigns.

The boundary records approved fallback, benchmark, first-party, denied,
failed, partial, succeeded, and manual-review provider adapter runs without
making raw provider payloads product authority.

## Service Boundary

`workers/api/src/targeted-site-capture-provider-adapter.ts` exposes:

- `recordTargetedSiteCaptureProviderAdapterRun`;
- list helpers by campaign, prospect, and normalized domain;
- customer-safe and operator-safe projections.

Provider kinds are typed:

- `first_party_worker`;
- `browser_run`;
- `firecrawl`;
- `browserless`;
- `browserbase`;
- `apify`;
- `container`.

## Policy Gate

Every provider adapter run requires a fetchable issue #183 capture-policy
event:

```text
policy.fetchable = true
policy.decision = allowed or paid_escalation
```

Paid provider fallback also requires explicit paid-escalation evidence. For
paid provider kinds such as Firecrawl, Browserless, Browserbase, Apify, or
Container, an approved fallback, partial result, or success must carry either
the policy event's `paidEscalationRef` or an input `paidEscalationRef`.

## Audit Records

Migration `0076_targeted_site_capture_provider_adapters.sql` adds
`targeted_site_capture_provider_adapter_runs`.

Each run records:

- campaign id;
- optional prospect id;
- normalized domain;
- capture-policy event id;
- optional static and rendered capture run ids;
- provider kind;
- state and reason;
- paid-escalation ref;
- provider request and receipt refs;
- output-pack ref;
- usage and cost refs;
- public-safe metadata;
- requested, completion, creation, and archive timestamps.

Runs are indexed by campaign, prospect, and normalized domain.

## Projection And Redaction

Customer-safe projections include:

- campaign id;
- prospect id;
- normalized domain;
- provider kind;
- state;
- output availability;
- requested and completion timestamps.

Operator-safe projections include typed provider/request/receipt/output/usage
refs and whether metadata exists. They still do not expose raw provider
payloads, browser logs, contact data, payment or wallet material, or
bot-protection bypass instructions.

## Remaining Work

Issue #187 should consume the first-party static/rendered capture ledgers and
provider adapter refs to produce bounded website quality audit scores.
