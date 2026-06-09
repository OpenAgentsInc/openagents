# 2026-06-04 Previous Resend And Gmail Email Systems Audit

## Purpose

This audit records the email-system work that already exists across the
OpenAgents workspace, with special attention to:

- Vortex's Resend-backed prelaunch invitation flow.
- Fireball's WorkOS email-code login flow.
- The deprecated Laravel `openagents.com` CRM transactional email system.
- The local Gmail draft wrapper that uses production Laravel CRM preview data.
- The narrow Resend billing notification rail already present in OpenAgents product surface.

The goal is not to port code blindly. The goal is to preserve the useful
product and safety lessons before OpenAgents product surface grows a broader first-party email
surface.

## Source Inventory

### Vortex

Audited paths:

- `vortex/package.json`
- `vortex/lib/email/invitations.ts`
- `vortex/app/api/admin/invitations/route.ts`
- `vortex/lib/admin/invitations.ts`
- `vortex/convex/prelaunchAccess.ts`
- `vortex/convex/schema.ts`
- `vortex/app/admin/admin-invitations-panel.tsx`
- `vortex/docs/admin-email-invitations.md`

Vortex is the active core product repo for `openagents.com` UX. Its email work
is mostly an admin-controlled prelaunch invitation rail.

### Fireball

Audited paths:

- `fireball/src/worker/authService.ts`
- `fireball/src/worker/authApi.ts`
- `fireball/src/worker/authApi.test.ts`
- `fireball/src/worker.ts`
- `fireball/docs/workos-production-auth-runbook.md`
- `fireball/docs/workos-login-auth-audit.md`

Fireball does not appear to have a Resend send rail in source. It has the
generated `Resend` icon name, but the meaningful email implementation is a
WorkOS Magic Auth/email-code login service.

### Deprecated Laravel OpenAgents

Audited paths:

- `deprecated/openagents.com/composer.json`
- `deprecated/openagents.com/config/mail.php`
- `deprecated/openagents.com/config/services.php`
- `deprecated/openagents.com/routes/api.php`
- `deprecated/openagents.com/routes/web.php`
- `deprecated/openagents.com/app/Http/Controllers/Api/AdminCrmEmailController.php`
- `deprecated/openagents.com/app/Http/Controllers/AdminCrmContactEmailStoreController.php`
- `deprecated/openagents.com/app/Http/Controllers/Api/AdminCrmWritebackController.php`
- `deprecated/openagents.com/app/Http/Controllers/Api/AdminCrmSourceExportController.php`
- `deprecated/openagents.com/app/Http/Requests/AdminCrmContactEmailSendRequest.php`
- `deprecated/openagents.com/app/Services/CrmTransactionalEmailService.php`
- `deprecated/openagents.com/app/Services/CrmEmailTemplateRenderer.php`
- `deprecated/openagents.com/app/Services/CrmEmailTemplateContextFactory.php`
- `deprecated/openagents.com/app/Services/CrmActivityService.php`
- `deprecated/openagents.com/app/Mail/CrmTransactionalMessage.php`
- `deprecated/openagents.com/app/Mail/AutopilotOnboardingCompleted.php`
- `deprecated/openagents.com/app/Services/AutopilotOnboardingNotifier.php`
- `deprecated/openagents.com/config/autopilot-onboarding.php`
- `deprecated/openagents.com/database/migrations/2026_04_29_211435_create_crm_email_templates_table.php`
- `deprecated/openagents.com/database/migrations/2026_04_29_211436_create_crm_email_messages_table.php`
- `deprecated/openagents.com/database/migrations/2026_04_29_211437_create_crm_email_deliveries_table.php`
- `deprecated/openagents.com/database/migrations/2026_04_29_211455_seed_default_crm_email_templates.php`
- `deprecated/openagents.com/resources/js/pages/admin/crm/contacts/show.tsx`
- `deprecated/openagents.com/resources/views/emails/crm-transactional-message.blade.php`
- `deprecated/openagents.com/tests/Feature/CrmTransactionalEmailTest.php`
- `deprecated/openagents.com/tests/Feature/AdminCrmApiTest.php`
- `deprecated/openagents.com/docs/investor-crm-transactional-email.md`
- `deprecated/openagents.com/docs/investor-crm-sanctum-agent-runbook.md`
- `deprecated/openagents.com/docs/investor-crm-api.md`

The Laravel repo is deprecated as an implementation home, but it contains the
most complete historical CRM email design.

### Workspace Gmail Wrapper

Audited paths:

- `docs/2026-04-29-gmail-api-drafts-and-openagents-production-data-audit.md`
- `docs/2026-04-30-gws-gmail-crm-runbook.md`
- `scripts/crm-gmail.sh`

The Gmail draft work lives at the workspace level. It is intentionally local
operator tooling, not a production Laravel mail transport.

### OpenAgents product surface

Audited paths:

- `openagents/workers/api/src/config.ts`
- `openagents/workers/api/src/config.test.ts`
- `openagents/workers/api/src/email.ts`
- `openagents/workers/api/src/email.test.ts`
- `openagents/workers/api/src/billing.ts`
- `openagents/workers/api/src/index.ts`
- `openagents/workers/api/migrations/0018_billing_out_of_credits.sql`
- `openagents/docs/2026-06-03-autopilot-billing-credits.md`

OpenAgents product surface already has a narrow Resend REST sender for billing exhaustion
notifications. It does not yet have the broader CRM/invitation/Gmail draft
system described below.

## Executive Summary

There are four distinct historical email patterns, not one:

1. Vortex Resend invitations: direct Resend SDK sends for prelaunch access,
   backed by Convex invitation state.
