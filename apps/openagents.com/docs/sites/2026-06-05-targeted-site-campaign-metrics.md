# Targeted Site Campaign Metrics And Conversion Ledger

Date: 2026-06-05

Status: implemented for issue #206.

## Purpose

Targeted Site remake campaigns need durable, auditable metrics without relying
on mutable counters or raw provider payloads. The campaign metrics ledger records
public-safe events and derives aggregate projections from those events.

## D1 Ledger

The table is `targeted_site_campaign_metric_events`.

Each row records:

- `id` and unique `idempotency_key`;
- `campaign_id`;
- optional `prospect_id`;
- optional `normalized_domain`;
- `event_kind`;
- `quantity`;
- `cost_cents`;
- optional `public_ref`;
- required `source_ref`;
- optional `related_event_id`;
- bounded `metadata_json`;
- `occurred_at`, `created_at`, and `archived_at`.

Event kinds:

- `capture_cost`
- `preview_generated`
- `outreach_sent`
- `email_bounced`
- `email_replied`
- `meeting_booked`
- `customer_converted`
- `accepted_outcome`
- `refund`
- `complaint`
- `suppressed`
- `blocked`

Refund and complaint events must link to a related prior metric event.

## Service Contract

`recordTargetedSiteCampaignMetricEvent`:

- validates id, idempotency, campaign, prospect, public, source, and related
  refs as public-safe refs;
- validates normalized domains;
- rejects raw provider, raw email, private customer, wallet, and payment-shaped
  material in refs and metadata;
- requires active, unarchived campaign records;
- requires prospect refs to belong to the same active campaign;
- records idempotently by `idempotency_key`.

`projectTargetedSiteCampaignMetrics`:

- validates the campaign ref;
- requires an active campaign;
- derives aggregate counts and total capture cost from unarchived metric rows.

## Projection

The projection includes:

- total capture cost in cents;
- preview, sent, bounce, reply, meeting, conversion, accepted outcome, refund,
  complaint, suppressed, and blocked counts;
- latest event timestamp;
- internal event count.

The public-safe helper omits raw event rows and internal event count. It exposes
only aggregate counts and totals.

## Boundaries

This slice does not send outreach, book meetings, accept customers, settle
rewards, or record payouts. It creates the campaign metrics ledger that later
operator dashboards, scoped agent campaign tools, and accepted-outcome reward
policy can consume.
