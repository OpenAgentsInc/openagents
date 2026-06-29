# Targeted Remake Outreach Email

`targeted-site-remake-outreach-email.ts` dispatches approved targeted-remake
concept preview emails through the existing `EmailService` boundary.

No domain code calls Resend directly. The product dispatch layer validates the
operator approval and suppression state, then calls
`EmailService.sendTargetedRemakeOutreachEmailWithLedger`.

## Email Template

`TargetedRemakeOutreachEmailInput` renders:

- concept preview link;
- meeting/review link;
- sender name and contact;
- postal/contact address;
- unsubscribe link;
- manage-preferences link;
- a source-safe value proposition;
- explicit concept disclosure that the preview is not operated or endorsed by
  the target organization.

The HTML includes light-mode metadata and `!important` colors to avoid mobile
dark-mode clients inverting body text into unreadable colors.

## Dispatch Ledger

The D1 table is `targeted_site_remake_outreach_email_dispatches`.

Each row links:

- campaign and optional prospect;
- normalized domain;
- preview generation ref;
- operator review event ref;
- EmailService message ref;
- recipient ref, not raw recipient email;
- template slug;
- suppression state;
- accepted/failed/blocked/skipped dispatch state;
- redacted error summary and metadata.

## Gates

Dispatch requires:

- operator decision `approve_outreach`;
- operator next state `outreach_approved`;
- operator suppression state `clear`;
- generated preview URL;
- public-safe value proposition and concept disclosure;
- unsubscribe and preferences URLs in the email input.

The dispatch layer rejects private provider payloads, raw browser logs, secret
material, payment or wallet material, and bypass instructions.

## Status

Implemented in GitHub issue `#191` as
`OPENAGENTS-SITES-OUTREACH-011: Add typed targeted-remake outreach email`.