2. Fireball WorkOS email-code login: email as authentication, not outbound
   product messaging.
3. Laravel CRM transactional email: templates, messages, deliveries, activity
   ledger, API previews, admin UI, and Blueprint writeback path.
4. Local Gmail drafts: a root-workspace wrapper that fetches production CRM
   previews from Laravel and asks `gws` to create Gmail drafts by default.

The strongest idea to carry into OpenAgents product surface is the separation of product intent,
rendered content, delivery attempt, provider result, and human approval/draft
posture. The weakest thing to carry forward would be the Laravel-era direct
synchronous `Mail::send` request path without an Effect service boundary,
durable typed result, or source-authority receipt.

OpenAgents product surface should treat email as a first-class domain with:

- typed Effect services and layers;
- schema-decoded provider responses;
- redacted config and observability;
- D1 records for logical messages and delivery attempts;
- explicit idempotency keys;
- separate draft and send capabilities;
- no production Gmail OAuth token storage until there is a deliberate product
  need.

## Vortex Resend Invitation System

### What It Does

Vortex uses the Node `resend` package for prelaunch invitations. The package is
declared in `vortex/package.json`, and the sender lives in
`vortex/lib/email/invitations.ts`.

The flow is:

1. Admin calls `/api/admin/invitations`.
2. `app/api/admin/invitations/route.ts` authenticates the admin through
   `authenticateAdminRequest`.
3. The request email is normalized with `normalizePrelaunchEmail`.
4. `upsertAdminInvitation` creates or reactivates a Convex
   `prelaunchApprovedEmails` row.
5. Unless `sendEmail: false` is supplied, the route calls
   `sendInvitationEmail`.
6. Delivery result is recorded back to Convex with
   `recordAdminInvitationDelivery`.

The email itself is a one-off invite:

- `subject`: `Your OpenAgents invitation`
- `to`: the invited address
- `from`: `RESEND_FROM_EMAIL`
- `replyTo`: optional `RESEND_REPLY_TO_EMAIL`
- text and HTML bodies are generated in code
- HTML output escapes dynamic fields
- Resend tags include category `prelaunch_invite`
- idempotency key shape is
  `prelaunch-invite/<convex-row-id>/<updated-at>`

### State Model

Convex table `prelaunchApprovedEmails` carries both access policy and
invitation delivery state:

- `email`
- `status`: `active` or `revoked`
- `note`
- `invitedAt`
- `invitedByEmail`
- `lastInvitationError`
- `lastInvitationSentAt`
- `lastInvitationStatus`: `pending`, `sent`, or `failed`
- `lastResendEmailId`
- `revokedAt`
- `revokedByEmail`
- timestamps

The important thing is that the invitation row is durable and user-visible in
admin tooling. Email delivery is not a transient side effect hidden in a log.

### Admin Surface

`vortex/app/admin/admin-invitations-panel.tsx` makes this operational:

- list active/revoked rows;
- invite and send;
- invite without send;
- resend by submitting the same email;
- revoke Convex-managed invites;
- show delivery status and Resend email id in operator feedback.

The runbook `vortex/docs/admin-email-invitations.md` documents the admin API,
required Vercel and Convex env vars, Resend sender requirements, idempotency,
and a production smoke path using a Resend test recipient.

### Good Patterns To Preserve

- Email send is server-only.
- Dynamic copy is escaped before embedding in HTML.
- Provider API key comes from env only.
- Provider id is stored on durable state.
- Delivery failure is recorded in product state, not just thrown.
- There is an explicit approve-without-send mode.
- Resend idempotency is used for the actual provider call.
- Admin bearer access exists for trusted repair scripts without browser
  cookies.

### Problems Or Limits

- Invitation access state and email delivery status share one table. That is
  fine for prelaunch, but less ideal for a general email system.
- Only last delivery is stored. There is no append-only delivery-attempt table.
- The error string is stored directly in Convex state. That is useful for
  operators but should be length-limited, classified, and redacted in OpenAgents product surface.
- The idempotency key includes the row id and updated timestamp. It is short
  and practical, but it is not a typed domain object.
- The sender is a standalone async function, not an Effect service/layer.
- The API route catches broad errors and maps to generic JSON. OpenAgents product surface should
  use typed error variants and a centralized HTTP mapper.

## Fireball Email-Code Auth System

### What It Does

Fireball's email system is authentication, not outbound marketing or CRM mail.
It wraps WorkOS Magic Auth/email-code login behind an Effect service:

- `AuthEmailCodeServiceShape`
- `AuthEmailCodeService`
- `authEmailCodeServiceLayer`
- `AuthEmailCodeServiceFake`
- `authEmailCodeServiceLiveLayer`

The service methods are:

- `startEmailLogin(email)`
- `verifyEmailLogin(email, code)`
- `logoutSession(session)`
- `refreshSessionCookie(cookieValue)`

The live layer uses WorkOS:

- `createMagicAuth({ email })`
- `authenticateWithMagicAuth({ email, code, session: { sealSession: true } })`
- sealed session cookie refresh and revoke paths

The fake layer uses deterministic code `123456` for local development and
tests.

### HTTP Boundary

`fireball/src/worker/authApi.ts` exposes:

- `GET /api/auth/session`
- `POST /api/auth/refresh`
- `POST /api/auth/start`
- `POST /api/auth/verify`
- `POST /api/auth/logout`

Notable safety details:

- auth mutations require an `x-openagents-auth-action` style action header;
- start normalizes email;
- start sanitizes `returnTo` to same-origin paths;
- pending auth is sealed in an HTTP-only cookie;
- verify enforces a six-digit code format before hitting WorkOS;
- production mode checks `isApprovedProductionUser`;
- browser responses carry only `BrowserSafeAuthSession`, not WorkOS raw tokens;
- logout clears both session and pending cookies.

