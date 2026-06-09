# Customer Site Feedback To Autopilot Queue

Date: 2026-06-05

## What Changed

Customer Site feedback submitted from the order detail surface now becomes an
actionable Autopilot queue item when the order already has a Site.

The customer API writes:

- a `site_revision_feedback` record;
- a public `adjutant_adjustment_requests` record;
- `adjutant_assignment_events` and `site_events` trace records;
- an `agent_queued` software order status so customer-safe projections show
  that the adjustment is back in the queue.

If the Site already has an active Adjutant assignment with a current run, the
adjustment is marked as a `follow_up_turn` continuation and preserves the
source run ID for the operator dispatch path. If there is no active assignment,
the customer route creates a public `site_adjustment` assignment and marks the
adjustment as a `new_goal_run` queue item.

## Safety

Customer text is copied into an Autopilot adjustment prompt only when it does
not look like provider/account secret material. Secret-shaped feedback is still
captured as customer feedback, but it remains in `submitted` status until an
operator can review it.

The queue path stores public visibility because the customer-facing Site
revision loop is intentionally public-safe. Runner internals, raw dispatch
payloads, and private operator data remain outside the customer projection.

## Idempotency

The first implementation uses duplicate open-feedback detection. If the same
user submits the same body for the same order, Site, version, and deployment
while the existing feedback is still `submitted`, `queued`, or `running`, the
API returns the existing feedback instead of creating another adjustment.

Future UI/API work can add an explicit client request ID for stronger retry
semantics across edited comments or network retries.

## Current Limit

If an order has no Site yet, feedback is saved but not queued into an Adjutant
adjustment because adjustment requests require a Site ID. This is acceptable
for the first batch because pre-Site feedback still appears in the order
timeline; the fulfillment queue should consume it when creating the first Site
assignment.
