# Targeted Site Static Capture Service

Implemented: 2026-06-05

Issue: #184 / OPENAGENTS-SITES-OUTREACH-004

## Summary

OpenAgents product surface now has the first static capture service contract for targeted Site
remake and outreach campaigns.

The service does not perform network fetches yet. It records and projects the
typed result of a future cheap Worker capture path: homepage refs, selected
page refs, same-origin asset refs, robots and sitemap refs, response summaries,
source hashes, and source-pack refs. Future static capture workers should use
this service as the ledger boundary after they fetch and normalize public
source material.

## Policy Gate

Static capture requires a fetchable capture-policy event from issue #183.

`recordTargetedSiteStaticCaptureRun` accepts a
`TargetedSiteCapturePolicyEventRecord` and rejects the run unless:

```text
policy.fetchable = true
policy.decision = allowed or paid_escalation
```

This prevents static capture from proceeding for suppressed, blocked,
disallowed, customer-owned, or manual-review prospects.

## Service Boundary

`workers/api/src/targeted-site-static-capture.ts` exposes:

- `normalizeTargetedSiteStaticCaptureUrl`;
- `recordTargetedSiteStaticCaptureRun`;
- list helpers by campaign, prospect, and normalized domain;
- customer-safe and operator-safe projections.

The URL normalizer allows only same-origin `http` and `https` URLs for the
prospect's normalized domain, removes fragments, rejects credentials, and
rejects cross-origin URLs.

## Audit Records

Migration `0074_targeted_site_static_capture.sql` adds
`targeted_site_static_capture_runs`.

Each run records:

- campaign id;
- optional prospect id;
- normalized domain;
- capture-policy event id;
- static capture state and reason;
- normalized homepage URL;
- homepage, robots, sitemap, source-pack, and source-hash refs;
- bounded selected page refs;
- bounded same-origin asset refs;
- bounded response summary;
- public-safe metadata;
- start, completion, creation, and archive timestamps.

Runs are indexed by campaign, prospect, and normalized domain.

## State Model

Static capture states are:

- `planned`;
- `succeeded`;
- `partial`;
- `failed`;
- `blocked`;
- `manual_review`;
- `archived`.

Reasons are typed and include policy fetchability, homepage fetch success,
partial page capture, network errors, invalid or cross-origin URLs, response
size/content-type limits, robots changes, manual review, and source-pack
readiness.

## Projection And Redaction

Customer-safe projections include counts and public-safe refs only:

- campaign id;
- prospect id;
- normalized domain;
- homepage URL;
- state;
- page and asset counts;
- source-pack ref;
- timestamps.

Operator-safe projections include the capture-policy event id, reason,
response summary, robots/sitemap refs, homepage/source refs, source hash, and
metadata presence. They still do not expose raw page refs, asset refs,
metadata, provider payloads, contact data, payment or wallet material, or
bot-protection bypass instructions.

## Remaining Work

Issue #185 should add rendered Browser Run capture records that consume the
same #183 policy gate and can optionally link to a static capture run.

Later issues should add actual Worker fetch orchestration, R2 source-pack
writes, audit scoring, remake briefs, and preview generation.
