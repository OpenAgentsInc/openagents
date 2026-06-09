# Customer Site Revision Feedback API

Date: 2026-06-05

GitHub issue: `#144`

## Purpose

Customer software orders need a review loop before full Autopilot/Adjutant
dispatch is complete. The first API slice makes Site revisions and customer
feedback durable and customer-visible from the order interface.

## Endpoints

- `GET /api/customer-orders/:orderId/site-revisions`
- `GET /api/customer-orders/:orderId/site-feedback`
- `POST /api/customer-orders/:orderId/site-feedback`

All routes require the signed-in browser session and only return data for the
order owner. Non-owned orders return `customer_order_not_found`.

## Feedback Ledger

Customer comments are stored in `site_revision_feedback`.

Each row links to:

- `software_order_id`
- optional `site_id`
- optional `site_version_id`
- optional `site_deployment_id`
- `author_user_id`
- bounded customer-safe `body`
- feedback `status`
- source and visibility metadata

This issue only records and projects feedback. Later issues promote submitted
feedback into Adjutant follow-up runs and latest-revision activation.

## Public-Safety Boundary

The customer projection does not expose provider account refs, auth grants,
raw runner payloads, callback tokens, private runner logs, source archives, or
secret values. It only exposes customer-authored feedback, revision IDs,
deployment URLs/status, source commit hashes, source hashes from bounded
metadata, and review-state labels.

## No-Site Orders

An owned order can accept feedback before a Site project exists. Those rows are
linked to the software order and have null Site/version/deployment refs. This
lets customer clarification enter the same feedback stream before the first
revision is built.
