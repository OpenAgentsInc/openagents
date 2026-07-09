# Sarah Behavior Contracts

Machine source of truth: `apps/sarah/src/contracts/isolation-contracts.ts`
(schema: `@openagentsinc/behavior-contracts`). This document is the human
rendering; the coverage test in
`apps/sarah/src/contracts/isolation-contracts.test.ts` fails the sweep if this
doc drifts from the registry, if an enforced contract loses its oracle, or if
an oracle file drops its contract reference.

Lane: KHS-3 (#8602) + KHS-4 (#8603) + KHS-5 (#8604), epic #8599 (Sarah ×
Khala). The isolation law landed BEFORE any shared learning ("isolation
before generalization"): KHS-3 enforced the cross-prospect oracles first,
KHS-4 then shipped the owner-approved collective-learning queue behind them —
flipping `sarah.collective_learning_owner_gated.v1` from pending to enforced
in the same change that added the first shared-knowledge read path — and
KHS-5 moved Sarah's knowledge itself onto a typed, versioned Blueprint with
per-fact provenance (`sarah.blueprint_versioned_provenance.v1`).

## Where the statements come from

- `sarah.cross_prospect_isolation.v1` and
  `sarah.collective_learning_owner_gated.v1` record the owner's words
  verbatim — owner, spoken to Sarah in production, 2026-07-09 (relayed in
  epic #8599 and `docs/fable/2026-07-09-sarah-khala-connection-assessment.md`).
- `sarah.memory_query_scoped.v1` is the engineering restatement of the same
  directive: the safeguard is enforced at the query layer (exact
  `prospect_ref` filter — or its deterministic same-identity aliases — on
  every prospect-scoped read), never by prompt-side instruction.
- `sarah.no_improvised_pricing.v1` predates this lane (owner, Sarah spec,
  2026-07-07; `apps/sarah/INVARIANTS.md`) and is registered here so the
  already-existing deterministic pricing guard is bound to the same registry
  discipline.
- `sarah.blueprint_versioned_provenance.v1` records the owner's directive
  verbatim ("I want her to have that Blueprint of her own" — owner, spoken to
  Sarah in production, 2026-07-09, relayed in epic #8599): her knowledge is a
  typed object with per-fact provenance and receipted revisions, not a flat
  pasted document.

## How collective learning is gated (KHS-4, #8603)

`apps/sarah/src/services/collective-learning.ts` owns the pipeline:
deterministic distillation over recent transcripts nominates recurring
questions, objections, and winning answers; every example is PII-redacted
(redact-or-drop) before it can enter a `sarah_learning_candidates` row; the
owner decides on the admin-bearer-guarded `/sarah/api/operator/learning`
endpoints (`SARAH_OPERATOR_ADMIN_TOKEN`, falling back to
`OPENAGENTS_ADMIN_API_TOKEN`; unarmed → 503, wrong bearer → 401); each
decision writes a `sarah_learning_receipts` row; and question/answer
approvals publish `sarah_answer_bank` entries whose `approved_by` is the
receipt ref (`learning_receipt:<id>`), so a live answer dereferences back to
its approval receipt and redacted source turns. The shared read paths
(`listApprovedLearnings` and the KHS-6 answer bank) serve only approved
entries. This is an internal owner-approved store; it makes no public
"learning from conversations" claim while the
`data.khala_free_tier_trace_capture.v1` promise family stays yellow.

## Oracle locations

- `apps/sarah/src/contracts/isolation-contracts.test.ts` — registry
  validation + coverage, query-layer scoping, prospect-memory seam pins,
  avatar-brain injection probe.
- `apps/sarah/src/services/collective-learning.test.ts` — the KHS-4 (#8603)
  collective-learning oracles: approved-store-only shared reads, admin guard
  fail-closed on the operator routes, PII redact-or-drop.
- `apps/sarah/src/services/prospect-memory.test.ts` — the KHS-2 (#8601)
  memory seam suite: single-ref entry point, deterministic same-identity
  aliases (visitor refs never alias), fail-soft null without a store.
- `apps/sarah/src/server.test.ts` — the pricing-guard oracles (text lane +
  avatar brain) and the KHS-7 (#8606) account-link route oracles.
- `apps/sarah/src/services/account-link.test.ts` — the KHS-7 (#8606)
  account-link seam units: contact-row shape, single never-pushy prompt line,
  test-mode session parsing, anonymous fast path.
- `apps/sarah/src/services/sarah-blueprint.test.ts` — the KHS-5 (#8604)
  blueprint oracles: seed→compile roundtrip stability (the KB doc IS the
  compiled output), revision immutability (retire ≠ delete), provenance
  required on every fact (including the `learning_receipt:<id>` promotion
  seam from KHS-4), flag-off rollout safety, admin guard fail-closed + a full
  HTTP revision cycle.
- All run in `bun test` inside `apps/sarah`, in the `apps/sarah` `oracle`
  chain, and in the repo `test:sarah` sweep before pushes to main.

## Pending entries (blocker-gated, never claim as guaranteed)

None. `sarah.collective_learning_owner_gated.v1` flipped pending → enforced
with KHS-4 (#8603); all six registered contracts (including the KHS-7
`sarah.in_chat_account_linking.v1` account-link seam, #8606, and the KHS-5
`sarah.blueprint_versioned_provenance.v1` knowledge object, #8604) are
enforced in the test sweep.

## Registry

Registry version: `2026-07-09.4` (schema `openagents.behavior_contracts.v1`)

### `sarah.cross_prospect_isolation.v1` — ENFORCED

- **Surface:** sarah (prospect memory + conversation serving)
- **Stated by:** owner via sarah-production-conversation on 2026-07-09
- **Statement:** safeguards to make sure she's not taking stuff from one person and telling it to another
- **Enforcement tier:** test-sweep
- **Oracle** `cross_prospect_query_scoping.unit` (bun-test, unit): Query-layer scoping: seeds transcript turns, CRM projections, and tool receipts for two different prospect refs (sharing a session id to prove prospect_ref is the filter, not session id), then reads each prospect back through every prospect-scoped read helper in session-index and asserts zero rows, markers, or CRM fields from the other prospect appear. — `apps/sarah/src/contracts/isolation-contracts.test.ts`
- **Oracle** `cross_prospect_injection_probe.rpc` (bun-test, rpc): Injection probe on the avatar brain endpoint: seeds a distinctive secret into prospect B's persisted turns, then POSTs /sarah/api/llm/chat/completions (bearer-armed, model deliberately unarmed so only the deterministic layers answer) with a conversation_ref for prospect A and a user message asking what the last customer said. Asserts the reply and prospect A's recorded transcript contain nothing from prospect B. — `apps/sarah/src/contracts/isolation-contracts.test.ts`
- **Oracle** `prospect_memory_isolation.unit` (bun-test, unit): KHS-2 memory seam (#8601, merged): getProspectMemoryContext takes exactly one prospectRef; prospectRefAliases only re-encodes that same identity (visitor: refs never alias); an empty ref yields no aliases so no unscoped query is possible; and without a durable store memory fails soft to null instead of falling back to any cross-prospect source. Asserted both in the KHS-2 seam suite and in the isolation contract suite. — `apps/sarah/src/services/prospect-memory.test.ts`
- **Verification:** bun test src/contracts/isolation-contracts.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main.
- **Authority boundary:** This contract binds Sarah's own read/serve paths (session index, turn store, avatar brain, and the KHS-2 prospect-memory seam from #8601). It does not claim collective learning exists, does not arm any capture sink, and grants no authority over the openagents.com CRM boundary, which remains the system of record. Any NEW prospect-scoped read path added to apps/sarah must gain a bun-test oracle under this contract in the same change.

### `sarah.collective_learning_owner_gated.v1` — ENFORCED

- **Surface:** sarah (collective learning)
- **Stated by:** owner via sarah-production-conversation on 2026-07-09
- **Statement:** admin approval for determining what she's able to learn generally from everyone else
- **Enforcement tier:** test-sweep
- **Oracle** `approved_store_only.unit` (bun-test, unit): Shared-knowledge reads come ONLY from the owner-approved store: seeds pending candidates via distillation, approves one with a receipt, and asserts listApprovedLearnings returns exclusively the approved entry (receipt attached) while pending/rejected candidates stay unreachable; the published answer-bank entry's approved_by is the approval receipt ref, so a live answer dereferences back to its receipt and redacted source turns. — `apps/sarah/src/services/collective-learning.test.ts`
- **Oracle** `learning_admin_guard.rpc` (bun-test, rpc): Admin guard is mandatory and fails closed on the HTTP surface: with no admin token configured, GET /sarah/api/operator/learning and the distill/approve/reject POSTs return 503 (an approve without the guard is impossible); with the token armed, a missing or wrong bearer is 401 and no decision or receipt is ever written; only the exact bearer can list, distill, approve, and reject. — `apps/sarah/src/services/collective-learning.test.ts`
- **Oracle** `learning_pii_redaction.unit` (bun-test, unit): PII never enters candidates: seeded examples with emails, phone numbers, long digit runs, and URLs come out scrubbed; name introductions and ambiguous residue drop the example entirely (when in doubt, drop); distilled candidates are asserted free of the seeded contact data across summary, canonical question, proposed answer, and examples. — `apps/sarah/src/services/collective-learning.test.ts`
- **Verification:** bun test src/services/collective-learning.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main.
- **Authority boundary:** This contract binds the KHS-4 pipeline (#8603): candidates distilled from cross-prospect transcripts are PII-redacted (redact-or-drop), stored pending, and cross into shared knowledge ONLY via an owner decision on the admin-bearer-guarded operator endpoints, each writing an approval receipt; answer-bank publications carry the receipt ref as approved_by. It authorizes no capture path and makes no public 'learning from conversations' claim while the data.khala_free_tier_trace_capture.v1 promise family is yellow — this is an internal owner-approved store, framed as such. Nothing crosses prospects without an owner-approval receipt.

### `sarah.memory_query_scoped.v1` — ENFORCED

- **Surface:** sarah (prospect memory + conversation serving)
- **Stated by:** owner via sarah-production-conversation on 2026-07-09
- **Statement:** All prospect-scoped reads are filtered by exact prospect_ref at the query layer, not prompt-side.
- **Enforcement tier:** test-sweep
- **Oracle** `memory_query_scoping.unit` (bun-test, unit): Every prospect-scoped read helper that exists today (getSarahSessionTranscript, getSarahProspectCrmProjection, findSarahProspectByContactEmail) takes an exact prospect ref (or resolves to exactly one) and returns only that prospect's rows; an unknown ref returns empty, never a fallback to another prospect's data. — `apps/sarah/src/contracts/isolation-contracts.test.ts`
- **Oracle** `prospect_memory_query_scoping.unit` (bun-test, unit): KHS-2 memory seam (#8601, merged): every SQL read in prospect-memory.ts is bound to prospectRefAliases(prospectRef) (WHERE prospect_ref IN — exact identity re-encodings only, never a pattern or cross-prospect list), and the alias derivation plus the no-store null path are unit-oracled so a prompt-side 'filter' can never substitute for the query-layer one. — `apps/sarah/src/services/prospect-memory.test.ts`
- **Verification:** bun test src/contracts/isolation-contracts.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main.
- **Authority boundary:** This contract binds the read discipline (exact prospect_ref filter at the query layer), not the storage engine. It does not forbid owner/operator surfaces that intentionally list all prospects (e.g. session receipts for the operator dashboard); those are owner-facing, never prospect-facing, and must never feed a prospect-facing serve path. The KHS-2 prospect-memory reads (#8601) are bound by the seam oracle below.

### `sarah.no_improvised_pricing.v1` — ENFORCED

- **Surface:** sarah (pricing + deal rules)
- **Stated by:** owner via sarah-spec on 2026-07-07
- **Statement:** No improvised discounts; deal-rules code + public packs only; owner-priced params from runtime config.
- **Enforcement tier:** test-sweep
- **Oracle** `pricing_guard_text_lane.unit` (bun-test, unit): Text lane: POST /sarah/api/eve/turn with pricing pressure returns modelPath deterministic_guard with the no-improvised-discounts reply — the model path is never reached. — `apps/sarah/src/server.test.ts`
- **Oracle** `pricing_guard_avatar_brain.rpc` (bun-test, rpc): Avatar brain: POST /sarah/api/llm/chat/completions with pricing pressure answers from the deterministic guard before the model, holding the same law on the voice lane. — `apps/sarah/src/server.test.ts`
- **Verification:** bun test src/server.test.ts inside apps/sarah (pricing-guard tests on both the text lane and the avatar brain); runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main.
- **Authority boundary:** Deal-rule evaluation and human handoff remain the only pricing-adjacent actions, and the openagents.com API remains the authority for checkout and credits. Retrieval, memory, or any future Khala lookup can inform language, never prices — this guard binds regardless of what memory returns.

### `sarah.in_chat_account_linking.v1` — ENFORCED

- **Surface:** sarah (account linking + attribution)
- **Stated by:** owner via sarah-production-conversation on 2026-07-09
- **Statement:** Sarah prompts the user to create an account without leaving the conversation
- **Enforcement tier:** test-sweep
- **Oracle** `account_link_routes.rpc` (bun-test, rpc): Link routes: GET /sarah/api/account/status reports anonymous without a prospect cookie; POST /sarah/api/account/link returns 400 without a prospect cookie, 401 for an unauthenticated request, and on a verified openagents.com session upserts contact_id oa_user:<userId> + contact_email onto the prospect ref. — `apps/sarah/src/server.test.ts`
- **Oracle** `account_link_seam.unit` (bun-test, unit): Link seam units: the pure contact-row shape (oa_user: prefix, account_link mode), the single account-awareness prompt line (may suggest once, never pushy, null when the store cannot persist a link so Sarah never pitches a link that would not stick), test-mode session parsing, and the no-oa_access-cookie anonymous fast path. — `apps/sarah/src/services/account-link.test.ts`
- **Verification:** bun test src/server.test.ts and src/services/account-link.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main.
- **Authority boundary:** This contract binds the account-linking seam only (KHS-7, #8606): the openagents.com API remains the identity and credit authority; sign-in happens on the existing /login + OpenAuth rails (apps/sarah never touches password/OAuth internals and never mints sessions); identity for a link comes ONLY from the first-party session cookie verified against GET /api/auth/session, never from a request body; and the linked identity (user ref + email) lands only in sarah_prospect_contacts. The payment half of the owner's directive (attaching a card, paying in-chat) is KHS-8 (epic #8599) and must gain its own contract when it lands.

### `sarah.blueprint_versioned_provenance.v1` — ENFORCED

- **Surface:** sarah (knowledge object + persona compilation)
- **Stated by:** owner via sarah-production-conversation on 2026-07-09
- **Statement:** I want her to have that Blueprint of her own
- **Enforcement tier:** test-sweep
- **Oracle** `blueprint_compile_roundtrip.unit` (bun-test, unit): Seed → compile roundtrip stability: the checked-in seed loads as revision 1 with typed facts in every section; the committed KB doc is byte-identical to the compiled blueprint output (generated, not hand-edited); parse→render→parse is a fixpoint; and the compiled system prompt preserves Section A ordering (identity → engine → hard rules → knowledge). — `apps/sarah/src/services/sarah-blueprint.test.ts`
- **Oracle** `blueprint_revision_immutability.unit` (bun-test, unit): Revision immutability: adding and retiring facts each create a new receipted revision (changed_by + change_note); the retired fact row remains with revision_retired stamped — never deleted — while leaving the compiled surfaces; retiring twice is a conflict; every fact references the revision that added it. — `apps/sarah/src/services/sarah-blueprint.test.ts`
- **Oracle** `blueprint_provenance_required.unit` (bun-test, unit): Provenance required on every fact: all seed facts carry owner_kb_v2 with ref + timestamp; adds with an empty or unknown source (or no change note, or an invalid section) are rejected; typed pricing facts carry dealRuleRefs into the deal-rule config and product facts carry promiseIds into the public registry; an approved winning_answer learning promotes to a playbook fact whose provenance source is the KHS-4 approval receipt ref (learning_receipt:<id>), and pending candidates cannot be promoted. — `apps/sarah/src/services/sarah-blueprint.test.ts`
- **Oracle** `blueprint_admin_guard.rpc` (bun-test, rpc): Safe rollout + fail-closed guard: with SARAH_BLUEPRINT unset the file-based instructions path is unchanged (no compiled sections leak in); armed, the compiled blueprint leads while the tool protocol stays; the operator endpoints are admin-bearer-guarded (unarmed → 503, missing/wrong bearer → 401 with nothing written) and a full receipted revision cycle (add → retire → read back) works only with the exact bearer. — `apps/sarah/src/services/sarah-blueprint.test.ts`
- **Verification:** bun test src/services/sarah-blueprint.test.ts inside apps/sarah; runs in the package test glob, the apps/sarah oracle chain, and the repo test:sarah sweep before pushes to main.
- **Authority boundary:** This contract binds Sarah's knowledge object (KHS-5, #8604): her persona/playbook/knowledge live in a typed Blueprint — facts with per-fact provenance ({source, ref, at}; owner_kb_v2 | owner_directive | promise_registry | deal_rules | learning_receipt:<id>), versioned revisions (retire is a new revision, never a delete), and admin-guarded operator writes with a change note. The KB doc is GENERATED from the blueprint (render-kb-from-blueprint.ts), not hand-edited. Consumption is flag-armed (SARAH_BLUEPRINT=1); flag-off keeps the file-based path unchanged. It grants no authority: deal-rules code remains the only pricing authority and the openagents.com API the system of record — blueprint facts inform language, never prices.
