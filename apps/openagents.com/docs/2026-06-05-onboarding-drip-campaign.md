# Onboarding Drip Campaign

Status: implemented campaign definition and enrollment scheduler for
`OPENAGENTS-O-007`.

The initial signup drip is a three-step campaign for new users who do not
already have active or delivered work:

- Day 0: welcome the user, explain that OpenAgents accepts concrete software
  requests, and point them back to the product/order surface.
- Day 1: explain what makes a useful Sites or software request, including
  audience, expected result, repository/source context, and beta expectations.
- Day 2: prompt the next action, explain that ready revisions appear on the
  order page, and invite one concrete follow-up comment.

## Runtime Shape

`workers/api/src/email-onboarding-drip.ts` owns the typed campaign definition.
It seeds:

- `new-signup-onboarding` in `email_campaigns`;
- `day_0`, `day_1`, and `day_2` steps in `email_campaign_steps`;
- idempotent `email_campaign_enrollments`;
- scheduled `email_campaign_sends` due at 0, 86,400, and 172,800 seconds.

The campaign uses the source-authority ref
`system.email_onboarding_drip.v1`. Scheduled sends carry per-step authority
refs so dispatcher and audit code can distinguish generated sends from manual
operator emails.

## Eligibility

Enrollment is skipped before any scheduling work when:

- the user already has active requested work;
- the user already has delivered requested work;
- the email address has an active drip/all suppression;
- the email preference record disables drip mail.

The enrollment function is intentionally separate from the dispatcher. This
issue creates durable send records; the scheduled dispatcher will claim due
records, render through `@openagentsinc/email-templates`, send through
`EmailService`, and attach the resulting `email_message_id`.

## Dispatcher

`workers/api/src/email-campaign-dispatcher.ts` is wired into the Worker
scheduled handler. Every minute it:

- selects due `email_campaign_sends` rows in `scheduled` state;
- atomically claims each row by changing `scheduled` to `claimed`;
- re-checks drip suppressions, drip preferences, active orders, and delivered
  orders before sending;
- renders `drip.signup_day_0.v1`, `drip.signup_day_1.v1`, or
  `drip.signup_day_2.v1`;
- sends through `EmailService`, which writes `email_messages` and
  `email_deliveries`;
- marks the campaign send `sent`, `suppressed`, `skipped`, `scheduled` for a
  bounded retry, or `failed`.

Dispatcher retry state lives on `email_campaign_sends.attempt_count` and
`next_attempt_at`. Provider failures are retried up to three attempts with
bounded redacted error fields. Duplicate dispatcher workers are prevented by
the claim update; if another worker already claimed the row, the duplicate path
does not render or send.

## Guardrails

- Campaign and send records never contain provider account refs, auth grants,
  raw provider payloads, or secrets.
- Idempotency is based on the durable campaign, enrollment, and step IDs, not
  on a newly generated retry ID.
- The email template package remains the source of rendered copy. Campaign
  records store template slugs and bounded metadata only.
