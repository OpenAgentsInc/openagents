# Resend Email Ledger Smoke Runbook

Issue: `OPENAGENTS-O-001`

This runbook verifies the current Resend/EmailService readiness boundary for
first-batch customer notifications.

## What The Smoke Proves

- Production Worker secret names include, or explicitly lack,
  `RESEND_API_KEY`, `RESEND_FROM_EMAIL`, and optional `RESEND_REPLY_TO_EMAIL`.
- `EmailService` renders the existing Adjutant customer notification template
  with escaped customer/Site values.
- `EmailService` reserves `email_messages` rows by idempotency key.
- Provider success and provider failure paths write `email_deliveries`.
- Duplicate idempotency keys do not double-send.
- Missing Resend config is treated as an explicit `email_config_missing`
  skip/failure state, not as silent success.

## Typed Order/Sites Lifecycle Email Subtypes

Issue `OPENAGENTS-O-002` adds explicit order/Sites lifecycle subtypes under the
ledger-compatible `operator_notification` kind. The D1 ledger kind remains
stable, while `template_slug`, tags, `metadata_json.emailSubtype`, and
`metadata_json.lifecycleKind` carry the product-specific lifecycle identity.

Current lifecycle subtypes:

- `order_received`
- `scoping_started`
- `repository_source_needed`
- `autopilot_queued`
- `autopilot_running`
- `review_ready`
- `site_saved_version_ready`
- `site_deployed`
- `customer_input_needed`
- `unavailable_declined`
- `delivered`
- `adjustment_received`
- `adjustment_completed`

Every subtype renders through `EmailService`, includes customer-safe status and
next action copy, links to an order/Site status surface when available, and
uses deterministic idempotency keys shaped as:

```text
order_sites_email:<lifecycleKind>:<orderId>:<assignmentId|none>:<siteId|none>:<eventRef|none>
```

Lifecycle template input is rejected before rendering if it contains
credential-shaped material, provider-account refs, auth-grant refs, raw runner
logs, private operator notes, raw Exa payload hints, `OPENCODE_AUTH_CONTENT`,
or `auth.json` references.

## Template Package And Preview

Issue `OPENAGENTS-O-005` moves order/Sites lifecycle templates into the
source-controlled `@openagentsinc/email-templates` package. The package is
Resend-compatible and Worker-safe: it uses schema-first props, renders
deterministic HTML and plain text, and avoids a runtime React dependency in the
Cloudflare Worker.

Run from the repo root:

```bash
bun run --cwd packages/email-templates preview
```

The preview command prints the currently reviewable template catalog:

- `order_sites.review_ready.v1`
- `drip.signup_day_0.v1`
- `drip.signup_day_1.v1`
- `drip.signup_day_2.v1`

The Worker `EmailService` delegates order/Sites lifecycle subject, HTML, text,
and template context rendering to that package while keeping ledger
reservation, idempotency, provider delivery, and event linkage in
`workers/api/src/email.ts`.

## Lifecycle Dispatch Wiring

Issue `OPENAGENTS-O-003` wires the typed lifecycle templates into the current
Adjutant and Sites lifecycle paths:

- Adjutant runner lifecycle callbacks for queued and running states emit
  `adjutant.notification.autopilot_queued` and
  `adjutant.notification.autopilot_running` notification events.
- Delivered callbacks send the `review_ready` lifecycle email and keep the
  existing `adjutant.notification.review_ready` event name.
- Waiting-for-input callbacks send `customer_input_needed` and keep
  `adjutant.notification.input_needed`.
- Failed/unavailable callbacks send `unavailable_declined` and keep
  `adjutant.notification.unavailable`.
- Operator Site deployments send `site_deployed` and keep
  `adjutant.notification.deployed` on both `adjutant_assignment_events` and
  `site_events`.

State transitions are authoritative before email success. Missing customer
email, missing Resend config, duplicate lifecycle events, and provider failure
do not roll back assignment, order, Site, or deployment state. When an email
ledger row exists, the relevant assignment/Site notification event stores
`email_message_id`. Provider failures store redacted `errorName` and
`errorMessage` in the private/operator event payload; customer/public
projections must not expose raw provider errors.