### Tests And Runbook

`fireball/src/worker/authApi.test.ts` verifies:

- normalized email and pending-cookie behavior;
- default return target;
- unconfigured state;
- CSRF/action-header enforcement;
- invalid email and invalid return target states;
- fake rate limit state;
- successful verify sets session and clears pending state;
- invalid verify has a typed state;
- logout clears cookies.

`fireball/docs/workos-production-auth-runbook.md` documents WorkOS setup,
Cloudflare secrets, fake-auth boundaries, deploy checks, smoke commands, and
typed expected API responses.

### Good Patterns To Preserve

- Email-code operations are behind an Effect service and injected via layers.
- The fake service is explicitly non-production and test-oriented.
- Provider secrets and raw provider responses do not cross into the browser.
- Mutating auth routes have explicit action headers.
- Failure modes are typed product states, not only exceptions.
- The runbook names secrets without printing values.

### Problems Or Limits

- This is not a general email transport. It should not be reused for Resend or
  Gmail sends except as a service/layer pattern.
- Rate limiting in the fake layer is deterministic test behavior. Production
  throttling belongs in Worker policy or provider settings.
- There is no reusable email-delivery ledger because WorkOS owns the email send.

## Deprecated Laravel CRM Transactional Email System

### What It Does

The Laravel system is the richest previous email implementation. It models CRM
transactional email as durable business data:

- templates;
- rendered previews;
- message records;
- delivery records;
- engagement/activity projection;
- source export with redaction;
- admin UI;
- API routes;
- Blueprint writeback compatibility;
- optional Resend transport through Laravel Mail config.

Composer includes `resend/resend-php`. `config/mail.php` defines a `resend`
mailer transport. `config/services.php` reads `RESEND_API_KEY` for Laravel's
Resend mail transport. The default mailer remains env-controlled and defaults
to `log`, so Resend activation depends on `MAIL_MAILER=resend` plus a real
key.

### Data Model

The migrations create:

`crm_email_templates`

- `slug`
- `name`
- `description`
- `subject_template`
- `body_markdown_template`
- `body_html_template`
- `available_variables`
- `status`
- creator/updater user ids

`crm_email_messages`

- `contact_id`
- `account_id`
- `template_id`
- `sent_by_user_id`
- `from_email`
- `to_email`
- `subject`
- `body_markdown`
- `body_html`
- `template_context`
- `send_reason`
- `status`
- `sent_at`
- `delivered_at`
- `opened_at`
- `first_clicked_at`
- `replied_at`
- provider thread/message ids

`crm_email_deliveries`

- `email_message_id`
- `provider_name`
- `provider_message_id`
- `delivery_status`
- `attempted_at`
- `completed_at`
- `provider_payload`
- `error_message`

This is the best historical shape for OpenAgents product surface to study. It separates the logical
message from individual provider attempts and leaves space for future delivery,
open, click, reply, and thread metadata.

### Seeded Template

`2026_04_29_211455_seed_default_crm_email_templates.php` seeds
`investor-portal-follow-up`.

It renders live variables including:

- `contact.first_name`
- `investor.roster_count`
- `investor.fund_count`
- `investor.portal_url`
- `investor.data_room_url`
- `account.name`
- `relationship_stage`

The template is intentionally operator-oriented, not a campaign engine.

### Rendering

`CrmEmailTemplateContextFactory` builds context from:

- CRM contact fields;
- account fields;
- app config;
- investor roster summary;
- investor portal/data room URLs;
- relationship stage.

`CrmEmailTemplateRenderer` performs simple `{{ path.to.value }}`
substitution using `Arr::get`, renders a markdown body, and derives HTML with
`Str::markdown` when an explicit HTML template is absent.

The important reusable idea is not the regex itself. The reusable idea is that
rendering is a named boundary that can be called independently for preview,
send, Gmail draft creation, and writeback dry runs.

### Web/Admin Route

The web admin route:

- `POST /admin/crm/contacts/{contact}/emails`

The React contact page shows:

- active templates;
- rendered subject;
- editable body;
- send reason;
- available variables as chips;
- disabled send button in read-only mode.

The direct send route is guarded by `crm.direct-mutations`. In read-only CRM
mode the UI says to use Autopilot/Blueprint writebacks instead, and the test
asserts that no email rows are created and no mail is sent.

### API Routes

Sanctum API routes live under:

- `GET /api/admin/crm/email-templates`
- `POST /api/admin/crm/contacts/{contact}/email-preview`
- `GET /api/admin/crm/contacts/{contact}/emails`
- `POST /api/admin/crm/contacts/{contact}/emails`

Abilities split read/preview from send:

- `crm:read` for template reads and previews;
- `crm:email:send` for direct sends;
- `crm:writeback` for Blueprint writebacks.

This split matters. Gmail draft creation can safely use the preview endpoint
without requiring direct-send authority.

### Send Service

`CrmTransactionalEmailService::send`:

1. creates a `crm_email_messages` row with status `queued`;
2. calls `Mail::to(...)->send(new CrmTransactionalMessage(...))`;
3. on success, marks the message `sent`, fills `sent_at` and `delivered_at`;
4. creates a `crm_email_deliveries` row with `provider_name` from
   `config('mail.default')`;
5. on failure, marks the message `failed`, creates a failed delivery row with
   `error_message`, then rethrows;
6. records a CRM activity through `CrmActivityService`.

The mailable view is a raw `{!! $bodyHtml !!}` Blade render. That is acceptable
only because the body is operator/template-controlled in this old app. OpenAgents product surface
should not preserve this raw-render pattern without a stronger sanitization and
trusted-template boundary.

