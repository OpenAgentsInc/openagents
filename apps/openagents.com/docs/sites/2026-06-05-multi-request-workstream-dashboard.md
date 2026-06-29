# Multi-Request Workstream Dashboard

Implemented: 2026-06-05

Issue: #172

## Summary

The customer order surface now separates the workstream dashboard from the
per-request detail view.

- `/order` is the customer's software request dashboard.
- `/orders/:orderId` remains the detail, revision, artifact, and feedback
  workflow for one request.
- `GET /api/customer-orders` returns all customer-safe software workstreams for
  the signed-in owner.
- `POST /api/customer-orders` creates another public software request from
  customer request text.
- `GET /api/customer-orders/active` still exists as the latest active-order
  fallback for older callers.

## Product Behavior

Customers can now have multiple active or delivered requests at once. The
dashboard lists Site requests, codebase/PR requests, and general software
requests as separate workstreams. Each card shows a friendly status, Adjutant
stage, next action, repository or Site context, and a link into the dedicated
request detail page.

Submitting a new request does not replace or hide any existing order. The new
request is inserted at the top of the dashboard and the composer clears after a
successful submit.

## Safety

The list endpoint reuses the existing customer-safe `CustomerOrder` projection.
It does not expose private runner payloads, provider account references,
private source refs, or secrets. Request creation inherits the customer's
onboarding repository context when available and stores the same public beta
acknowledgement defaults as the onboarding-created order path.

## Tests

Coverage added:

- API list/create/empty-request rejection in
  `workers/api/src/customer-order-routes.test.ts`.
- Dashboard rendering, Site/non-Site links, and successful request creation in
  `apps/web/src/page/loggedIn/view.scene.test.ts`.
- Startup route command assertions updated so `/order` loads
  `LoadCustomerOrders`.
