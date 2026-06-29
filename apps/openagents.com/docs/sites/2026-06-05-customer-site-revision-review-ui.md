# Customer Site Revision Review UI

Date: 2026-06-05

GitHub issue: `#145`

## Purpose

The customer software order page is now the first review surface for Site
revisions. When an order is loaded, the browser also loads the order's Site
revision list and customer feedback timeline.

## Customer Surface

The order detail page shows:

- the current Autopilot progress state;
- the latest active Site URL from the order projection;
- latest saved Site version metadata when present;
- a revision panel with current/prior Site versions;
- review state and deployment state for revisions;
- customer feedback history;
- a follow-up composer that posts to the customer feedback API.

The UI keeps deployment state separate from customer acceptance. A live URL can
be shown as the latest revision without implying the customer accepted it as
final work.

## Data Flow

Initial order load still uses:

- `GET /api/customer-orders/active`
- `GET /api/customer-orders/:orderId`

After a non-null order is loaded, the browser requests:

- `GET /api/customer-orders/:orderId/site-revisions`
- `GET /api/customer-orders/:orderId/site-feedback`

Submitting the composer posts:

- `POST /api/customer-orders/:orderId/site-feedback`

The returned feedback row is inserted into the visible feedback timeline and
the composer is cleared.

## Empty States

The order page handles:

- no linked Site yet;
- no Site revisions yet;
- no customer feedback yet;
- loading and failed revision/feedback requests;
- feedback submit failure.

## Verification

The implementation is covered by the logged-in scene test that renders a
customer order with a Site revision and submitted feedback, then checks the
revision panel, latest revision summary, feedback text, feedback textbox, and
submit button.
