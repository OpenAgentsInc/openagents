# Sarah Behavior Contracts

Machine source of truth: `apps/sarah/src/contracts/isolation-contracts.ts`
(schema: `@openagentsinc/behavior-contracts`). This document is the human
rendering; the coverage test in
`apps/sarah/src/contracts/isolation-contracts.test.ts` fails the sweep if this
doc drifts from the registry, if an enforced contract loses its oracle, or if
an oracle file drops its contract reference.

Lane: KHS-3 (#8602), epic #8599 (Sarah × Khala). The isolation law lands
BEFORE any shared learning: KHS-4 owner-approved collective learning (#8603)
cannot ship while these oracles are red or missing.

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

## Oracle locations

- `apps/sarah/src/contracts/isolation-contracts.test.ts` — registry
  validation + coverage, query-layer scoping, prospect-memory seam pins,
  avatar-brain injection probe.
- `apps/sarah/src/services/prospect-memory.test.ts` — the KHS-2 (#8601)
  memory seam suite: single-ref entry point, deterministic same-identity
  aliases (visitor refs never alias), fail-soft null without a store.
- `apps/sarah/src/server.test.ts` — the pricing-guard oracles (text lane +
  avatar brain).
- All run in `bun test` inside `apps/sarah`, in the `apps/sarah` `oracle`
  chain, and in the repo `test:sarah` sweep before pushes to main.

## Pending entries (blocker-gated, never claim as guaranteed)

- `sarah.collective_learning_owner_gated.v1` — **pending**, blocked on
  **#8603** (KHS-4 owner-approved shared-knowledge store). There is no
  shared-knowledge read path in `apps/sarah` today; the contract flips to
  enforced in the same change that adds one.

## Registry

Registry version: `2026-07-09.1` (schema `openagents.behavior_contracts.v1`)

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

### `sarah.collective_learning_owner_gated.v1` — PENDING

- **Surface:** sarah (collective learning)
- **Stated by:** owner via sarah-production-conversation on 2026-07-09
- **Statement:** admin approval for determining what she's able to learn generally from everyone else
- **Enforcement tier:** unenforced
- **Oracle** `approved_store_only.planned` (planned, unit): PENDING (#8603, KHS-4): when the owner-approved shared-knowledge store lands, an oracle must prove Sarah's shared-knowledge reads come ONLY from that store (never raw cross-prospect tables), that every entry carries an owner-approval receipt, and that unapproved candidate learnings are unreachable from any serve path. — `apps/sarah/src/contracts/isolation-contracts.test.ts`
- **Verification:** Pending #8603 (KHS-4 owner-approved collective learning). Until the approved store exists there is no shared-knowledge read path in apps/sarah, and this contract must flip to enforced (with the planned oracle made real) in the same change that adds one.
- **Blockers:** `#8603`
- **Authority boundary:** This contract states the gate for shared learning; it does not build the approved store, define its schema, or authorize arming any capture path. Capture-adjacent product claims stay capped while the data.khala_free_tier_trace_capture.v1 promise family is yellow. Nothing crosses prospects without an owner-approval receipt.

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
