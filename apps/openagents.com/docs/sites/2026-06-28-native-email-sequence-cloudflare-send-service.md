# Native Email Sequence Send Service

This records the issue #6841 implementation boundary for
`autopilot_sites.native_email_sequences.v1`.

## Runtime Gate

- Worker binding: `EMAIL` via Cloudflare Email Sending (`send_email` in
  `workers/api/wrangler.jsonc`).
- Feature flag: `EMAIL_SEQUENCE_SEND_ENABLED`.
- Sender address: `EMAIL_SEQUENCE_FROM_EMAIL`; falls back to
  `RESEND_FROM_EMAIL` only when the dedicated sequence sender is unset.
- Reply-to: `EMAIL_SEQUENCE_REPLY_TO_EMAIL`; falls back to
  `RESEND_REPLY_TO_EMAIL`.

When `EMAIL_SEQUENCE_SEND_ENABLED` is not truthy, authored sequence sends keep
the dry-run/skipped path and do not call Cloudflare Email.

## Domain Authentication

Before arming production sends, the sending domain must be onboarded in
Cloudflare Email Sending. Cloudflare-managed onboarding configures SPF and DKIM
for the domain. The domain must also publish a DMARC record, for example:

```txt
v=DMARC1; p=quarantine; rua=mailto:dmarc-reports@openagents.com
```

Operator check:

```sh
npx wrangler email sending list
```

The sending domain must appear in that list before `EMAIL_SEQUENCE_SEND_ENABLED`
is set to `true`.

## Receipt Check

A live proof for a green claim requires a real sequence send whose
`email_campaign_sends` row reaches `sent`, an `email_messages` row with
`provider='cloudflare_email'`, and an `email_deliveries` row with
`status='accepted'` or later provider delivery evidence. Bounce/complaint
handling remains a separate receipt gate; code-level accepted receipts alone do
not flip the promise green.
