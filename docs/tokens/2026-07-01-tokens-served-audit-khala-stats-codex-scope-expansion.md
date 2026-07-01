# Audit: expanding "tokens served" beyond Khala to all product-served tokens (incl. direct/local Codex)

Date: 2026-07-01

## Why this doc exists

The public `/khala` and `/stats` surfaces (and the homepage) show a single
headline counter, "Khala Tokens Served," plus a per-day history chart and a
model-family mix panel. The ask driving this audit: stop treating this as a
"Khala-only" number and instead count **any tokens served through our
products**, including Codex tokens, while preserving a clean split between:

1. Tokens served through the **Khala API** (our hosted inference gateway),
   and from there, which backend model actually did the work (a GLM route, a
   Fireworks/DeepSeek route, a Pylon-delegated Codex turn, etc.), vs.
2. Tokens generated **directly on a user's own computer** via our client
   tooling (e.g. a bare `codex` CLI session on a machine that has Pylon/Khala
   tooling installed) that never round-tripped through the Khala API at all.
   These should still count toward a total "served through our products"
   figure — just not be double-counted as "Khala" traffic.

This doc is a precise map of the current implementation (schema, write
paths, aggregation, page wiring), an honest answer to "is this already
Khala-specific or generic," and what concretely needs to change to add the
missing direct/local-Codex bucket and the channel taxonomy to distinguish it
from Khala-API traffic.

## 1. Current architecture, in one paragraph

There is exactly one canonical ledger table, `token_usage_events`, in the
`openagents.com` Worker's D1 database
(`apps/openagents.com/workers/api/migrations/0137_token_usage_events.sql`).
Two write paths insert rows into it today. A pure-SQL aggregation layer
(`apps/openagents.com/workers/api/src/token-usage-ledger.ts`) sums that table
with **no model or provider filter** to produce the public "Khala Tokens
Served" scalar, its per-day history, and a display-only model-family mix.
Four public routes expose those aggregates, all under
`/api/public/khala-tokens-served*`. The `/khala`, `/stats`, and homepage
pages all render the same shared components
(`apps/web/src/page/loggedOut/page/home.ts`) backed by those four routes.
There is no Durable Object state or Analytics Engine dataset involved in this
counter — it's D1 plus a live-push sync-outbox layer for instant updates.

## 2. Schema

`workers/api/migrations/0137_token_usage_events.sql`:

```
id, idempotency_key (UNIQUE), observed_at, ingested_at,
producer_system, source_route,
actor_user_id, actor_team_id, account_ref, anonymized_source_ref,
run_ref, session_ref, task_ref, repository_ref,
provider, model, backend_profile,
input_tokens, output_tokens, reasoning_tokens,
cache_read_tokens, cache_write_5m_tokens, cache_write_1h_tokens,
total_tokens,
usage_truth ('exact' | 'estimated' | 'unknown'),
cost_amount, currency,
leaderboard_eligible, privacy_opt_out,
safe_metadata_json
```

`migrations/0232_token_usage_demand_attribution.sql` later added three more
columns that matter a lot here:

```
demand_kind   TEXT NOT NULL DEFAULT 'unlabeled'   -- CHECK: internal | internal_stress | own_capacity | external | unlabeled
demand_source TEXT                                 -- free-text bounded token, e.g. 'khala_coding_delegation', 'heartbeat', 'canary'
demand_client TEXT
```

`demand_kind`/`demand_source`/`demand_client` are a **business-demand**
taxonomy (dogfood vs. real external usage vs. stress-test vs. own-capacity
delegation) — not a channel/transport taxonomy. This distinction matters
below: there is currently no column that means "did this request touch our
hosted API, or was it entirely local."

`migrations/0236_agent_traces_demand_attribution.sql` mirrors the same two
fields onto the private `agent_traces` table so a captured trace and its
ledger row always agree.

## 3. Ingestion paths today (both write the same table)

### A. Khala inference-gateway completions

`workers/api/src/inference/served-tokens-recorder.ts` builds one
`token_usage_events` row per successfully-served completion, idempotent on
`requestId`. Called from `workers/api/src/inference/chat-completions-routes.ts`,
which resolves `demand_kind`/`demand_source` from request headers
(`x-openagents-demand-kind` / `x-openagents-demand-source`). `provider` and
`model` here are whatever backend actually served the completion (GLM,
Fireworks/DeepSeek, GPT-OSS, Gemini, etc.) — this path is **already fully
generic across backend models**, not hardcoded to a "Khala" model id.

