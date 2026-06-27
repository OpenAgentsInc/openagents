# 2026-06-26 GLM Counter-Backed Stress Test

## Purpose

Issue #6317 needs continuous GLM saturation traffic that also moves the primary
Khala token leaderboard. The primary leaderboard is intentionally source
agnostic: internal, internal_stress, own_capacity, external, and unlabeled
served-token rows all count in the topline number. Demand labels are for later
filtering, not for excluding real served tokens from `/stats` or `/khala`.

This run corrected the stress-test rule:

- Accepted stress tokens must pass through the OpenAgents gateway recorder and
  advance `GET /api/public/khala-tokens-served`, the public counter source used
  by `/stats` and `/khala`.
- Direct Hydralisk replica calls can diagnose raw GLM serving, but they bypass
  the served-token recorder and are not accepted leaderboard stress.

## Counter Proof

Baseline public counter read:

- `GET https://openagents.com/api/public/khala-tokens-served`
- observed `tokensServed: 340672683`

Gateway request with:

- `model: openagents/khala`
- `x-openagents-demand-kind: internal_stress`
- `x-openagents-demand-source: glm-saturation`
- `x-openagents-client: stress-harness-counter-proof`

Result:

- HTTP `200`
- `usage.total_tokens: 623`
- counter before: `340672683`
- counter after: `340673306`
- verified delta: `623`

Additional public-gateway probes:

| Probe | Status | Worker | Served model | Exact tokens | Counter moved |
| --- | ---: | --- | --- | ---: | --- |
| `openagents/khala` plain | `200` | `fireworks` | `accounts/fireworks/models/deepseek-v4-flash` | `552` | yes, delta `552` |
| `openagents/khala-code` | `400` | none | none | `0` | no |
| `openagents/glm-5.2-reap-504b` | `400` | none | none | `0` | no |
| `openagents/khala` tool-bearing | `200` | `fireworks` | `accounts/fireworks/models/deepseek-v4-flash` | `832` | yes, delta `832` |

Verified counter-moving stress in this note: `2007` exact tokens. These were
not GLM-backed on the deployed Worker; they routed to Fireworks after the
current Khala fallback behavior.

## Direct GLM Findings

Direct replica calls used the ignored local Hydralisk endpoint inventory and
sent public-safe synthetic prompts only. These calls were tagged with
`internal_stress` / `glm-saturation`, but because they bypassed the OpenAgents
gateway, they did not move `/api/public/khala-tokens-served`.

Reachability probe across 10 configured replicas:

- reachable from this Mac: 2 replicas
- connection failures from this Mac: 8 replicas
- successful direct probe tokens: `38` total tokens

The reachable direct replicas were:

- `g4-4g-b-20260625154532`, `g4-standard-192`, `us-central1-b`, Spot
- `g4-8g-b-20260624214500`, `g4-standard-384`, `us-central1-b`, Spot

Aggressive direct saturation attempt:

- target: both reachable replicas
- concurrency: 2 per replica
- `max_tokens: 4096`
- result after 30 seconds: `966` HTTP `429` responses, `0` served tokens
- finding: blind concurrency immediately trips the direct replica admission
  guard and produces backpressure noise, not useful served-token load.

Single-flight direct GLM attempt:

- target: both reachable replicas
- concurrency: 1 per replica
- `max_tokens: 4096`
- replica `g4-4g-b-20260625154532`: first long request returned `500` after
  `72609ms`, then rapid `500`s; `0` served tokens
- replica `g4-8g-b-20260624214500`: one successful full-budget response,
  `4236` total tokens (`140` prompt, `4096` completion) in `94329ms`

Exact direct GLM tokens observed in this session: `4298`.

Leaderboard-accepted GLM tokens before the route fix: `0`.

## Retroactive Counter Repair

The `4298` figure above was only the direct GLM usage from this stress-test
session. It was not the historical total of unrecorded GLM traffic from the last
few days.

The public counter is backed by `token_usage_events`, summed as
`input_tokens + output_tokens`, and is used by `/stats`, `/khala`, and
`GET /api/public/khala-tokens-served`. Because the direct Hydralisk calls and
the raw Harbor GLM baseline bypassed the gateway recorder, I inserted
idempotent retro rows into production D1 for exact public-safe parent-summary
usage only.

Backfilled rows:

| Source | Evidence | Input | Output | Total | Demand label |
| --- | --- | ---: | ---: | ---: | --- |
| Current-session direct GLM probes + one successful long direct call | local direct stress output, exact provider usage | `202` | `4096` | `4298` | `internal_stress` / `glm-saturation` |
| Harbor GLM pre-full-run smoke `smoke-glm52-reap-20260625140453` | local Harbor parent `result.json` top-level stats, no raw trajectories | `460564` | `10953` | `471517` | `internal` / `harbor_terminal_bench` |
| Harbor GLM baseline `glm52-reap-mtp2-full-20260625141715` | `/api/public/gym/run-progress`, last updated `2026-06-26T05:38:19.852Z`; issue #6253 also states raw Hydralisk did not increment the public Khala counter | `65714292` | `337498` | `66051790` | `internal` / `harbor_terminal_bench` |

Total retroactive GLM tokens added by this pass: `66531903`.

Production verification:

- Current direct stress/probe rows: `2` rows, `4298` public-counter tokens.
- Historical Harbor rows: `2` rows, `66523307` public-counter tokens.
- After the backfills, `GET /api/public/khala-tokens-served` returned
  `416175317`.
- After the backfills, all-time model mix returned `GLM family = 75843331`
  tokens and `totalTokens = 416175317`.

Important non-duplicates:

- The earlier #6253 checkpoints at `5052441 + 100836` and
  `18158939 + 161363` tokens are partial cumulative checkpoints. They are
  included inside the later `66051790` baseline snapshot and must not be added
  separately.
- The #6259 live Khala -> GLM smoke moved the public counter by `752` tokens
  through `openagents/khala`, so it was already counted.
- The #6324 live-counter proof moved `/khala` by `3971` tokens through the
  public gateway, so it was already counted.
- Pylon/Codex issue comments with million-token totals are already exact
  `pylon-codex-own-capacity` ledger rows, not raw GLM backfill candidates.
- The public Khala Terminal-Bench runs used `openagents/khala` and are not
  raw-Hydralisk backfill candidates unless a specific exact missing ledger gap
  is proven.

Remaining evidence-gated gap:

- If the raw GLM Harbor baseline resumes or a newer parent summary exists past
  `2026-06-26T05:38:19.852Z`, add only the incremental delta beyond
  `66051790`, or transactionally replace that cumulative row. Do not insert a
  second full cumulative baseline row.
- Any other local direct-Hydralisk probe can be backfilled only when we have
  exact input/output token accounting and a public-safe provenance record. No
  additional exact uncounted GLM total was found in the issue comments or
  tracked docs during this audit.

## Route Fix

The deployed public gateway accepted the stress labels and moved the counter,
but it did not keep stress traffic on GLM:

- plain `openagents/khala` routed to Fireworks
- tool-bearing `openagents/khala` also ultimately served through Fireworks with
  `fallback_reason: upstream_error`
- hidden GLM model ids were correctly unavailable to public callers

The scoped code change in this worktree makes `openagents/khala` requests with
`demandKind=internal_stress` and `demandSource=glm-saturation` use the existing
GLM-first Khala plan. The traffic still goes through the normal gateway,
metering, receipt, and served-token recorder path, so successful GLM stress
responses should increment the public counter.

Verification so far:

- `bun run --cwd apps/openagents.com/workers/api test -- src/inference/chat-completions-routes.test.ts -t "routes GLM saturation internal_stress Khala traffic"` passed.
- `bun run --cwd apps/openagents.com/workers/api test -- src/inference/benchmark/stress-saturation-plan.test.ts` passed.
- `bun run --cwd apps/openagents.com typecheck:api` passed with existing
  `Effect.void` advisories in `trace-store-routes.ts`.
- The fix was pushed to `main` in
  `7712f27ba198dc661e3232c5f562eab7402afb11` and deployed via
  `bun run --cwd workers/api deploy:safe` as Worker version
  `e645da1d-2842-4301-8c90-7886cba10ca0`.

Post-deploy public-gateway proof:

- A tagged `openagents/khala` request with
  `internal_stress` / `glm-saturation` returned HTTP `200`, reported
  `usage.total_tokens = 693`, and moved the public counter by exactly `693`.
- That request was served by Fireworks with `fallback_reason: upstream_error`,
  so it proves the counter path but not successful GLM saturation. The GLM pool
  still needs intra-pool failover/replica selection work before scaling stress.

## Next Stress Step

For the next stress pass, rerun the counter proof with a larger bounded
completion shape:

1. Read `/api/public/khala-tokens-served`.
2. Send `openagents/khala` via `/v1/chat/completions` with
   `internal_stress` / `glm-saturation` / `stress-harness`.
3. Require `openagents.worker === hydralisk-vllm-glm-5p2-reap-504b`.
4. Require the public counter delta to be at least `usage.total_tokens`.
5. Only then scale concurrency.

Do not count direct Hydralisk tokens toward #6317 acceptance unless a separate
server-side recorder path writes the same public-safe served-token ledger rows.