### Blueprint Writeback Path

`AdminCrmWritebackController` supports `transactional-email-send`.

The writeback path requires Blueprint/source-authority metadata in the broader
controller path and supports `dry_run`. In dry run, it returns:

- target contact id;
- status `dry_run_passed`;
- external state `not_sent`;
- template id;
- destination email;
- subject;
- body markdown;
- render context.

In live mode it calls the same transactional email service, records an
activity, and returns a message payload.

This is the strongest historical bridge between "agent proposes an action" and
"external side effect happens only through an approved, idempotent writeback."

### Source Export Redaction

`AdminCrmSourceExportController` treats:

- `email_messages` as `derived_sensitive`, redacting `body_markdown` and
  `body_html`;
- `email_deliveries` as `derived_sensitive`, redacting `provider_payload` and
  `error_message`.

This is exactly the posture OpenAgents product surface should keep. Email bodies and provider errors
can contain private recipient data, private prompts, quoted threads, unsubscribe
tokens, URLs, or provider diagnostics.

### Tests

`CrmTransactionalEmailTest` verifies:

- preview renders live investor roster counts;
- admin can send a transactional email;
- message and delivery records are created;
- Laravel `Mail` sends the expected mailable;
- direct transactional sends are blocked in read-only CRM mode.

`AdminCrmApiTest` verifies the API/writeback path, including approved email
writebacks and `Mail::assertSent`.

### Good Patterns To Preserve

- Preview is a first-class API separate from send.
- Send authority is separate from read authority.
- Logical message records and provider delivery records are separate.
- Activity projection is triggered after send.
- Direct mutation mode can be turned off.
- Writeback path has dry-run support.
- Source export redacts email bodies and provider payloads.
- The Gmail wrapper can consume the preview path without needing a send token.

### Problems Or Limits

- The service is synchronous and request-bound.
- Provider interaction is hidden behind Laravel Mail, which makes provider ids
  and idempotency harder to reason about.
- The raw Blade view prints trusted HTML without a typed trusted-template
  wrapper.
- Template rendering is regex/string-based and not schema-validated.
- There is no append-only event table for template render, draft created,
  delivery attempted, delivery failed, or delivery accepted.
- Direct send can exist when CRM admin mode is transitional.
- Provider error messages are stored directly. They are redacted from export,
  but OpenAgents product surface should classify and length-limit them at write time.
- Gmail draft ids are not part of the Laravel model because Gmail draft tooling
  lives outside the app.

## Deprecated Laravel Autopilot Onboarding Notification

Laravel also had a simpler notification rail for onboarding completions:

- `config/autopilot-onboarding.php`
- `AutopilotOnboardingCompleteController`
- `AutopilotOnboardingNotifier`
- `AutopilotOnboardingCompleted` mailable

When an onboarding session completes:

1. the session is marked completed;
2. a spec is generated;
3. `AutopilotOnboardingNotifier` sends a mailable to configured
   `AUTOPILOT_ONBOARDING_NOTIFY_EMAILS`;
4. `notification_sent_at` and `notification_recipients` are stored.

This is less architecturally rich than the CRM email system, but it captures a
separate product use case: operator notifications. OpenAgents product surface should not mix these
with CRM/customer transactional messages. They deserve a separate kind such as
`operator_notification`.

## Local Gmail Draft Wrapper

### What It Does

The local Gmail work is not a production app feature. It is a workspace-level
operator tool:

- `docs/2026-04-29-gmail-api-drafts-and-openagents-production-data-audit.md`
- `docs/2026-04-30-gws-gmail-crm-runbook.md`
- `scripts/crm-gmail.sh`

The script:

1. reads `.secrets/openagents-com-crm-production.env`;
2. uses `OPENAGENTS_COM_CRM_BASE_URL` and `OPENAGENTS_COM_CRM_TOKEN`;
3. resolves a CRM contact by contact id or email;
4. resolves a template by id, slug, or first active template;
5. calls the production CRM email preview endpoint;
6. prefers the preview HTML body when available;
7. invokes `gws gmail +send`;
8. defaults to `--draft`;
9. only sends immediately if `--send` is explicitly supplied.

`gws` is used as the Gmail OAuth/API execution layer. The wrapper falls back to
`npx -y @googleworkspace/cli` if `gws` is not on `PATH`.

### Safety Model

The strategy document correctly chooses draft-first mode:

- Gmail account remains user-controlled.
- Drafts are reviewable in Gmail before send.
- Gmail OAuth tokens stay local rather than in production.
- The production CRM API remains source of truth for contact/template/context.
- Direct live send requires an explicit mode switch.

This split was right for experimentation. It avoided turning a local personal
mailbox into a production app dependency.

### What The Wrapper Does Not Do

The current wrapper does not:

- write Gmail draft ids back to CRM;
- store Gmail message ids;
- store Gmail thread ids;
- create `gmail_draft_created` CRM activities;
- sync sent/reply state back to CRM;
- enforce Blueprint writeback metadata;
- encode a typed approval receipt;
- run as an Effect service;
- keep secrets anywhere but the local workspace secret file.

Those omissions are fine for local operator tooling. They would not be fine for
a production OpenAgents product surface user-facing Gmail integration.

## OpenAgents product surface Current Email Surface

OpenAgents product surface already has one narrow email rail:

- `workers/api/src/config.ts` parses optional Resend config with Effect.
- `RESEND_API_KEY` is stored as `Redacted`.
- `RESEND_FROM_EMAIL` and `RESEND_REPLY_TO_EMAIL` are validated as email-like
  sender values.