`public-khala-chat-served-tokens.ts` is the specific caller used for the
public free-tier `khala-cli-public-chat` demand source; it tags
`demandClient: 'khala-cli'`, `demandKind: 'external'`.

### B. Pylon → Codex (and Pylon → Claude) delegated coding turns

This is the existing "Codex tokens" path, and it is real, exact, and already
counted. Full flow is documented at length in the root `CLAUDE.md` under
"Khala -> Pylon -> Codex Coding Delegation Runbook." Mechanically:

1. A Khala coding request (`--workflow codex_agent_task`) creates a Pylon
   assignment (`assignmentRef`) against a caller-owned, linked Pylon.
2. The user's local Pylon runs the Codex SDK turn locally against the
   user's own connected Codex/ChatGPT account.
3. On completion, local Pylon posts the turn's exact usage to
   `POST /api/pylon/codex/turns`
   (`workers/api/src/pylon-codex-turn-ingest-routes.ts`), which inserts a
   `token_usage_events` row with `provider: 'pylon-codex-own-capacity'`,
   `model: 'openagents/pylon-codex'`, `demand_kind: 'own_capacity'`,
   `demand_source: 'khala_coding_delegation'`, `usage_truth: 'exact'`.
   Reasoning output tokens are folded into `output_tokens` (and kept
   separately in `reasoning_tokens`) so they count toward the public total.
4. A sibling route, `POST /api/pylon/claude/turns`, does the same for a
   Pylon-Claude own-capacity lane.

**Critically: this ingest route requires `assignmentRef`.** It is only
reachable when Khala itself issued the delegation. There is no ingest path
for Codex usage that didn't originate from a Khala-issued assignment.

## 4. Aggregation — is the counter already Khala-model-specific?

**No — it is already fully generic, and this predates today's ask.** The
literal WHERE clause backing the scalar counter and the history chart is:

```ts
// workers/api/src/token-usage-ledger.ts:281
const publicTokensServedDemandWhere = `1 = 1`
```

with an explicit comment: *"Public Khala token counters are total-only:
every real served-token ledger row counts, including internal dogfood,
`internal_stress`, `own_capacity`, external, and unlabeled demand."* So the
existing Pylon-Codex-delegated rows (path B above) **already count** toward
the headline "Khala Tokens Served" number and already appear as a distinct
`pylon_codex` family in the model-mix panel (label "Pylon-Codex"). This is
confirmed in `apps/openagents.com/docs/stats/2026-06-26-stats-page-audit.md`.

So the premise "we'd been marking all of those as Khala tokens" is true only
in the sense that the *label* on the counter says "Khala Tokens Served" and
everything in it is, definitionally, tokens that flowed through the Khala
Worker at some point (either as a direct completion, or as a Codex turn
Khala delegated out and got reported back). It is **not** true that the
counter filters by model — it already includes non-GLM backends and already
includes Pylon-Codex rows.

## 5. The actual gap: direct/local Codex usage that never touches Khala

What is **not** tracked anywhere, confirmed by reading every write path into
`token_usage_events` and every Pylon local-usage code path:

- A user running the bare `codex` CLI on their own machine, with their own
  ChatGPT/OpenAI account, with **no** Khala-issued assignment behind it (no
  `assignmentRef`), produces **zero** rows in `token_usage_events`. There is
  no ingest route for this shape of usage at all.
- Pylon *does* have a local usage-observation mechanism —
  `apps/pylon/src/account-usage.ts` (`PylonLocalSessionUsageSnapshot`,
  `PylonProviderRateLimitSnapshot`) reads back rate-limit-window usage
  percentages and, where available, exact session token counts, via
  `pylon accounts usage --account <ref> --refresh`. This is explicitly
  called out in `CLAUDE.md` as diagnostic only: *"That refresh ... proves
  the local Codex login works, but it is not the Khala counter proof."* It
  is stored locally and is never posted to the Worker as a
  `token_usage_events` row.
- `clients/khala-code-desktop` only reads back server-side ledger/proof data
  for assignments it itself created; it does not observe or report
  unrelated local Codex activity.

In short: **Codex-via-Khala-delegation is counted; Codex used purely
locally, off-Khala, is invisible and structurally cannot be counted today**
— there is no route that accepts a "here's usage that happened without an
assignment" report.

## 6. The channel-taxonomy gap

Even setting aside the missing ingestion, there is no existing field that
cleanly answers "did this request go through our hosted Khala API, or was it
reported after the fact from something that ran entirely on the user's
machine." The closest existing concepts:

- `demand_kind` (`internal | internal_stress | own_capacity | external |
  unlabeled`) is a **business-demand** classification (dogfood vs. real
  external users vs. stress test vs. delegated own-capacity work), not a
  transport/channel classification. `own_capacity` + `demand_source =
  'khala_coding_delegation'` is the closest analog today, but it still means
  "delegated *by* Khala" — every row in this bucket required a live
  round-trip *through* the Khala Worker (the assignment creation, then the
  turn-ingest POST). It does not cover "never touched Khala at all."
- `producer_system` / `source_route` (schema literals: `producer_system ∈
  {probe, omega, provider_broker, shc_runner, manual, unknown}`,
  `source_route ∈ {probe_direct_provider, probe_local_model,
  omega_provider_broker, omega_hosted_gemini, shc_runner_callback, manual,
  unknown}`) — `probe_local_model` is the one existing literal that means
  "ran against a model locally," but it's Probe/local-model-specific,
  unrelated to Codex, and only surfaced on the internal/logged-in stats page
  (`apps/web/src/page/loggedIn/page/stats.ts`), never on the public
  `/khala` or `/stats` pages.
- A demand-*mix* endpoint already exists
  (`GET /api/public/khala-tokens-served/demand-mix`,
  `public-khala-tokens-served-demand-mix-routes.ts`) that groups by
  `demand_kind`/`demand_source`/`demand_client`, but **it is not wired into
  any page UI today** — it's a live, tested, unused API.

## 7. What needs to change

### 7.1 Add a channel dimension, distinct from demand_kind

Add a new bounded column, `demand_channel` (or similar; naming is a product
decision, not just an engineering one — see open questions), with values
along the lines of:

- `khala_api` — the request was served (or delegated) through our hosted
  Khala Worker. This covers *both* existing write paths (A and B above).
- `direct_local` — usage happened entirely on the user's own machine via our
  client tooling (Pylon, Khala Code Desktop, a connected Codex account) and
  was self-reported to us after the fact without ever being an inline Khala
  API call.

This keeps `demand_kind` doing what it already does well (dogfood vs. real
external vs. stress vs. delegated) and adds the orthogonal dimension the
current ask actually needs. Every existing row backfills to `khala_api`
(all current write paths are inline API calls or Khala-issued delegations).

### 7.2 Build the missing ingestion path for direct/local Codex usage

This is the real gap, not a labeling change. Needs a new, narrow, public-safe
ingest route (e.g. `POST /api/pylon/codex/local-usage` or a new
`demand_channel: 'direct_local'` variant on the existing turn-ingest route)
that:

- Does **not** require a live `assignmentRef` / Khala-issued delegation.
- Accepts periodic, batched, idempotent usage reports from a standing local
  Pylon for Codex sessions that ran with no Khala involvement (sourced from
  the same `account-usage.ts` local-session-usage snapshot mechanism that
  already exists for diagnostics, wired up to actually persist instead of
  staying purely local).
- Marks `usage_truth` honestly: `'exact'` when the Codex SDK gives exact
  token counts for the local session, `'estimated'` when only rate-limit
  window percentages are available (do not silently upgrade estimated data
  to exact).
- Stays within the existing "no raw prompts/completions/paths/secrets" floor
  already enforced for `token_usage_events` and `safe_metadata_json`
  (`INVARIANTS.md` "Canonical Token Usage Ledger").
- Needs explicit opt-in from the user before Pylon starts self-reporting
  ambient local Codex usage — this is usage that has nothing to do with a
  request the user made *to* us, so silently phoning home token counts for
  unrelated local work is a consent question, not just a schema question.

### 7.3 Extend the model-mix vocabulary

The public model-family enum
(`packages/sync-schema/src/token-usage-ledger.ts`,
`PublicKhalaTokensServedModelFamily`) already has `pylon_codex` for
Khala-delegated Codex turns. A `direct_local` channel needs its own family
label (e.g. `codex_direct`) so the model-mix panel can show "Codex (direct)"
separately from "Pylon-Codex" (delegated-via-Khala), rather than collapsing
both into the same bucket.

### 7.4 Update the aggregation query and the public counter's meaning

`publicTokensServedDemandWhere = "1 = 1"` should very likely stay `1 = 1`
for the **total** "tokens served through our products" figure — that's
exactly the union the new ask wants at the top level. What changes is:

- The `/khala`-page counter's copy/semantics: today it's explicitly scoped
  and labeled as a Khala/network counter ("Khala Tokens Served" /
  "the homepage's network-wide aggregate"). Once `direct_local` rows are
  in the same table, that headline number silently becomes a broader
  "tokens served through OpenAgents products" number unless the UI is
  updated to say so and/or split it.
