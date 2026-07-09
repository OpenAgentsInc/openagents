# Connecting Sarah to the Khala API — Assessment

Date: 2026-07-09
Status: assessment. States a direction and labels live-vs-designed
explicitly. Nothing here widens a promise-registry entry, claims capture
is live where the registry says yellow, or arms any money path.
Companions: the Sarah spec (`2026-07-07-sarah-sales-agent-spec.md`), the
consolidation plan (`2026-07-09-sarah-monorepo-effect-native-consolidation-plan.md`,
epic #8594), the Khala brain audit
(`../khala/2026-06-24-khala-brain-and-blueprint-hookup-audit.md`), and
MASTER_ROADMAP rev 6.10 (P1 Sarah, P4 employees + brain).

## 0. The thesis in one paragraph

Sarah is our AI sales employee: an owned runtime brain (Gemma 4 on our
Google inference) serving both the text lane and the LiveAvatar
voice/avatar surface at openagents.com/sarah, with every conversation
turn durably persisted to Postgres. Khala is our collective-intelligence
product: one free OpenAI-compatible endpoint over a network of agents,
plugins, and (designed) typed Blueprint programs, improved by traces and
optimization (Episode 242, `docs/transcripts/242.md`). Today the two do
not touch: Sarah calls Google inference directly with a raw
`GEMINI_API_KEY`, and her transcript turns sit in `sarah_*` tables that
feed nothing. That is a double loss — Sarah neither contributes her
richest-in-the-company source of real prospect intelligence to the
collective, nor benefits from the metering, routing, receipts, memory,
and sync rails the rest of the system runs on. Connecting her is mostly
wiring, not building: the rails exist, and doing it now converts the P4
`ai_employee.v1` promotion from a rewrite into a data migration.

## 1. What Khala actually is today (live vs designed)

Precision matters here because Sarah's own behavior contract
(`sarah.claims_bound_to_promise_registry.v1`) caps claims at promise
state — the same discipline applies to this doc.

**LIVE:**

- The free OpenAI-compatible API at `https://openagents.com/api/v1`
  (`POST /api/v1/chat/completions`, `GET /api/v1/models`) —
  `inference.khala_free_openai_compatible_api.v1` is green (registry
  `2026-06-27.1`, `docs/promises/registry.md`). One model,
  `openagents/khala`; the older khala-mini/pro/code tiering in
  `docs/khala/khala.md` is historical (`docs/khala/README.md` names the
  single endpoint).
- Gateway metering and telemetry: receipt-first token accounting from
  provider `usage` (exact-only `token_usage_events`), the typed
  `openagents.khala.telemetry.v1` disclosure block with honest
  `not_measured` sentinels (`docs/khala/khala.md` §3, §7).
- The public tokens-served counter (`metrics.khala_tokens_served_public.v1`,
  green) served through a Khala Sync projection.
- The operator trace-review loop: `GET /api/operator/khala/trace-review`
  aggregates `agent_traces`, `token_usage_events`, and
  `pylon_codex_raw_events` into failure modes, intent refs, and triage
  items — aggregates and refs only, never raw content
  (`docs/khala/2026-06-26-khala-trace-review-runbook.md`).
- The Blueprint kernel in code: Effect-Schema program/module/optimizer
  schemas, repositories, routes, migrations, and the typed signature
  selector — evidence-only Program Runs, Action Submissions as the only
  external-write path, release gates with no self-promotion
  (`docs/khala/2026-06-24-khala-brain-and-blueprint-hookup-audit.md` §4).

**YELLOW / DESIGNED (do not claim as live):**

- Free-tier trace capture: `data.khala_free_tier_trace_capture.v1`,
  `privacy.khala_paid_capture_optout.v1`, and
  `data.free_tier_capture_disclosure.v1` are yellow — capture is
  disclosed as redacted, private, `owner_only`, owner-gated, opt-out by
  paying for privacy, and grants no payout; the production capture flag
  and parts of the sink are not armed
  (`docs/promises/registry.md`; `khala_code.free_plan_trace_capture.v1`
  likewise yellow with the ingest sink missing).
- Khala turns as typed Blueprint programs: the runtime
  (`chat-program-runtime.ts`) exists but is not yet called from the
  Khala request path — labeled FUTURE in the brain audit (§3).
- The refusal → offer → guided session → skill → rev-share loop, trace
  distillation, and the accepted-outcome settlement machine (INERT,
  owner-gated) are designed, not shipped (brain audit §5–§7).

## 2. What Sarah is today (verified in-repo)

- `apps/sarah/` in the monorepo (#8594): Bun/Effect service, owned agent
  runtime, text lane + LiveAvatar avatar sessions, mounted at
  `openagents.com/sarah` (`apps/sarah/README.md`).
- Inference: **direct** calls to the Generative Language API with a raw
  `GEMINI_API_KEY`, Gemma 4 (`gemma-4-31b-it` default), thought parts
  filtered and never stored
  (`apps/sarah/src/services/google-inference.ts`). No gateway auth, no
  Khala metering receipt, and the file itself documents fighting tight
  per-model RPM quota with retry loops — exactly the problem the
  gateway's routing/fallback lanes exist to absorb.
- Persistence: every turn to `sarah_transcript_turns` (with
  `prospect_ref`), contacts linked via `sarah_prospect_contacts`, avatar
  lifecycle in `sarah_avatar_sessions`
  (`apps/sarah/src/services/turn-store.ts`, #8598 owner directive).
  Notably, the store's DSN already falls back to
  `KHALA_SYNC_DATABASE_URL` — Sarah's turns physically live next to the
  Khala Sync substrate today.
- Grounding: promise registry fetched server-side into her instructions
  (`apps/sarah/src/services/promise-registry.ts`); knowledge base is a
  static single-paste doc (`docs/sarah/SARAH_KNOWLEDGE_BASE.md`); deal
  rules are code-enforced (`sarah.no_improvised_pricing.v1`, spec §7/§10).
- Boundary law: Sarah owns only `sarah_*` tables; the openagents.com API
  stays the authority for CRM, credits, and checkout (turn-store header;
  MASTER_ROADMAP P1).

## 3. Benefits of Sarah CONTRIBUTING to Khala

Sarah's persisted turns are the most valuable conversational dataset we
generate: real prospects, real objections, real funnel language, tied to
outcomes (settled checkout receipts). Episode 242's whole claim is that
the collective **improves instead of depreciating** because traces feed
optimization. Concretely:

1. **Objection and question intelligence.** Redacted turn traces feed the
   same trace-review loop that already mines `agent_traces` for failure
   modes and recurring intents. A sales analogue of the
   unsupported-request ledger
   (`docs/khala/2026-06-26-khala-unsupported-request-list.md`) turns
   "questions Sarah couldn't answer" into knowledge-base and promise-copy
   backlog items instead of anecdotes.
2. **Optimizing the persona program with evidence.** The Blueprint
   optimizer path (GEPA-style candidates, executed evals, release gates,
   no self-promotion — brain audit §3–§4) is exactly what Sarah's
   playbook (`SARAH_KNOWLEDGE_BASE.md` §B) should be optimized against:
   which openers, objection handles, and closes correlate with settled
   receipts. Today that document is hand-tuned; connected, it becomes an
   optimizable module with her Eval Suite as the gate.
3. **Riding existing rails, not building parallel ones.** The capture
   discipline is already specified: redacted, private-by-default,
   `owner_only`, owner-gated arming, no payout implication (the yellow
   capture promises above). Sarah's contribution should be a redaction
   gate over `sarah_transcript_turns` feeding that same sink when the
   owner arms it — never a second pipeline, and never a green claim
   before the registry flips.
4. **Seeding the collective flywheel with our own employee first.** The
   Reactor/flywheel doctrine (Palantir analysis §VI: run the flywheel on
   our own traces first) applies verbatim — Sarah is the first-party
   trace source we control end to end, the safest place to prove
   trace → improvement before any customer data is involved.

## 4. Benefits of Sarah CONSUMING Khala

1. **Inference through the gateway (the gap, stated honestly).** Sarah
   currently bypasses Khala entirely: raw `GEMINI_API_KEY`, no
   `token_usage_events` row, no telemetry block, no receipt. Our flagship
   AI employee sells the collective-intelligence API while not using it.
   Migrating her text lane to `openagents.com/api/v1` with an internal
   agent token keeps the Gemma 4 model pin (the gateway routes to the
   same Google lane) while gaining exact token receipts, cost caps on the
   same rails as everything else, quota-aware fallback instead of
   hand-rolled retry loops, and one place to rotate credentials. This is
   both the honest gap and the cheapest migration.
2. **Live product truth as retrieval, not paste.** Her promise-registry
   fetch is the precedent; the knowledge layer should extend to grounded
   lookups against company-brain public-safe slices (spec §9 layer 3)
   through Khala's typed selector — never keyword routing (workspace
   semantic-routing law; brain audit "the selector is already the thing").
3. **Khala Sync for cross-surface continuity.** The relationship spans
   web text, avatar, email (SR-3), and eventually the mobile cockpit
   (AE-2.3 Agents panel). Sarah's turns already sit in the Khala Sync
   Postgres; a proper owner-scoped sync scope makes one conversation
   thread follow the prospect across surfaces and gives the owner's phone
   live visibility into Sarah's day — the same engine, no new transport
   (`docs/khala-sync/SPEC.md`).

## 5. The Blueprint tie-in: why this makes P4 a migration, not a rewrite

P4 (MASTER_ROADMAP) hardens Sarah into the formal `ai_employee.v1`
record (AE-2.1) and builds `company_brain.v1` as **Blueprint-lite**:
typed objects/properties/links with per-fact provenance,
Action-Submission writes, Access Explanation, versioned entries
(sovereignty analysis §3, `2026-07-07-palantir-institutional-sovereignty-smb-analysis.md`).
Sarah is named the generalization seed and flagship instance.

Connecting her to Khala now pre-shapes exactly the objects P4 needs:
turns become provenance-carrying trace refs, prospects/contacts become
typed links into the CRM boundary, receipts become the outcome evidence
her template listing (SR-6, P5) must carry. If we wait, P4 inherits an
unmetered inference lane and an unlinked transcript pile and has to
retrofit both; if we connect now, promotion onto the formal record is a
schema mapping over data that already has the right joints. The brain
audit's core loop — gap → offer → capture → typed skill → rev-share —
gets its first real, first-party instantiation from sales conversations
rather than a synthetic demo.

## 6. Risks and law (unchanged by any of this)

- **Prospect PII never enters collective traces.** The spec's privacy
  posture is binding: prospect data confined to the CRM boundary, no
  prospect content in public projections or traces, consent gate first,
  redaction-before-inference (spec §9). Contribution means redacted,
  `owner_only` aggregates — objection *patterns*, not people.
- **Promise-registry honesty.** Capture stays yellow until the owner arms
  it with the disclosure and opt-out live; nothing in Sarah's copy or
  ours claims trace capture, collective learning, or payout where the
  registry does not.
- **Deal rules stay code-enforced.** Retrieval can inform language, never
  prices: `sarah.no_improvised_pricing.v1` binds regardless of what any
  Khala lookup returns. Same for `close_requires_receipt` and the
  authority ladder — Khala consumption raises no authority.
- **The openagents.com API remains the system of record** for CRM,
  credits, checkout, and promises. Khala connection adds rails; it moves
  no authority.
- **Untrusted input ceiling holds.** Inbound content (email, prospect
  text) can never raise effective authority (CB-2.1); a Khala-connected
  Sarah inherits that ceiling on every new rail.

## 7. Recommended lanes (KH-S-1..6)

| Lane | Scope | Unlocks |
|---|---|---|
| **KH-S-1** Gateway inference migration | Text lane calls `openagents.com/api/v1` with an internal agent token; Gemma 4 pin preserved; retire the raw `GEMINI_API_KEY` path; exact token receipts + cost caps | Metering parity with the rest of the system; quota fallback; the dogfood credibility claim |
| **KH-S-2** Redacted turn contribution | Redaction gate over `sarah_transcript_turns` → the existing `owner_only` capture sink (armed only with the yellow-promise owner gate); turns appear in the trace-review aggregates | Objection/question/funnel intelligence in the standing operator loop |
| **KH-S-3** Sales gap ledger | Unsupported-question analogue for sales: unanswerable prospect questions → typed ledger → knowledge-base/copy backlog | The refusal→capability loop applied to selling |
| **KH-S-4** Khala Sync conversation scope | Owner-scoped sync scope for Sarah threads (already co-located in the Khala Sync Postgres) | Cross-surface continuity; cockpit visibility (AE-2.3) |
| **KH-S-5** Persona-program optimization | Sarah's turn emitted as an evidence-only `BlueprintProgramRunRecord`; GEPA candidates gated by her Eval Suite + settled-receipt outcomes; release-gate promotion only | The playbook becomes an optimizable module; first real Khala-on-Blueprint consumer |
| **KH-S-6** `ai_employee.v1` pre-shaping | Type Sarah's turns/contacts/receipts to the Blueprint-lite object vocabulary ahead of AE-2.1 | P4 promotion as data migration |

Order matters: KH-S-1 first (smallest, immediately honest, unblocks
receipts for everything after), KH-S-2/3 next (contribution, behind the
owner capture gate), KH-S-4 alongside, KH-S-5/6 with P4 sequencing.

## Sources

- `docs/khala/khala.md`, `docs/khala/README.md`,
  `docs/khala/2026-06-24-khala-brain-and-blueprint-hookup-audit.md`,
  `docs/khala/2026-06-26-khala-trace-review-runbook.md`
- `docs/transcripts/242.md` (Khala as collective intelligence),
  `docs/transcripts/247.md` (sell-in-public revenue loop)
- `docs/promises/registry.md` (green free API; yellow capture family)
- `docs/fable/MASTER_ROADMAP.md` (P1 Sarah, P4 AE-2/CB-1),
  `docs/fable/2026-07-07-sarah-sales-agent-spec.md`,
  `docs/fable/2026-07-07-palantir-institutional-sovereignty-smb-analysis.md` §3
- `apps/sarah/src/services/google-inference.ts`,
  `apps/sarah/src/services/turn-store.ts`,
  `apps/sarah/src/services/promise-registry.ts`,
  `docs/sarah/SARAH_KNOWLEDGE_BASE.md`
- `docs/khala-sync/README.md`, `docs/khala-sync/SPEC.md`