## Production Config And Ledger Inspection

Run from the repo root:

```bash
node scripts/operator-email-smoke.mjs --idempotencyKey operator-email-smoke:issue-114
```

The command uses Wrangler only for secret-name and D1 ledger inspection. It
does not print secret values, provider request bodies, raw customer content, or
raw provider payloads.

Safe output fields:

- config status;
- whether each Resend secret name is present or missing;
- idempotency key;
- email message ID when a matching ledger row exists;
- message status;
- provider name;
- provider message ID when safe;
- delivery ID and delivery status; and
- redacted error name.

On June 5, 2026, production returned:

```text
Email config: missing
RESEND_API_KEY: missing
RESEND_FROM_EMAIL: missing
RESEND_REPLY_TO_EMAIL: missing
Idempotency: operator-email-smoke:issue-114
Ledger rows: none
```

That means first-batch lifecycle email sends must remain operationally marked
as skipped with `email_config_missing` until production Resend secrets are set.

## Operator Delivery Inspection

Issue `OPENAGENTS-O-004` adds the browser-session operator API for inspecting
delivery state without shell-only D1 queries:

```text
GET /api/operator/email-deliveries?softwareOrderId=<software_order_id>
GET /api/operator/email-deliveries?siteId=<site_project_id>
```

The response includes:

- message kind, template slug, status, idempotency key, source-authority ref,
  and safe provider message ID;
- delivery attempt count and latest delivery status;
- skipped reason for `email_config_missing` or suppression states;
- redacted provider error name/message;
- related order IDs, Site IDs, assignment IDs, and event refs; and
- safe operator next action.

The inspection projection does not select or expose `text_body`, `html_body`,
raw template context, raw metadata JSON, Resend API keys, raw provider payload
summaries, auth grants, provider account refs, callback tokens, or secret refs.
Use it after each order/Site lifecycle transition to confirm whether customer
email was accepted, failed, intentionally skipped, or is still only reserved or
rendered.

## Resend Webhook Smoke

Issue `OPENAGENTS-O-010` adds the Resend lifecycle webhook route:

```text
POST /api/webhooks/resend
```

Configure `RESEND_WEBHOOK_SECRET` as a Worker secret before registering the
route in Resend. When configured, the route verifies the Svix signature headers
before processing the payload. Keep the webhook secret separate from the Resend
API key and do not store it in `.env.example`, docs, issue comments, or
deployment logs.

The route writes bounded `email_provider_events` summaries and updates
matching `email_deliveries` rows by Resend provider message ID. Bounce and
complaint events also write all-scope suppression records. Duplicate provider
event IDs are accepted idempotently and do not repeat delivery or suppression
side effects.

Safe operator checks:

- invalid signed payloads return `401` and write no rows;
- duplicate `svix-id` values return a duplicate result and write no repeated
  side effects;
- delivered/failed/bounced/complained events update only bounded projection
  fields; and
- webhook inspection must not expose raw provider bodies, headers, secrets, or
  rendered email content.

## Local EmailService Ledger Smoke

Run the focused EmailService smoke tests:

```bash
cd workers/api
bunx vitest run src/email.test.ts
```

The tests cover:

- present config and successful Resend response;
- missing config skip recorded as an email ledger failure;
- failed provider response recorded with redacted error state;
- idempotency preventing duplicate provider sends;
- Adjutant customer notification rendering without raw HTML leakage; and
- every order/Sites lifecycle subtype rendering with deterministic
  idempotency, customer-safe status and next action copy, and secret-shaped
  input rejection; and
- existing out-of-credits email rendering and Resend request behavior.

## Production Send Rule

Do not attempt a live production send until both required Resend secret names
are present:

- `RESEND_API_KEY`
- `RESEND_FROM_EMAIL`

`RESEND_REPLY_TO_EMAIL` is optional. If a live smoke is run later, use an
approved internal recipient and a unique idempotency key. Record only the
message ID, delivery ID, status, provider message ID if present, and redacted
error summary.