- `workers/api/src/email.ts` sends out-of-credits emails through Resend REST.
- `workers/api/src/email.test.ts` covers HTML escaping, idempotency headers,
  reply-to shape, and malformed provider errors.
- `billing_credit_notifications` stores one-time out-of-credits notification
  reservation, delivery id, and failure message.
- `sendOutOfCreditsNotificationOnce` reserves first, sends second, then marks
  sent or failed.

This is already closer to the desired OpenAgents product surface style than the Laravel version:

- no SDK dependency needed;
- no raw provider key leaves the config boundary;
- Resend responses are schema-decoded;
- error payloads are normalized;
- idempotency key is explicit;
- delivery is tied to a durable D1 reservation.

But it is still narrow:

- only one email type exists;
- there is no general `EmailService`;
- there is no reusable `email_messages` / `email_deliveries` table;
- there is no template registry;
- there is no draft concept;
- there is no operator notification kind;
- there is no Resend event webhook ingestion;
- send logic is still called directly from the Worker route surface rather than
  through a fully layered domain service.

## Cross-System Lessons

### 1. Keep Intent, Content, Attempt, And Delivery Separate

The Laravel CRM model had the right separation:

- template says what can be rendered;
- preview says what would be sent;
- message says what the product intended to send;
- delivery says what the provider accepted or rejected.

OpenAgents product surface should avoid one-table shortcuts once email grows beyond billing alerts.

### 2. Preview Is Safer Than Send

The Gmail wrapper succeeded because it consumed preview output, not direct-send
output. The same rule should hold in OpenAgents product surface:

- preview/render can be low-risk read authority;
- draft creation is a controlled intermediate side effect;
- send is a higher-authority external action.

### 3. Gmail Should Stay Local Until There Is A Product Need

The previous Gmail design intentionally kept OAuth tokens local. That was the
right blast-radius decision.

OpenAgents product surface should not put Gmail OAuth tokens into Cloudflare secrets or D1 just to
preserve old experimentation. If Gmail becomes a product feature, it needs a
real provider-account model, scoped OAuth, refresh handling, revocation,
receipting, and user-facing account state.

### 4. Resend Is Better For Product Emails

Resend is appropriate for:

- invitations;
- billing alerts;
- operator notifications;
- transactional product mail;
- possibly email-code auth if OpenAuth needs a `CodeProvider`.

Gmail is appropriate for:

- local operator drafts;
- human-owned one-off outreach;
- draft review in a real mailbox;
- experimental CRM follow-up composition.

Do not collapse those into one transport.

### 5. Provider Idempotency Is Not Enough

Vortex and OpenAgents product surface both use provider idempotency keys. Laravel writebacks use
application idempotency keys.

OpenAgents product surface needs both:

- application idempotency to avoid duplicate domain records;
- provider idempotency to avoid duplicate sends when retrying provider calls.

### 6. Email Bodies Are Sensitive

Laravel's source export redaction is a strong precedent. OpenAgents product surface should treat
email bodies, rendered HTML, provider payloads, and provider errors as
derived-sensitive material by default.

### 7. Direct Send Should Be Hard To Reach

Laravel's read-only CRM mode and Blueprint writeback transition were the right
direction. OpenAgents product surface should not expose casual direct send surfaces without:

- explicit capability;
- source authority;
- approved action metadata;
- idempotency;
- dry run;
- durable receipt.

## Recommended OpenAgents product surface End State

### Domain Types

Introduce an email domain package or Worker module around schemas like:

- `EmailAddress`
- `EmailSender`
- `EmailTemplateId`
- `EmailMessageId`
- `EmailDeliveryId`
- `EmailDraftId`
- `EmailProvider`
- `EmailKind`
- `EmailIntentStatus`
- `EmailDeliveryStatus`
- `EmailDraftStatus`

Suggested `EmailKind` values:

- `prelaunch_invitation`
- `billing_out_of_credits`
- `operator_notification`
- `crm_transactional`
- `provider_auth_code`

Suggested delivery statuses:

- `reserved`
- `rendered`
- `queued`
- `accepted`
- `failed`
- `unknown_external_state`

Suggested draft statuses:

- `draft_requested`
- `draft_created`
- `draft_failed`
- `sent_from_draft`
- `abandoned`

### D1 Tables

OpenAgents product surface should generalize the billing notification table into a richer email
ledger when a second email kind appears:

`email_templates`

- id
- kind
- slug
- name
- subject template
- text template
- html template
- variable schema version
- status
- created/updated timestamps

`email_messages`

- id
- kind
- actor id
- target user/contact id
- from email
- to email
- subject
- text body
- html body
- template id
- template context JSON
- idempotency key hash
- source authority ref
- action submission id
- status
- created/updated timestamps

`email_deliveries`

- id
- message id
- provider
- provider message id
- provider thread id
- provider request id
- provider idempotency key hash
- status
- attempted/completed timestamps
- redacted error classification
- redacted provider payload summary

`email_drafts`

- id
- message id
- provider
- provider draft id
- provider message id
- provider thread id
- status
- created/updated/sent timestamps
- local/operator provenance where applicable

For billing-only, the current `billing_credit_notifications` table can remain
until another reusable kind justifies migration.

### Effect Services

Create a typed service boundary:

```text
EmailTemplateService
  render(template, context) -> RenderedEmail

EmailDeliveryService
  reserve(intent) -> EmailMessage
  send(message) -> EmailDeliveryResult
  markAccepted(...)
  markFailed(...)

ResendEmailTransport
  send(rendered, metadata) -> ProviderAccepted | ProviderRejected

GmailDraftTransport
  createDraft(rendered, metadata) -> DraftCreated | DraftRejected
```

