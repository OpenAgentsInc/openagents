# Cloudflare Email Automation Audit

Date: 2026-06-16
Scope: `apps/openagents.com` email, auth, customer notification, and inbound
mail possibilities.
Companion: `docs/auth/2026-06-16-login-and-auth-audit.md`.

## TL;DR

Cloudflare Email Service is now a plausible first-party email rail for
OpenAgents because the live product already runs on Cloudflare Workers and the
service supports outbound transactional email through a Workers binding, REST
API, and beta authenticated SMTP. It also supports inbound Email Routing to
verified destination addresses or Worker `email()` handlers.

The strong fit is:

- auth sign-in codes or magic links;
- lifecycle/customer transactional emails;
- billing, security, operator, and delivery notifications;
- internal smokes to verified destination addresses;
- inbound support/order/reply handling through Email Routing;
- email-as-agent-input flows, if routed through typed parsers and explicit
  authority tokens rather than prompt keyword matching.

The weak fit is marketing/cold/bulk email. Cloudflare's own FAQ says Email
Service is intended only for transactional email today and that marketing and
bulk sender tooling is future work. Our targeted-remake outreach, broad drip
campaigns, partner campaigns, and CRM-style prospecting should therefore stay
on Resend/Gmail/a dedicated marketing platform unless we strictly reclassify a
message as transactional, opt-in, expected, and compliant.

Recommended posture:

1. Add Cloudflare as a second product email provider behind the existing
   `EmailService` ledger, not as direct route-handler sends.
2. Prefer the Workers `send_email` binding inside `apps/openagents.com`; use
   REST or SMTP only for non-Worker surfaces.
3. Keep auth email separate from bulk/campaign sending, but move it from direct
   Resend to a small first-party auth sender once Cloudflare sending is
   onboarded and smoke-tested.
4. Use Cloudflare Email Routing for inbound support and order/reply capture,
   with D1/R2 persistence and human/agent review before any outbound reply.
5. Do not remove Resend yet. Cloudflare currently gives dashboard/activity logs,
   GraphQL analytics, and suppression APIs, but I did not find a Cloudflare
   Email lifecycle webhook equivalent to our Resend webhook route in the MCP
   docs. We need a polling/reconciliation plan before replacing webhook-backed
   delivery state.

## Unified Email & Auth Strategy — what we do when

This is the single source of truth for sequencing across **auth** and **all
product email**. The companion `docs/auth/2026-06-16-login-and-auth-audit.md` is
the auth/login subsystem reference; this section owns the cross-cutting roadmap.

### Does the Cloudflare option change our current plans?

**No reversal, no rush.** Email OTP login already shipped on **direct Resend** and
is stable; nothing here requires changing or reverting it. Cloudflare is a
**forward, additive** rail. The one thing it *settles* is the login audit's last
open question (auth-email deliverability domain): the **target** is a dedicated
first-party Cloudflare auth sender/subdomain, reached **only after** a
verified-destination smoke and a provider adapter exist — not before. Until then,
Resend stays the auth transport. Do **not** fast-track auth onto Cloudflare ahead
of the smoke + reconciliation work.

### Provider roles (Phase 0 decision — locked)

| Lane | Provider (target) | Provider (today) |
| --- | --- | --- |
| Auth sign-in codes / security notices | Cloudflare (dedicated auth subdomain) | **Resend, direct** (shipped) |
| Product lifecycle / billing transactional | Cloudflare, behind `EmailService` | Resend, behind `EmailService` |
| Internal / operator smokes | Cloudflare (free verified-destination sends) | Resend |
| Inbound support / order replies | Cloudflare Email Routing → Worker | none (no inbound today) |
| Operator-authored drafts | Gmail / local `gws` | Gmail / local `gws` |
| Cold / bulk / marketing / drip | **dedicated marketing platform — never Cloudflare yet** | Resend / Gmail drafts |

### Sequence (status: DONE / NOW / NEXT / LATER / NEVER-YET)

