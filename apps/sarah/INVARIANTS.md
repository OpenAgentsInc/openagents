# apps/sarah invariants

- **Authority:** openagents.com Worker is system of record for CRM, credits,
  checkout, receipts, promise registry. Sarah only uses public HTTP contracts.
- **Mount:** public surface is `openagents.com/sarah` (not a separate subdomain).
- **Token route:** realtime token mint keeps origin/rate/session/daily caps (S-3).
- **AI disclosure:** always-on in the UI shell; never optional.
- **Pricing:** no improvised discounts; deal-rules code + public packs only;
  owner-priced params from runtime config.
- **Email:** only via monorepo approval-gated CRM rail (`crm-email-rail`); no
  Sarah-local Resend/suppression/approval queue.
- **UI:** zero React in `apps/sarah`. DOM shell now; Effect Native component
  set is the growth path (gaps via EN-2).
- **Agent runtime:** HTTP turns use owned seed runtime; eve is not required for
  monorepo serving path.
