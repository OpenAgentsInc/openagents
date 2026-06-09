# Stable Slug Latest Revision Policy

Date: 2026-06-05

## Near-Term Policy

For the first customer Sites batch, the stable Site URL follows the latest
successful Adjutant-generated revision. When a saved Site artifact receipt is
recorded for an Adjutant run, the lifecycle now:

- saves the generated version;
- marks that version `customer_review_ready`, not customer accepted;
- creates or reuses a deployment for the same Site slug;
- rolls back the previous active deployment without deleting it;
- updates `site_projects.active_version_id` and `active_deployment_id`;
- records `site_deployment.superseded` and `site_deployment.activated` events;
- marks linked customer feedback as `addressed`.

The runtime at `https://sites.openagents.com/<slug>` already serves the
deployment referenced by `site_projects.active_deployment_id`, so updating that
pointer makes the stable URL show the latest revision.

## Acceptance Boundary

Deployment is not acceptance. The lifecycle writes:

- `customerReviewState = "customer_review_ready"`
- `customerAccepted = false`
- `runtimeActivationPolicy = "latest_successful_revision"`

Customer acceptance remains a separate future action. The order UI reflects
this by saying "Latest revision live" while still showing the review state.

## History And Rollback

Prior deployments remain in `site_deployments`. The active prior deployment is
changed to `rolled_back` with `rolled_back_at` set, and a Site event records the
supersession. No version or deployment evidence is deleted.

## Current Limit

This fast policy is intentionally broad for overnight iteration. A later
promotion-control issue should add staging URLs, manual hold/promote controls,
and customer approval gates before final public promotion.