1. **DONE — Email OTP login.** `/login` page, OpenAuth `CodeProvider`, `success`
   for `provider:'code'`, Resend transport, gating preserved (issue #5111).
2. **DONE — OTP hardening.** The direct-Resend auth baseline now has a
   first-party D1-backed send/resend guard on `/code/authorize`: per-IP,
   per-normalized-email, and global hourly caps; hashed bucket subjects; no-store
   retry responses with `Retry-After`; fail-closed behavior when storage or the
   sender is unavailable; no raw code in the subject; and a 10-minute
   session-issuance expiry layered over OpenAuth 0.4.3's fixed 24-hour code
   state.
3. **NOW — hold steady on Resend.** Keep auth OTP on direct Resend and all
   webhook-backed lifecycle mail on Resend behind `EmailService`. No code change
   is forced by this audit. This is the stable baseline.
4. **NOW — Cloudflare smoke + adapter.** The code adapter slice is landed:
   `cloudflare_email` is now a supported `EmailService` provider in Effect
   schemas and D1 checks, and rendered messages can be sent through a
   Cloudflare `send_email` binding while preserving the existing
   `email_messages.idempotency_key` ledger boundary. Still remaining before any
   live traffic: onboard a sending subdomain, add a restricted staging binding,
   and send one internal `operator_notification` to a verified destination
   address only. (Phases 1–2 below.)
5. **LATER — move auth email to Cloudflare.** Once the smoke passes: a dedicated
   auth sender on `auth.openagents.com` (or `mail.openagents.com`) that preserves
   the #5120 throttle, expiry, no-enumeration, and fail-closed discipline; Resend
   kept as fallback. (Phase 3.)
6. **LATER — product lifecycle cutover + reconciliation.** Dual-send / cut over
   lifecycle mail to Cloudflare, and add the GraphQL reconciliation poller +
   suppression sync (there is **no** Cloudflare lifecycle webhook — see
   Observability). Keep Resend on webhook-backed flows until the poller is proven.
   (Phases 2 + 5.)
7. **LATER — inbound Email Routing.** `support@` / `orders@` / tokenized reply
   aliases → Worker `email()` handler → typed parse → D1/R2 → operator
   task/draft. Inbound is untrusted; never grants authority. (Phase 4.)
8. **NEVER-YET — cold/bulk/marketing on Cloudflare.** Stays on a dedicated
   marketing platform until Cloudflare ships bulk-sender tooling and the message
   is opt-in + unsubscribe + suppression compliant.

### Open questions — now decided (defaults; revisit at onboarding)

- **Sender subdomains:** auth `login@auth.openagents.com`; product lifecycle
  `notify@mail.openagents.com`; billing/security `billing@` / `security@openagents.com`;
  inbound `support@openagents.com`.
- **Auth first or product first on Cloudflare?** Internal smoke first, then
  **auth** (small, already decoupled), then product lifecycle.
- **No Cloudflare lifecycle webhook — acceptable?** Yes for auth (we don't ledger
  auth email) and internal smokes; for product lifecycle, keep Resend until the
  reconciliation poller is proven.
- **Auth path stays outside `EmailService`?** Yes — keep it decoupled/reliable
  with its own redaction, rate-limit, expiry, and no-enumeration discipline; a
  narrow auth-email metrics lane can come later if needed.
- **Split relationship vs marketing mail before any provider change?** Yes —
  classify recurring mail as transactional-relationship vs marketing first;
  marketing never moves to Cloudflare yet.
- **Inbound replies create what?** Operator tasks / support records first; Forum
  or workroom mapping later and only via tokenized routing, never keyword
  matching.

## Current OpenAgents State

The earlier auth audit says email login was not present yet. Current `main`
has moved since that document was written: `apps/openagents.com/INVARIANTS.md`
now declares `/login/email`, and `workers/api/src/index.ts` wires OpenAuth
`CodeProvider` with a Resend-backed sign-in-code sender.

Relevant current facts:

- `docs/auth/2026-06-16-login-and-auth-audit.md` is the same-day companion
  audit for login/auth posture.
- `workers/api/src/index.ts` has `/login/email` and OpenAuth `CodeProvider`.
- `sendSignInCodeEmail` sends directly to Resend and comments that auth email
  stays decoupled from CRM/marketing email-intent machinery.
- `workers/api/src/email.ts` owns production product email rendering, ledger
  reservation, provider sends, and delivery writes through `EmailService`.
- `workers/api/migrations/0026_email_ledger.sql` records
  `email_templates`, `email_messages`, `email_deliveries`, and `email_drafts`.
- The provider enum and D1 checks allow `resend`, `gmail`, and
  `cloudflare_email`.
- `workers/api/wrangler.jsonc` has Resend config vars, but no Cloudflare
  `send_email` binding yet.
- `apps/openagents.com/INVARIANTS.md` requires product email sends to pass
  through `EmailService`, carry typed kinds and idempotency keys, and persist
  `email_messages` before provider delivery.
- Resend webhook ingestion exists at `POST /api/webhooks/resend` and updates
  provider events, deliveries, and suppressions.

This means Cloudflare Email should stay integrated as a provider under our
existing ledger rather than bypassing it. The first provider-adapter slice now
adds `cloudflare_email` to Effect schemas and D1 checks and maps Cloudflare
send responses/errors into the existing ledger shape. Analytics reconciliation
and suppression sync remain later slices.

## Cloudflare Email Service Capabilities

Cloudflare Email Service has two major parts:

- Email Sending: outbound transactional email.
- Email Routing: inbound email forwarding or Worker processing.

Cloudflare documents three outbound interfaces:

- Workers binding: `env.EMAIL.send(...)` from a Worker after adding
  `send_email` to Wrangler config.
- REST API: `POST /accounts/{account_id}/email/sending/send` with a
  Cloudflare API token.
- Authenticated SMTP beta: `smtp.mx.cloudflare.net:465` using implicit TLS,
  username `api_token`, and an API token with `Email Sending: Edit`.

For OpenAgents, the Workers binding is the right default because the live app
is a Cloudflare Worker. It avoids storing an Email Sending API token in the
Worker and lets us keep sends inside the same typed service boundary. REST is
useful for external tools or non-Worker services. SMTP is useful only for
existing SMTP-native software; it is not the ideal path for new Worker code.

Inbound Email Routing can:

- forward `support@`, `contact@`, `orders@`, or similar addresses to verified
  destination addresses;
- route addresses to a Worker `email(message, env, ctx)` handler;
- let that Worker parse raw MIME with `postal-mime`;
- forward, reject, or reply to incoming mail;
- use routing rules, catch-all behavior, and subaddressing for mailbox
  patterns.

## Requirements, Limits, And Pricing

Current Cloudflare docs surfaced through the MCP say:

- The domain must use Cloudflare DNS.
- Email Sending for arbitrary recipients requires Workers Paid.
- Workers Paid includes 3,000 outbound emails per account per month, then
  $0.35 per 1,000 emails.
- Email Routing is available on Workers Free and Workers Paid.
- Sends to verified destination addresses are free on all plans and do not
  count toward monthly quota or daily sending limits.
- New accounts begin with conservative daily limits that scale with behavior,
  deliverability, and account standing.
- Before a sending domain is onboarded, sends are limited to verified
  destination addresses.
- After a sending domain is onboarded, sends can go to arbitrary recipients
  subject to plan/quota/rate limits.
- General outbound message size is 5 MiB including attachments.
- Sends to verified destination addresses may use up to 25 MiB.
- Combined `to`/`cc`/`bcc` recipients are limited to 50 per email.
- Subject lines are limited to 998 characters.
- Custom headers are limited to 16 KB total.
- Email Routing has 200 routing rules per domain and 200 destination addresses
  per account.
- Inbound message size is 25 MiB.
- `message.reply()` rejects inbound messages with more than 100 `References`
  entries to reduce loops.
- SMTP submission is port 465 with implicit TLS only. Plaintext SMTP,
  STARTTLS-on-587, and unauthenticated outbound relay are not supported.

The important correction versus some older notes: the current Cloudflare REST
API docs and limits page say the normal outbound size limit is 5 MiB, not
25 MiB, except for verified destination-address sends.

## Domain And Deliverability Posture

Onboarding Email Sending adds Cloudflare-managed records under the `cf-bounce`
subdomain, including MX records for bounces plus SPF, DKIM, and DMARC records.
Email Routing onboarding adds inbound MX plus SPF/DKIM records for routing.
Sending and Routing are managed separately; removing one does not remove the
other.

Cloudflare says it automatically handles:

- SPF/DKIM alignment for outbound mail;
- DKIM and ARC signing;
- IP reputation management;
- soft-bounce retry with exponential backoff;
- hard-bounce suppression;
- spam-complaint feedback processing and suppression.

We still own:

- sender/domain choice;
- list hygiene;
- unsubscribe behavior for recurring mail;
- double opt-in where a subscription-like flow exists;
- suppression sync into our own product state;
- honest classification of transactional vs marketing;
- avoiding shared reputation failure between auth, product, and outreach.

Recommended domain split:

- Auth: use a dedicated sender such as `login@auth.openagents.com` or
  `login@mail.openagents.com`, onboarded separately.
- Product lifecycle: use `notify@openagents.com` or
  `notify@mail.openagents.com`.
- Billing/security: use `billing@openagents.com` or
  `security@openagents.com`, possibly restricted to a separate binding.
- Support replies: use `support@openagents.com` with Email Routing into a
  Worker or verified inbox.
- Campaign/cold outreach: do not move to Cloudflare Email Service yet unless
  Cloudflare's marketing/bulk support becomes available and the campaign has
  explicit opt-in, unsubscribe, suppression, and compliance gates.

Cloudflare treats subdomains as separate sending domains. That is useful for
reputation isolation, but each subdomain must be onboarded explicitly.

## What We Can Automate Transactionally

### Auth

Cloudflare is a good fit for:

- one-time sign-in codes;
- magic-link delivery;
- email verification;
- passwordless account recovery links if we add them;
- security notices for new login, new device, provider connected, or provider
  disconnected.

Implementation note: auth email should keep a separate throttle and provider
boundary from broad product/campaign mail. The current code already says this
for Resend. With Cloudflare, use either:

- a restricted `AUTH_EMAIL` send binding with allowed sender addresses; or
- a tiny auth sender wrapper that uses the same binding but is policy-separated
  from `EmailService`.

The security-sensitive part is not the transport. It is rate limiting, code
expiry, no user enumeration, no raw code logging, and preserving the invariant
that login only authenticates while product access remains downstream gated.

### Product Lifecycle

Cloudflare can send the existing lifecycle emails already modeled in
`EmailService`, including:

- order received;
- scoping started;
- repository/source needed;
- Autopilot queued/running;
- review ready;
- saved version ready;
- deployed;
- customer input needed;
- unavailable/declined;
- delivered;
- adjustment received/completed.

The current `EmailService` design is already appropriate: render deterministic
HTML/text, reserve `email_messages`, record `email_deliveries`, use an
idempotency key, and keep state transitions authoritative before provider
delivery. A Cloudflare adapter should preserve that. Provider failure must not
roll back order, Site, assignment, billing, or deployment state.

### Billing And Account Notifications

Good Cloudflare candidates:

- out-of-credits notices;
- payment/checkout status notices;
- receipt availability;
- payout/settlement readiness notices where the copy is receipt-backed;
- admin/operator alerts;
- provider-account reconnect notices;
- security and credential-change notices.

These are transactional, expected, and tied to account state. They should use
HTML and text bodies, bounded provider error summaries, and source-authority
refs in the existing email ledger.

### Internal And Operator Notifications

Verified destination sends are free and do not count against daily/monthly
limits. That makes Cloudflare attractive for:

- production smoke emails to internal operator addresses;
- low-volume deployment or incident notices;
- runbook verification;
- local/staging tests to verified accounts.

This is the safest first adoption path because it does not require arbitrary
recipient sending and does not risk user-facing delivery posture.

### Recurring Product Digests

Cloudflare can technically send recurring digests through the same outbound
interfaces. Policy is the constraint. These should be limited to opted-in
product/account relationship mail, such as:

- "your workspace weekly summary";
- "your open work needs review";
- "your order has pending input";
- "your team had new accepted outcomes";
- "your agent account has new security-relevant activity."

Each recurring email needs preference state, unsubscribe headers, suppression
checks, and a separate reputation lane. Anything that is principally lead
nurture, investor update, promotion, or cold sales belongs elsewhere for now.

## What We Can Automate Inbound

### Support Inbox

Email Routing can receive `support@openagents.com`, parse with `postal-mime`,
store the message in D1, store attachments in R2, and create an operator-safe
support thread. The Worker can forward to a verified address immediately while
also storing a structured copy.

Recommended flow:

1. Email Routing rule sends `support@openagents.com` to a Worker.
2. Worker buffers `message.raw` once and parses MIME.
3. Worker stores envelope sender, recipient, subject, text/html summary,
   `Message-ID`, `In-Reply-To`, `References`, attachment metadata, and a
   redaction classification.
4. Worker forwards to a verified support inbox or creates an operator task.
5. Replies are drafted for human review; no agent auto-send from inbound mail
   without an explicit approval record.

### Order And Workroom Replies

Inbound email can map customer replies to orders or workrooms if we avoid
freeform keyword routing. Use bounded identifiers instead:

- plus-addressing such as `orders+<opaque-order-ref>@openagents.com`;
- per-message reply tokens;
- `In-Reply-To` / `References` stored from the original outbound email;
- signed HMAC tokens in reply addresses or headers.

Do not infer order/workroom selection from subject words or body text. The
workspace guidance bans ad hoc keyword routing. Freeform body text can be the
customer message after a typed route has already been selected by token or
thread headers.

### Agent Interaction By Email

Cloudflare docs explicitly mention email as an agent interaction mode. For us,
that can mean:

- email creates a support ticket;
- email appends input to an existing order;
- email supplies a missing file or answer;
- email asks a registered agent to inspect a bounded thread;
- email replies become drafts for an operator or agent to review.

Hard rule: inbound email is untrusted input. It must not grant spend,
deployment, accepted-work, provider mutation, payout, or public-claim
authority. It can create evidence, tasks, drafts, or review requests.

## What Cloudflare Should Not Own Yet

Do not move these to Cloudflare Email Service as of this audit:

- cold outreach;
- broad marketing campaigns;
- investor/newsletter campaigns;
- high-volume prospecting;
- generic drip marketing;
- any list without explicit opt-in and unsubscribe state;
- any campaign that depends on bulk-sender tooling or sophisticated audience
  management.

Cloudflare's FAQ says marketing support is future work. We should not stretch
the product's transactional framing to save a provider. Keep these on a
dedicated marketing platform, or on the existing local Gmail draft path when
human-authored/operator-owned draft semantics are required.

## Observability And Delivery State

Cloudflare gives:

- immediate send responses from the REST API with recipient delivery buckets;
- Workers binding errors for validation, sender verification, suppression,
  rate limit, daily limit, and delivery failures;
- dashboard analytics and activity logs;
- GraphQL Analytics datasets:
  - `emailSendingAdaptiveGroups`;
  - `emailSendingAdaptive`;
  - `emailRoutingAdaptiveGroups`;
  - `emailRoutingAdaptive`;
- 31-day analytics retention;
- suppression lists automatically populated by hard bounces, repeated soft
  bounces, and spam complaints.

The gap versus our current Resend setup:

- We already ingest Resend lifecycle webhooks into our ledger.
- I did not find an equivalent Cloudflare Email lifecycle webhook in the MCP
  docs.
- Cloudflare analytics are zone-level, require `Analytics Read`, and are a
  query/reconciliation surface rather than an event push.

Therefore the Cloudflare adapter should initially:

- record the provider response synchronously in `email_deliveries`;
- store Cloudflare message IDs/session IDs when returned;
- periodically reconcile final statuses from GraphQL analytics for the last
  31 days;
- periodically import suppression state from Cloudflare suppression APIs;
- expose operator state as "accepted/queued/polled delivered/polled failed"
  instead of pretending webhook parity exists.

If Cloudflare later adds lifecycle webhooks, we can replace or supplement the
poller.

## Implementation Plan

### Phase 0: Product Decision

Decide provider roles before code:

- Cloudflare: auth, product transactional, inbound support, internal smokes.
- Resend: retained fallback and any current lifecycle webhook-backed flows
  until Cloudflare reconciliation is live.
- Gmail/local `gws`: operator-owned drafts only.
- Dedicated marketing platform: cold/bulk/marketing sends.

### Phase 1: Cloudflare Smoke Without User Blast Radius

1. Onboard a low-risk sending domain or subdomain in Cloudflare Email Sending.
2. Add a verified destination address controlled by us.
3. Add a restricted Wrangler `send_email` binding for an internal smoke sender.
4. Run `npx wrangler types` so the Worker sees the real `SendEmail` types.
5. Add a local/staging dry-run route or script that sends only to verified
   destination addresses.
6. Record the smoke result in the existing email ledger shape.

This proves DNS, binding, sender, and basic delivery without arbitrary
recipient sends.

### Phase 2: Add Cloudflare Provider Behind `EmailService`

1. **Done:** Extend Effect schemas and D1 provider checks from `resend|gmail`
   to include `cloudflare_email`.
2. **Done:** Add a Cloudflare sender adapter that accepts `RenderedEmail`.
3. **Done:** Map:
   - the current `RenderedEmail` fields (`to`, `from`, `replyTo`, `subject`,
     `html`, `text`, and headers) to Workers binding fields;
   - Cloudflare binding success to `accepted` delivery state;
   - Cloudflare validation/suppression/rate-limit errors to bounded error
     names and length-limited summaries.
4. **Done:** Preserve idempotency by checking `email_messages.idempotency_key` before
   provider send. Do not rely on Cloudflare to dedupe product intent.
5. **Done:** Keep provider payload summaries bounded and redacted.

### Phase 3: Move Auth Email Carefully

The current auth code sends sign-in codes directly through Resend. Once
Cloudflare smoke passes:

1. Create an auth-specific Cloudflare sender wrapper or restricted binding.
2. Keep auth send metrics and throttling separate from campaign/product bulk.
3. Use a dedicated auth sender address/subdomain.
4. Preserve OpenAuth `CodeProvider` behavior and downstream authorization
   gates.
5. Add tests that missing Cloudflare email config fails closed with no session
   issuance.

Do not route sign-in-code mail through broad marketing/campaign dispatchers.

### Phase 4: Inbound Email Routing

1. Onboard Email Routing for `openagents.com` or a support subdomain.
2. Add routing rules for `support@`, `orders@`, and tokenized reply aliases.
3. Add a Worker `email()` handler in the existing Worker entrypoint.
4. Parse inbound mail with `postal-mime`.
5. Store structured thread/message rows in D1 and attachments in R2.
6. Forward to verified operator inboxes where needed.
7. Draft replies through `EmailService`; require human/operator approval before
   send for any customer-impacting reply.

### Phase 5: Reconciliation And Suppression

1. Add a scheduled reconciliation job for Cloudflare GraphQL analytics.
2. Add suppression-list import/sync.
3. Add operator inspection fields for Cloudflare message IDs, status source,
   and last reconciled time.
4. Define a retention policy because Cloudflare analytics are only 31 days.
5. Keep our own `email_messages`/`email_deliveries` as the durable product
   source of truth.

## Open Questions — resolved

These are now decided in **"Unified Email & Auth Strategy — what we do when" →
Open questions — now decided** above (sender subdomains, auth-first vs
product-first, the no-webhook/reconciliation tradeoff, keeping auth outside
`EmailService`, splitting relationship vs marketing mail, and inbound-reply
landing). The defaults there are revisitable at onboarding, but the strategy is
no longer ambiguous. The remaining genuinely-deferred item is **timing**: when we
spend the slice on the Cloudflare verified-destination smoke (strategy step 3),
which is the gate for everything after it.

## Recommended Next Slice

Do not start with broad migration. Start with a verified-destination smoke and
a provider adapter behind `EmailService`:

1. **Done:** Add `cloudflare_email` as a supported provider in schemas and
   migrations.
2. Add the Wrangler `send_email` binding in staging only after the sending
   domain is onboarded.
3. **Done:** Implement `sendRenderedEmailViaCloudflareBinding`.
4. Add a smoke that sends one internal `operator_notification` to a verified
   destination address.
5. Add operator inspection showing provider `cloudflare_email`, message id,
   send status, and "not reconciled yet" delivery finality.
6. Only after that, choose whether auth sign-in codes or product lifecycle mail
   moves first.

## Sources

Cloudflare MCP documentation consulted:

- Cloudflare Email Service overview:
  `https://developers.cloudflare.com/email-service/`
- Send emails:
  `https://developers.cloudflare.com/email-service/get-started/send-emails/`
- Workers API for outbound sending:
  `https://developers.cloudflare.com/email-service/api/send-emails/workers-api/`
- REST API:
  `https://developers.cloudflare.com/email-service/api/send-emails/rest-api/`
- SMTP beta:
  `https://developers.cloudflare.com/email-service/api/send-emails/smtp/`
- Route emails:
  `https://developers.cloudflare.com/email-service/get-started/route-emails/`
- Email handler:
  `https://developers.cloudflare.com/email-service/api/route-emails/email-handler/`
- Pricing:
  `https://developers.cloudflare.com/email-service/platform/pricing/`
- Limits:
  `https://developers.cloudflare.com/email-service/platform/limits/`
- Email authentication:
  `https://developers.cloudflare.com/email-service/concepts/email-authentication/`
- Deliverability:
  `https://developers.cloudflare.com/email-service/concepts/deliverability/`
- Suppressions:
  `https://developers.cloudflare.com/email-service/concepts/suppressions/`
- Metrics and analytics:
  `https://developers.cloudflare.com/email-service/observability/metrics-analytics/`
- Logs:
  `https://developers.cloudflare.com/email-service/observability/logs/`
- Headers:
  `https://developers.cloudflare.com/email-service/reference/headers/`
- FAQ:
  `https://developers.cloudflare.com/email-service/reference/faq/`
- SMTP changelog:
  `https://developers.cloudflare.com/changelog/post/2026-06-08-smtp-submission/`

OpenAgents repo sources inspected:

- `docs/auth/2026-06-16-login-and-auth-audit.md`
- `apps/openagents.com/INVARIANTS.md`
- `apps/openagents.com/workers/api/src/index.ts`
- `apps/openagents.com/workers/api/src/email.ts`
- `apps/openagents.com/workers/api/src/config.ts`
- `apps/openagents.com/workers/api/wrangler.jsonc`
- `apps/openagents.com/workers/api/migrations/0026_email_ledger.sql`
- `apps/openagents.com/docs/2026-06-04-previous-resend-gmail-email-systems-audit.md`
- `apps/openagents.com/docs/2026-06-05-resend-email-ledger-smoke-runbook.md`
