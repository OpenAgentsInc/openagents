# apps/sarah invariants

- **Authority:** openagents.com Worker is system of record for CRM, credits,
  checkout, receipts, promise registry. Sarah only uses public HTTP contracts.
- **Mount:** public surface is `openagents.com/sarah` (not a separate subdomain).
- **Token route:** realtime token mint keeps origin/rate/session/daily caps (S-3).
- **AI disclosure:** always-on in the UI shell; never optional.
- **Pricing:** no improvised discounts; deal-rules code + public packs only;
  owner-priced params from runtime config.
- **Prospect memory:** reads are scoped by exact `prospect_ref` identity
  aliases only; no cross-prospect listing, pattern scans, or prompt-side
  filtering. Sarah must deterministically refuse requests to reveal another
  prospect/customer's private conversation, memory, profile, contact details,
  objections, or needs. Any fact promoted outside one prospect's private scope
  must pass redaction and the future owner-approved collective-learning store.
- **Email:** only via monorepo approval-gated CRM rail (`crm-email-rail`); no
  Sarah-local Resend/suppression/approval queue.
- **UI:** zero React in `apps/sarah`. DOM shell now; Effect Native component
  set is the growth path (gaps via EN-2).
- **Agent runtime:** HTTP turns use owned seed runtime; eve is not required for
  monorepo serving path.
- **Cross-prospect isolation (KHS-3, #8602):** no data from one prospect_ref
  may surface in another prospect's conversation. Isolation is enforced at the
  query layer — every prospect-scoped read (session-index and the KHS-2
  prospect-memory service, #8601) filters by exact `prospect_ref` (or its
  deterministic same-identity aliases) — never by prompt-side instruction.
  Contracts + oracles:
  `src/contracts/isolation-contracts.ts` /
  `src/contracts/isolation-contracts.test.ts`; human doc
  `docs/sarah/SARAH_CONTRACTS.md`.
- **Collective learning is owner-receipt-gated (KHS-4, #8603):** Sarah's
  shared knowledge reads only from the owner-approved store
  (`src/services/collective-learning.ts`): candidates distilled from
  transcripts are PII-redacted (redact-or-drop) and sit pending until the
  owner approves or rejects them on the admin-bearer-guarded
  `/sarah/api/operator/learning` endpoints (unarmed → 503, wrong bearer →
  401); every decision writes a receipt, answer-bank publications carry the
  receipt ref as `approved_by`, and nothing crosses prospects without one.
  Prospect PII never enters collective stores, and no public "learning from
  conversations" claim is made — internal owner-approved store only
  (`sarah.collective_learning_owner_gated.v1`, state enforced).