The services should be Effects, not raw promises. Provider fetches should be
inside Effect boundaries and should return typed error variants.

### Provider Config

Keep the current OpenAgents product surface config posture:

- parse env once;
- validate sender/reply-to;
- store API keys as `Redacted`;
- expose only typed config to services;
- log only redacted classifications.

Add config only when needed:

- Resend production config for product email.
- Gmail local config should remain outside Worker config unless Gmail becomes a
  production provider account.

### Resend Transport

The existing `sendOutOfCreditsEmail` REST approach is a good base. Generalize
it rather than introducing the SDK by default:

- accept rendered text/html;
- add tags per domain kind;
- use `Idempotency-Key`;
- decode response with Effect Schema;
- classify provider errors;
- length-limit stored messages;
- never log API key or full provider payload.

### Gmail Draft Transport

Do not put Gmail draft creation in the Cloudflare Worker yet.

Recommended near-term shape:

- keep local `gws` wrapper as operator tooling;
- if OpenAgents product surface needs to generate preview content, expose a read-only preview API;
- let the local tool create Gmail drafts from that preview;
- optionally add an operator-only writeback endpoint to record:
  - draft id;
  - message id if available;
  - thread id if available;
  - selected template;
  - local operator identity;
  - timestamp;
  - source intent id.

Only move Gmail into production when there is a real provider-account product
surface.

### Policy

Every external email send should answer:

- who requested it;
- who approved it;
- what source authority allowed it;
- what rendered body hash was approved;
- what recipient was approved;
- what idempotency key protects it;
- whether this is draft-only or send-now;
- where the receipt is stored.

For draft-only local Gmail, the answer can be a local operator note plus a
writeback record. For production sends, it must be durable inside OpenAgents product surface.

## Concrete Porting Recommendations

### Port From Vortex

- Resend idempotency key usage.
- Admin repair path that can approve without sending.
- Durable invitation delivery status.
- HTML escaping for dynamic fields.
- Test-recipient smoke concept.

Do not port:

- one-row-only delivery history for reusable email;
- generic thrown errors from env readers;
- plain Promise sender as the final OpenAgents product surface architecture.

### Port From Fireball

- Effect service/layer boundary.
- Fake service for tests.
- Typed mutation states.
- Runbook style that names secrets without revealing values.
- Browser-safe projection discipline.

Do not port:

- WorkOS-specific email-code concepts into product email;
- fake deterministic auth as a production fallback.

### Port From Laravel

- `email_templates`, `email_messages`, `email_deliveries` conceptual split.
- Preview endpoint before send.
- Separate read, send, export, and writeback authorities.
- Source export redaction for bodies/provider payloads.
- Dry-run writeback for external sends.
- Activity projection after send.
- Read-only/direct-mutation kill switch.

Do not port:

- synchronous request-bound sending as the long-term Worker shape;
- raw Blade `{!! $bodyHtml !!}` trust model;
- regex-only template rendering without schema/versioning;
- direct send routes that bypass action submission;
- provider errors stored as raw strings with no classification.

### Preserve From Gmail Wrapper

- Draft-first default.
- Local OAuth and local token storage.
- Use production CRM preview output rather than direct template duplication.
- Explicit `--send` opt-in.
- HTML preference for rich Gmail drafts.

Do not move to OpenAgents product surface yet:

- Gmail OAuth tokens;
- `gws` shell invocation inside Worker;
- Gmail as a product dependency without provider-account UX.

### Keep From OpenAgents product surface

- Redacted config.
- Effect Schema provider response decoding.
- HTML escaping tests.
- Resend REST sender with explicit idempotency header.
- Durable reservation before send.
- Mark sent/failed after provider result.

Need to improve:

- wrap the current sender in an Effect service;
- move current billing email into reusable email transport once another kind
  exists;
- centralize provider error classification;
- add general delivery attempt records before multiple email kinds appear.

## Suggested Implementation Sequence For OpenAgents product surface

### 2026-06-04 OpenAgents product surface Implementation Pass

OpenAgents product surface now has the first concrete implementation of the recommended end state,
excluding the WorkOS/auth-code pieces by design:

- `workers/api/src/email.ts` defines `EmailService` as a typed Effect service
  with schemas for email ids, providers, kinds, statuses, rendered messages,
  provider-accepted/provider-rejected results, and service errors.
- The old billing sender surface is preserved as a compatibility wrapper, but
  its Resend transport now flows through the Effect service.
- Billing out-of-credits notifications now call
  `sendOutOfCreditsEmailWithLedger`, which reserves a general email message,
  sends through the reusable Resend transport, marks the message accepted or
  failed, and records a delivery attempt.
- `workers/api/migrations/0026_email_ledger.sql` adds `email_templates`,
  `email_messages`, `email_deliveries`, and `email_drafts`. This implements
  the Laravel-style template/message/delivery/draft separation without
  reviving deprecated Laravel code.
- Gmail is represented only as a future ledger provider/draft-recording path.
  The Worker does not store Gmail OAuth material and does not execute `gws`.
- `INVARIANTS.md` now records the typed email side-effect policy: every
  production email send must pass through `EmailService`, use idempotency and
  source-authority metadata, persist durable message/delivery state, and store
  only classified, length-limited provider error summaries.
- `workers/api/src/email.test.ts` covers the existing render/escape/request
  behavior plus Effect-service rendering and D1 message/delivery recording.

Remaining follow-up work is product-surface dependent: adding an invitation,
operator notification, CRM transactional, or local Gmail-draft writeback flow
can now reuse the service and ledger instead of introducing another direct
provider call.

### Production Persistence Confirmation

The 2026-06-04 implementation is designed to persist email data to the
Cloudflare D1 production database, but only after the new migration is applied
to the remote database.

