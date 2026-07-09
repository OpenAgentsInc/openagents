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
- **Live ecosystem truth is state-capped and flag-gated (KHS-9, #8608):**
  the ecosystem tools (`src/services/ecosystem-tools.ts`) read ONLY public
  openagents.com surfaces (promise registry, tokens-served, pylon stats,
  Khala Code plan catalog), fail-soft with a 60s cache, and serve registry
  `safeCopy` only — a record's state caps every claim (yellow always ships
  operator-assisted caveat wording; non-green ships do-not-pitch wording).
  Promise lookup and the grounding intent match are embedding/cosine via the
  shared `sarahEmbedText` lane — never keyword routing; no embedder means an
  honest miss. The grounding hook on both brain lanes is a no-op unless
  `SARAH_ECOSYSTEM_GROUNDING=1` and always runs AFTER the deterministic
  pricing guard.
- **Customer Blueprint drafts are per-prospect, provenance-carrying, and
  operator-handed (KHS-9, #8608):** `src/services/customer-blueprint.ts`
  composes a draft from ONE prospect's profile facts, contact, and stated
  needs (every need cites its source turn id), bound to
  `prospectRefAliases(prospectRef)` at the query layer (oracled in
  `src/services/customer-blueprint.test.ts`). Suggested modules carry the
  deal-rules `pricingStatus` verbatim (`owner_pricing_required` passes
  through) and never a price. The draft is a DRAFT for the operator-assisted
  business-workspace pipeline (CB-1.4 convergence) — no automated workspace
  provisioning is claimed or performed. The operator listing
  (`/sarah/api/operator/customer-blueprints`) is admin-bearer-guarded,
  fail-closed.