- Wire the **already-built** demand-mix endpoint (or a new channel-mix
  endpoint following the same pattern) into `/stats` so `khala_api` vs.
  `direct_local` is visible, instead of being an unused API.

### 7.5 Decide the headline framing (product decision, not engineering)

Two reasonable shapes, worth deciding explicitly before implementing:

- **Option A — one big total, sub-labeled.** Keep one headline counter
  ("Tokens Served," renamed from "Khala Tokens Served"), backed by `1 = 1`
  over the whole table, with a channel-mix panel underneath showing the
  `khala_api` vs. `direct_local` split (and, within `khala_api`, the
  existing model-family split).
- **Option B — two counters.** Keep "Khala Tokens Served" scoped exactly as
  today (`demand_channel = 'khala_api'`), and add a second, clearly-labeled
  "Total Tokens Served Through OpenAgents Products" that unions both
  channels. This avoids any appearance of quietly inflating the existing
  "Khala" metric with tokens that never touched Khala's own inference path.

## 8. Everything already correct / does not need to change

- The core ledger table, idempotency design, and `usage_truth` distinction
  already generalize cleanly to a third+ producer; no schema rework needed
  beyond the new channel column.
- The aggregation layer is already model-agnostic (`1 = 1`); the "all real
  rows count" policy in ADR
  `docs/adr/0009-count-served-tokens-from-exact-usage-ledger-rows.md` already
  states the right principle (exact ledger rows as usage truth, no synthetic
  counter deltas) — the new direct/local ingestion just needs to follow the
  same rule with an honest `usage_truth`.
- Khala-delegated Codex accounting (path B) is solid today and needs no
  changes other than tagging it `demand_channel = 'khala_api'` for
  consistency with the new dimension.

## 9. Open questions for the owner

1. Naming: does the headline counter get renamed away from "Khala Tokens
   Served," or does a second counter get added alongside it (Option A vs.
   B in §7.5)?
2. Consent: should Pylon require an explicit opt-in before it starts
   self-reporting ambient/local Codex usage that has no Khala request
   behind it, given it's reporting on activity the user didn't direct at
   us?
3. Precision: for local Codex sessions where only rate-limit-window
   percentages are available (not exact token counts), is an `estimated`
   row acceptable in the public total, or should direct/local rows be
   gated on exact SDK usage only (mirroring ADR 0009's "no synthetic burn"
   stance)?
4. Scope of "our products" for this ask: does "direct to Codex" also cover
   other agent CLIs OpenAgents tooling wraps (e.g. a future Claude Code /
   Gemini CLI direct-local channel), or is this Codex-specific for now with
   the channel design left open for more direct-local providers later?

## 10. Key files referenced

- `apps/openagents.com/workers/api/migrations/0137_token_usage_events.sql`
- `apps/openagents.com/workers/api/migrations/0232_token_usage_demand_attribution.sql`
- `apps/openagents.com/workers/api/migrations/0236_agent_traces_demand_attribution.sql`
- `apps/openagents.com/workers/api/src/token-usage-ledger.ts`
- `apps/openagents.com/workers/api/src/inference/served-tokens-recorder.ts`
- `apps/openagents.com/workers/api/src/public-khala-chat-served-tokens.ts`
- `apps/openagents.com/workers/api/src/pylon-codex-turn-ingest-routes.ts`
- `apps/openagents.com/workers/api/src/public-khala-tokens-served-routes.ts`
- `apps/openagents.com/workers/api/src/public-khala-tokens-served-history-routes.ts`
- `apps/openagents.com/workers/api/src/public-khala-tokens-served-model-mix-routes.ts`
- `apps/openagents.com/workers/api/src/public-khala-tokens-served-demand-mix-routes.ts` (built, unused by any page)
- `apps/openagents.com/packages/sync-schema/src/token-usage-ledger.ts`
- `apps/openagents.com/apps/web/src/page/loggedOut/page/home.ts`
- `apps/openagents.com/apps/web/src/page/loggedOut/page/stats.ts`
- `apps/openagents.com/apps/web/src/page/khala-chat/page.ts`
- `apps/openagents.com/apps/web/src/route-table.ts`
- `apps/pylon/src/account-usage.ts`
- `apps/openagents.com/INVARIANTS.md` ("Canonical Token Usage Ledger",
  "Captured Trace Demand-Origin Segmentation")
- `apps/openagents.com/docs/stats/2026-06-26-stats-page-audit.md`
- `docs/adr/0009-count-served-tokens-from-exact-usage-ledger-rows.md`
- root `CLAUDE.md` ("Khala -> Pylon -> Codex Coding Delegation Runbook")