OpenAgents product surface's Worker config binds `OPENAGENTS_DB` to the Cloudflare D1 database named
`openagents-autopilot`, using migration files from `workers/api/migrations`.
Cloudflare D1 migrations are tracked in the database's migration table when
applied, and Wrangler's D1 migration command is the mechanism that applies
unapplied migration files to the remote database. See:

- Cloudflare D1 migrations:
  https://developers.cloudflare.com/d1/reference/migrations/
- Wrangler D1 migration commands:
  https://developers.cloudflare.com/workers/wrangler/commands/d1/#d1-migrations-apply

Current persistence facts:

- `workers/api/migrations/0026_email_ledger.sql` creates
  `email_templates`, `email_messages`, `email_deliveries`, and
  `email_drafts`.
- `EmailService.sendOutOfCreditsEmailWithLedger` writes a rendered
  `email_messages` row before the Resend call and an `email_deliveries` row
  after the provider result.
- `email_messages` stores recipient, sender, reply-to, subject, text body,
  HTML body, template slug/context JSON, idempotency key, source authority,
  action submission id, status, provider, provider message/draft/thread ids,
  and classified error fields.
- `email_deliveries` stores provider, provider message id, provider
  idempotency key, delivery status, attempted/completed times, classified
  error fields, and a provider payload summary.
- `email_drafts` stores future Gmail/local draft identifiers and provenance,
  but OpenAgents product surface does not currently create Gmail drafts in the Worker.

Important deployment caveat: the current `workers/api` deploy script runs
checks, builds web assets, and runs `wrangler deploy`; it does not currently
run `wrangler d1 migrations apply OPENAGENTS_DB --remote`. Therefore, a code
deploy alone does not guarantee the production D1 schema contains
`email_messages` and related tables. The required production step is:

```bash
cd workers/api && bunx wrangler d1 migrations apply OPENAGENTS_DB --remote
```

or an equivalent package script added before Worker deploy. Without that
remote migration application, the Worker code will attempt to write to tables
that do not exist in production.

What is not persisted yet:

- Resend webhook events such as `email.sent`, `email.delivered`,
  `email.opened`, `email.clicked`, `email.bounced`, `email.complained`, or
  `email.suppressed`.
- Open counts, click counts, first/last opened timestamps, first/last clicked
  timestamps, click URLs, IP/user-agent metadata, bounce details, or complaint
  metadata.
- Provider webhook raw payloads. This is intentional until a retention and
  redaction policy exists because webhook payloads may contain personal data.

### Open And Click Tracking Requirements

Resend supports open and click tracking, but OpenAgents product surface must add both provider
configuration and an ingestion surface before engagement data lands in D1.

Resend tracking setup:

- Enable open and/or click tracking for the Resend sending domain.
- Configure and verify a custom tracking subdomain, for example
  `links.openagents.com` or `links.emails.openagents.com`, with the CNAME
  record Resend provides in Cloudflare DNS.
- Resend documents that tracking is active only when tracking is enabled for
  the domain and the tracking subdomain is successfully verified:
  https://resend.com/docs/dashboard/domains/tracking
- Open tracking uses an inserted 1x1 transparent image. Click tracking rewrites
  HTML links to the tracking subdomain, records the click, and redirects to the
  original URL.

OpenAgents product surface callback setup:

- Yes, OpenAgents product surface needs a callback endpoint if we want open/click events stored in
  the production database.
- The recommended endpoint is
  `POST https://openagents.com/api/webhooks/resend`.
- This endpoint should not require a browser session. It should authenticate
  by verifying Resend's Svix signature headers against a Worker secret such as
  `RESEND_WEBHOOK_SECRET`.
- Resend's webhook docs require using the raw request body for signature
  verification because parsing/re-stringifying the JSON changes the signed
  payload:
  https://resend.com/docs/webhooks/verify-webhooks-requests
- Configure the webhook in Resend for at least:
  `email.sent`, `email.delivered`, `email.delivery_delayed`, `email.failed`,
  `email.bounced`, `email.complained`, `email.suppressed`, `email.opened`,
  and `email.clicked`.
- Resend webhook creation returns a signing secret that must be stored as a
  Cloudflare Worker secret, not committed:
  https://resend.com/docs/api-reference/webhooks/create-webhook
- Resend retries webhook delivery with exponential backoff and supports manual
  replay, so the handler must be idempotent:
  https://resend.com/docs/webhooks/retries-and-replays

Recommended D1 extension:

- Add `email_events` as an append-only provider event table:
  - `id`
  - `provider`
  - `webhook_message_id` from `svix-id`, unique
  - `event_type`
  - `event_created_at`
  - `provider_email_id`
  - `message_id` nullable, resolved by `email_messages.provider_message_id`
  - `delivery_id` nullable
  - `recipient_email`
  - `subject`
  - `link_url` for click events
  - `ip_address_hash` or redacted IP field, not raw long-term IP by default
  - `user_agent_hash` or redacted user-agent field
  - `tags_json`
  - `provider_payload_summary_json`
  - `received_at`
- Add summary columns to `email_messages` for product queries:
  - `sent_at`
  - `delivered_at`
  - `failed_at`
  - `bounced_at`
  - `complained_at`
  - `first_opened_at`
  - `last_opened_at`
  - `open_count`
  - `first_clicked_at`
  - `last_clicked_at`
  - `click_count`
  - `last_event_at`
- Update `email_deliveries.status` as provider events arrive. The current
  service marks a message `accepted` when Resend accepts the API request, but
  provider webhooks should distinguish API acceptance from downstream delivery,
  bounce, complaint, suppression, and delay.

