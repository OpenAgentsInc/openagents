# openagents.com/stats — deep audit & spec (2026-06-26)

Goal: a public `openagents.com/stats` page showing (1) the **total Khala tokens
served** count, (2) a **tokens-per-day graph in Central Time**, and (3) the
**model / provider mix** (GLM vs Fireworks vs Codex vs …, as percentages). This
doc audits what exists, the real data shape, the gaps, and a concrete build spec.

Status: audit/spec only. Implementation is broken into the issues at the end.

---

## 1. What exists today (audited in `apps/openagents.com`)

### 1a. Public tokens-served counter (realtime) — EXISTS
- `GET /api/public/khala-tokens-served` → `{ tokensServed, schemaVersion:
  "openagents.public_khala_tokens_served.v1", … }`
  (`workers/api/src/public-khala-tokens-served-routes.ts`).
- Realtime push over the sync scope `public-khala-tokens-served:network`
  (WebSocket; `SyncRoomDurableObject`, throttled ≤3/s via
  `sync-broadcast-throttle.ts`, #6324). Client countup on `/khala`:
  `apps/web/src/page/loggedOut/khala-tokens-served-countup*.ts` +
  `khala-tokens-served-feed.ts`.
- **Reusable as-is** for the /stats headline count.

### 1b. Per-day history — EXISTS but **UTC-only**
- `GET /api/public/khala-tokens-served/history` → `{ schemaVersion:
  "openagents.public_khala_tokens_served_history.v1", window:"30d",
  bucket:"day", series:[{ day:"YYYY-MM-DD", tokensServed }], staleness:{…} }`.
- Buckets by **UTC calendar day** (`date(observed_at)` semantics). This is the
  core gap for requirement (2): a Central-time viewer's late-night work rolls
  into the next UTC day. Tracked as **#6330**. Verified concretely
  (2026-06-26): the 24th shows **0 in UTC** but **14.68M in America/Chicago**.

### 1c. Token-usage ledger aggregate — EXISTS (admin/internal, not public-page)
- `token-usage-ledger.ts` + `token-usage-ledger-routes.ts`:
  `GET /api/stats/token-usage/{events,aggregate,leaderboards}`.
- `aggregate` already groups by **provider** and **model** with filter support
  (`since`, `provider`, `model`, `producerSystem`, `usageTruth`, …) and ships a
  **public-safety redactor** (the credential/prompt/source scrub regexes) and a
  `public_tokens_served_aggregate` field. This is the right backend to derive the
  model/provider mix from — but it is an operator/ledger surface, not a
  public-page projection.

### 1d. A public `/stats` PAGE — DOES NOT EXIST in this monorepo
- No `/stats` route in `workers/api` or `apps/web`. (The legacy
  `openagents.com/stats` referenced in old root-workspace docs was the
  deprecated Laravel site, not this Worker.) `/khala` is the closest existing
  surface — it has the counter + countup but no per-day chart or model mix.

---

## 2. Data model — `token_usage_events` (D1, source of truth)

Relevant columns (full list in the table; the counter + history + ledger all
project from this one table):
- `observed_at` (UTC ISO 8601) — the event time; bucket source.
- `total_tokens` (incl. reasoning tokens in the served total), plus
  `input_tokens`/`output_tokens`/`reasoning_tokens`/cache fields.
- `model`, `provider`, `backend_profile` — the mix dimensions.
- `demand_kind` (`own_capacity` | `internal` | `unlabeled` | `external`),
  `demand_source`, `demand_client` — the public/internal segmentation (#6298).
- `usage_truth` (`exact` | …), `leaderboard_eligible`, `privacy_opt_out`.

Projection staleness pattern to reuse: `projection_staleness.v1`
(`composition:"live_at_read"`, `maxStalenessSeconds:0`, `rebuildsOn`) — already
used by the history endpoint and `public-activity-timeline.ts`.

---

## 3. Real data snapshot (2026-06-26, all-time) — grounds the "mix" UI

| Model | Provider | Reqs | Tokens |
|---|---|---:|---:|
| `openagents/pylon-codex` | `pylon-codex-own-capacity` | 94 | **141.3M** |
| `accounts/fireworks/models/deepseek-v4-flash` | `fireworks` | 3,785 | **87.1M** |
| `openagents/glm-5.2-reap-504b` | `hydralisk-vllm-glm-5p2-reap-504b` | 1,009 | 5.54M |
| `z-ai/glm-5.2-20260616` | `openrouter-khala-glm-fallback` | 580 | 2.44M |
| `openai/gpt-oss-20b` | `hydralisk-vllm` | 395 | 0.85M |
| `glm-5.2-reap-504b-g4` | `hydralisk-vllm-glm-5p2-reap-504b` | 13 | 0.017M |
| `gemini-3.5-flash` | `google_gemini` | 1 | ~0 |

demand_kind split: `own_capacity` 141.3M · `unlabeled` 77.5M · `internal` 18.4M.

**Key insights the page must reflect honestly:**
- **GLM-REAP is currently a *small* share** of served tokens (~5.5M). The bulk
  is **Pylon-Codex own-capacity (141M)** and the **Fireworks DeepSeek fallback
  (87M)** — the latter is the #6310 tool-calling fallback lane (tool-bearing
  requests route off GLM). A naive "GLM vs other" chart would mislead; the mix
  must name the real lanes.
- **Model grouping is required**: the "GLM family" spans **3** ids/providers
  (`openagents/glm-5.2-reap-504b`, `z-ai/glm-5.2-20260616` via OpenRouter
  fallback, `glm-5.2-reap-504b-g4`). Group to a canonical family for the %.
- **all real demand counts**: the public counter and `/stats` projections include
  `internal`, `internal_stress`, `own_capacity`, `external`, and unlabeled rows.
  They stay public-safe by exposing only aggregate totals and grouped families,
  never demand labels, per-user rows, prompts, provider payloads, or secrets.

---

## 4. Requirements → gaps

| # | Requirement | Status / gap |
|---|---|---|
| R1 | Total tokens-served count | ✅ reuse `/api/public/khala-tokens-served` + realtime countup |
| R2 | Tokens-per-day graph, **Central Time** | ⚠️ history is **UTC-only** → need DST-aware America/Chicago bucketing (**#6330**) |
| R3 | Model/provider mix % | ❌ no public model-mix endpoint; ledger `aggregate` is admin + needs a public-safe projection + family grouping |
| R4 | The `/stats` page itself | ❌ does not exist |

---

## 5. Proposed design

### 5a. Page (`apps/web`, dark operational aesthetic, matches `/khala`)
1. **Headline counter** — total tokens served, reuse the realtime countup
   component (smooth ≤3/s).
2. **Tokens-per-day chart** — bar/area, **Central Time** day buckets, last 30d.
   Label axis "America/Chicago". Tooltip = exact tokens + req count.
3. **Model / provider mix** — a donut or 100%-stacked bar of the grouped
   families (GLM family · Fireworks DeepSeek · Pylon-Codex · GPT-OSS · Gemini ·
   OpenRouter-GLM-fallback) + a small table with tokens, reqs, %.
4. No public/all split in the default public projection: the public numbers are
   the all-demand aggregate. Internal/external segmentation belongs in
   authenticated analytics, not in the public scalar.
- Three.js / `@openagentsinc/three-effect` is the house first choice for the
  chart/visualization per workspace UI guidance; a clean SVG/canvas bar chart is
  an acceptable fallback. Reuse the `/khala` styling.

### 5b. Endpoints (public-safe projections off `token_usage_events`)
1. **Extend** `GET /api/public/khala-tokens-served/history?tz=America/Chicago`
   — DST-aware day bucketing. Implementation: bucket on
   `datetime(observed_at, <offset>)` is **not** DST-safe with a fixed offset; do
   one of: (a) aggregate hourly UTC in SQL and re-bucket to the viewer tz with a
   real tz library / `Intl` in the worker, or (b) use a tz-offset table that
   accounts for the CDT/CST transition. Default tz stays UTC for back-compat.
2. **New** `GET /api/public/khala-tokens-served/model-mix?window=30d` —
   `{ schemaVersion:"openagents.public_khala_model_mix.v1", window, totalTokens,
   groups:[{ family, label, tokens, reqs, pct }], staleness:{…} }`. Public-safe:
   aggregates only, **no per-user**, all demand included (matches the counter),
   reuse the ledger's public-safety redactor + `projection_staleness.v1`. Derive
   `family` from a canonical model→family map (below).
3. Headline reuses the existing counter (1a).

### 5c. Canonical model → family map (single source of truth)
```
openagents/glm-5.2-reap-504b        -> glm        (GLM 5.2 REAP, own GPU)
glm-5.2-reap-504b-g4                 -> glm
z-ai/glm-5.2-20260616               -> glm        (OpenRouter GLM fallback)
accounts/fireworks/.../deepseek-v4-flash -> fireworks_deepseek
openagents/pylon-codex              -> pylon_codex (own-capacity Codex delegation)
openai/gpt-oss-20b / -120b          -> gpt_oss
gemini-3.5-flash / *gemini*         -> gemini
(unknown)                            -> other      (never drop; bucket as other)
```
Keep this map server-side; the page renders labels + colors from the response.

---

## 6. Privacy / invariants (do not regress)
- Public-safe **aggregates only** — never per-user rows, never raw prompts/
  completions/credentials. Reuse the ledger public-safety redactor.
- Include every real served-token row in the public aggregate (`internal`,
  `internal_stress`, `own_capacity`, `external`, and unlabeled). #6298
  segmentation remains available for authenticated analytics/corpus hygiene, not
  for subtracting internal rows from the public scalar.
- `projection_staleness.v1` (`live_at_read`) on every projection; register the
  new surface in the projection-freshness inventory + OpenAPI + exact-route
  manifest (the repo's `check:deploy` enforces these).
- Read `apps/openagents.com/INVARIANTS.md` (public projection + tokens-served
  monotonic/no-double-count invariants) before changing the counter math.

---

## 7. Implementation plan (issues to file)
- **S1 — Central-time + model-mix endpoints**: add `?tz=` (DST-aware) to the
  history endpoint (#6330) and the new public `…/model-mix` endpoint with the
  family map, public-safety, staleness, OpenAPI + manifest + freshness registration,
  tests (UTC vs Chicago bucketing differs at the day boundary; mix percentages
  sum to 100; internal/all demand included).
- **S2 — the `/stats` page**: counter + Central per-day chart + model-mix
  visualization, reusing the `/khala` realtime countup and styling.
- Depends on / relates to: #6330 (Central history), #6298 (demand segmentation),
  #6324 (counter realtime/throttle), the token-usage ledger aggregate.
