# Email Campaign Records

Status: initial schema and repository helpers for `OPENAGENTS-O-006`.

The campaign tables model scheduling state. They do not replace the existing
`email_messages` and `email_deliveries` ledger, which remains the authority for
rendered emails, provider sends, delivery attempts, and idempotent send
results.

## Tables

- `email_campaigns`: named campaign definitions and source authority.
- `email_campaign_steps`: ordered/delayed steps with template and lifecycle
  references.
- `email_campaign_enrollments`: durable user/email membership in a campaign.
- `email_campaign_sends`: scheduled step sends with due time, status,
  idempotency key, optional `email_message_id`, and bounded error fields.
- `email_suppression_entries`: active unsubscribe, bounce, complaint, operator,
  or manual suppressions for marketing, drip, or all campaign mail.
- `email_preferences`: opt-in flags for marketing, drip, and transactional
  classes.
- `email_provider_events`: bounded provider webhook/event summaries.

## Boundary

Campaign sends are scheduling records. A dispatcher should render and send
through `EmailService`, then attach the resulting `email_message_id` to the
campaign send. Provider webhooks should write bounded `email_provider_events`
and suppression rows where appropriate.

Do not store raw provider webhook payloads, prompts, cookies, bearer tokens,
provider account refs, auth grants, callback tokens, or unbounded diagnostics
in campaign records.

## Resend Webhook Ingestion

`POST /api/webhooks/resend` ingests Resend/Svix email lifecycle events. If
`RESEND_WEBHOOK_SECRET` is configured as a Worker secret, the route verifies the
raw body against `svix-id`, `svix-timestamp`, and `svix-signature` before
decoding any payload fields. Without the secret configured, the route remains
available only for bounded smoke payloads and should not be treated as a
production-authenticated webhook.

The route records one `email_provider_events` row per provider event ID using
`ON CONFLICT(provider, provider_event_id) DO NOTHING`, so duplicate webhook
deliveries are idempotent. It stores only bounded summary fields: event type,
occurred-at time, safe provider message ID, and recipient. It does not store the
raw webhook body, headers, secret values, provider diagnostics, or rendered
email content.

Delivery events update matching `email_deliveries` rows by
`provider_message_id`. `email.delivered` marks the delivery accepted;
`email.failed`, `email.bounced`, and `email.complained` mark it failed with a
length-limited error summary. Bounce and complaint events also create
all-scope provider suppression records through the email preference boundary.

## Preference Policy

`workers/api/src/email-preferences.ts` is the policy boundary for campaign
preferences and suppression.

- Drip unsubscribe updates `email_preferences.drip_opt_in` and does not disable
  transactional order/Sites lifecycle email.
- Marketing unsubscribe updates `email_preferences.marketing_opt_in` and does
  not disable drip or transactional email.
- Transactional email has its own `transactional_opt_in` policy bit and is not
  collapsed into marketing/drip unsubscribe semantics.
- Provider bounces and spam complaints create `all`-scope suppression records.
  All-scope suppression blocks drip, marketing, and transactional sends unless
  a future legal/safety policy creates a narrower exception.
- Drip and marketing category suppression records block only their matching
  campaign categories.

Public one-click preference routes are intentionally deferred until the route
boundary can add the surface without increasing the current Worker response
budget. Until then, suppressions and unsubscribe requests should be written
through the domain helpers or operator/webhook flows, with source-authority
refs and bounded metadata.