Recommended Effect service work:

- Add `ResendWebhookEvent` schemas for the supported event payloads.
- Add `EmailWebhookService` or extend `EmailService` with
  `ingestResendWebhook(rawBody, headers)`.
- Verify Svix signatures before JSON decoding.
- Decode event payloads with Effect Schema.
- Insert into `email_events` idempotently by `svix-id`.
- Resolve `data.email_id` to `email_messages.provider_message_id`.
- Update message summary columns and delivery status in one D1 batch.
- Return `2xx` only after the event is safely persisted. Return `400` for
  invalid signatures or invalid schema, and `5xx` only for retryable storage
  failures.

Privacy and analytics notes:

- Opens are directional, not ground truth. Resend notes open rates are not
  always accurate; open tracking depends on image loading and can be affected
  by privacy proxies, blocked images, previews, and plain-text-only views:
  https://resend.com/docs/webhooks/emails/opened
- Clicks are generally stronger engagement signals, but click webhook payloads
  can include link, IP address, and user agent data:
  https://resend.com/docs/webhooks/emails/clicked
- Resend recommends storing at minimum event id, event type, timestamp, and
  email id, with optional recipient, subject, tags, bounce details, and click
  URLs for deeper analytics:
  https://resend.com/docs/dashboard/webhooks/how-to-store-webhooks-data
- OpenAgents product surface should choose a retention policy before storing raw webhook payloads.
  A good default is to store normalized event rows and redacted summaries in
  D1, then add longer-term aggregates if campaign analytics grows.

1. **Document Email Policy**
   - Add an OpenAgents product surface invariant for email side effects:
     "production email sends require typed intent, idempotency, source
     authority for non-system sends, and durable delivery status."

2. **Extract Resend Transport**
   - Move `sendOutOfCreditsEmail` into a reusable `ResendEmailTransport`.
   - Keep the current billing text/html renderer as a domain-specific template.
   - Preserve existing tests.

3. **Create Email Domain Schemas**
   - Add schemas for rendered email, provider result, provider error, and
     delivery status.
   - Avoid generic stringly statuses.

4. **Generalize Delivery Ledger**
   - If another email kind lands, add `email_messages` and `email_deliveries`.
   - Migrate or mirror billing notifications into the new ledger only when
     needed.

5. **Add Preview-Only Template Boundary**
   - Implement render-only API for any CRM/invitation style future flow.
   - Do not let preview endpoints send.

6. **Add Draft Recording Before Gmail Sending**
   - If the local Gmail wrapper is revived for OpenAgents product surface, have it call an
     operator-only writeback endpoint after draft creation.
   - Store draft id/thread id if `gws` returns them.

7. **Add Approval Receipts For Send**
   - Any user/customer/investor send should carry action submission and approval
     metadata.
   - System billing notices can be policy-approved by system rule, but should
     still record the rule name/version.

8. **Add Provider Webhook Handling Later**
   - Resend delivery/open/click webhooks can update `email_deliveries` and
     `email_messages`.
   - Keep webhook payloads redacted and schema-decoded.

## Verification Plan

For any OpenAgents product surface port, test at these layers:

- Config tests:
  - missing Resend config returns no transport;
  - partial Resend config fails with typed config error;
  - malformed sender/reply-to fails;
  - secrets stay redacted.

- Template tests:
  - text and HTML render expected URLs;
  - dynamic values are escaped in HTML;
  - unsupported variables fail or render as explicit blanks by policy.

- Transport tests:
  - Resend request includes idempotency header;
  - tags are correct for each email kind;
  - provider accepted response maps to `accepted`;
  - malformed error maps to classified fallback;
  - network error maps to retryable/nonretryable policy.

- Ledger tests:
  - reserve before send;
  - duplicate idempotency returns existing record;
  - success records provider id;
  - failure records classification without raw secret or unbounded payload;
  - bodies and provider payloads are redacted from export.

- Policy tests:
  - preview does not require send authority;
  - send requires explicit capability;
  - non-system sends require source-authority/action-submission metadata;
  - direct send is disabled when policy flag says draft/writeback-only.

- Gmail wrapper tests:
  - dry-run does not create Gmail draft;
  - default mode is draft;
  - `--send` is required for live send;
  - script refuses missing contact/email;
  - script refuses missing secret file;
  - script uses preview HTML by default when available.

## Open Questions

1. Should OpenAgents product surface ever own customer/outreach email directly, or should customer
   email stay in Vortex/CRM surfaces?
2. Should invitation email move from Vortex to OpenAgents product surface, or should OpenAgents product surface only
   consume authenticated users after invitation/prelaunch access is resolved?
3. Should Gmail drafts remain purely local, or should OpenAgents product surface record local Gmail
   draft ids through an operator-only writeback?
4. Is billing email a system notification exempt from human approval, and if
   so what policy/version should be recorded on the delivery record?
5. Should future email templates be stored in D1, in source-controlled code, or
   both with code-owned schema versions and DB-owned active state?

## Bottom Line

The previous work is valuable, but it points to a more disciplined OpenAgents product surface shape
than any single old implementation:

- use Vortex's Resend idempotency and admin invitation state;
- use Fireball's Effect service/layer discipline;
- use Laravel's template/message/delivery/activity model and preview-before-send
  flow;
- preserve the Gmail wrapper's draft-first local safety model;
- keep OpenAgents product surface's current redacted config and schema-decoded Resend REST approach.

The immediate OpenAgents product surface rule should be simple: no new email side effect should be a
bare provider call from a route. It should pass through a typed service, carry
an idempotency key, record durable state, redact sensitive bodies/provider
payloads from exports and logs, and make draft versus send an explicit product
state.
