# Targeted Site Rendered Capture Service

Implemented: 2026-06-05

Issue: #185 / OPENAGENTS-SITES-OUTREACH-005

## Summary

OpenAgents product surface now has the rendered capture service contract for targeted Site remake
and outreach campaigns.

Rendered capture is the escalation path after cheap static capture is
insufficient or an operator/campaign approves additional browser cost. The
service records Browser Run-style output refs and bounded usage summaries
without storing provider payloads or raw browser logs.

## Policy Gate

Rendered capture requires the same issue #183 capture-policy event used by the
static capture service.

`recordTargetedSiteRenderedCaptureRun` rejects the run unless:

```text
policy.fetchable = true
policy.decision = allowed or paid_escalation
```

This prevents rendered capture from proceeding for suppressed, blocked,
disallowed, customer-owned, or manual-review prospects.

## Service Boundary

`workers/api/src/targeted-site-rendered-capture.ts` exposes:

- `recordTargetedSiteRenderedCaptureRun`;
- list helpers by campaign, prospect, and normalized domain;
- customer-safe and operator-safe projections.

It reuses the static capture URL normalizer so rendered capture targets remain
same-origin `http` or `https` URLs for the prospect's normalized domain.

## Audit Records

Migration `0075_targeted_site_rendered_capture.sql` adds
`targeted_site_rendered_capture_runs`.

Each run records:

- campaign id;
- optional prospect id;
- normalized domain;
- capture-policy event id;
- optional static capture run id;
- rendered capture state and reason;
- normalized target URL;
- provider ref;
- screenshot, rendered HTML, markdown, links, structured JSON, and crawl refs;
- viewport and device refs;
- bounded usage summary;
- public-safe metadata;
- start, completion, creation, and archive timestamps.

Runs are indexed by campaign, prospect, and normalized domain.

## State Model

Rendered capture states are:

- `planned`;
- `succeeded`;
- `partial`;
- `failed`;
- `blocked`;
- `manual_review`;
- `archived`.

Reasons cover policy fetchability, static-capture insufficiency, screenshot
readiness, rendered source readiness, crawl readiness, usage limits, network
errors, provider errors, bot-protection/login walls, and manual review.

Bot-protection or login-wall signals can be recorded as blocked evidence, but
they cannot record rendered output refs. This keeps the system from turning
into a bypass service.

## Usage Summary

Usage is bounded before storage:

- browser milliseconds are capped at one hour;
- bytes are capped at 50 MB;
- page count is capped at 100;
- estimated credits are capped at 10,000;
- cost evidence is a public-safe ref only.

## Projection And Redaction

Customer-safe projections expose only:

- campaign id;
- prospect id;
- normalized domain;
- target URL;
- state;
- booleans for screenshot, markdown, and crawl availability;
- timestamps.

Operator-safe projections expose typed provider/output refs and bounded usage
summary. They still do not expose raw provider payloads, raw browser logs,
metadata, contact data, payment or wallet material, or bot-protection bypass
instructions.

## Remaining Work

The next outreach batch should add provider adapter boundaries, quality audit
scoring, and remake/source-authority briefs that consume static and rendered
capture records.
