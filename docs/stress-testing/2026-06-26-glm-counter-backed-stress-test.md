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

## External-Wins Probe

A bounded #6318 probe on 2026-06-27 attempted to prove live external-wins
preemption:

1. Start one authenticated `internal_stress` / `glm-saturation` streaming
   request with public-safe synthetic content.
2. Send one authenticated `external` / `public-api` request while the stress
   request is active.
3. Require the external response to include scheduler-preemption metadata and
   the stress response to yield.

The first two attempts did not reach routing:

- unauthenticated requests returned HTTP `401`;
- the local Pylon agent token returned HTTP `402 insufficient_credits`.

Using the local Khala CLI token reached the inference path. The external request
returned HTTP `200`, served through `fireworks`, reported `599` exact tokens,
and moved the public counter by `599`. That is valid counter evidence for the
external request, but it is **not** #6318 acceptance evidence:

- the external response did not include scheduler-preemption metadata;
- the internal stress stream was still active after the bounded `30s` local
  wait and was aborted by the local client;
- therefore, the live external-wins preemption invariant was not proven.

The route-admission bug found by this probe: Hydralisk route-admission headroom
was still allowed to count degraded/reclaimed heartbeat states as external
capacity. The follow-up code fix excludes degraded replicas from reserved
external headroom. Post-deploy proof still must rerun this external-wins probe
and require scheduler-preemption metadata before #6318 can close.

## 2026-06-27 Ramp Continuation

Readiness immediately before the continuation:

- `GET /v1/gateway/readiness` was ready with one servable Hydralisk model.
- `GET /v1/gateway/glm-fleet/readiness` reported `10` configured replicas,
  `2` ready replicas, `2` total ready inflight slots, `8` reclaimed replicas,
  and `0` warm replicas.
- The ready replicas were `g4-4g-b-20260625154532` and
  `g4-8g-b-20260624214500`, each with `maxInflight=1`.

Public-gateway ramp through `POST /v1/chat/completions`, model
`openagents/khala`, and
`internal_stress` / `glm-saturation` / `stress-harness-ramp`:

| Wave | Concurrency | Max tokens | Requests OK | Response tokens | Served backend | Public counter proof |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| 1 | `1` | `512` | `1` | `1134` | Fireworks DeepSeek | exact `+1134` |
| 2 | `2` | `2048` | `2` | `5340` | Fireworks DeepSeek | counter moved `+5965`; `5340` attributable to these responses, remainder concurrent live traffic |
| 3 | `4` | `4096` | `4` | `16898` | Fireworks DeepSeek | exact `+16898` |

Gateway ramp total from completed waves: `23372` counted tokens, all served by
Fireworks DeepSeek, `0` served by GLM. The public counter path held, but the
deployed stress route still did not land on GLM. Wave 4 was stopped after the
pattern was clear to avoid burning fallback capacity.

Direct Hydralisk GLM saturation then targeted the two ready replicas directly
for an eight-minute single-flight pass. These direct calls are raw replica
diagnostics, not public-gateway acceptance traffic, unless and until exact usage
is written to the served-token ledger.

| Replica | Result | Exact tokens |
| --- | --- | ---: |
| `g4-4g-b-20260625154532` | `93` requests, `0` OK, `93` HTTP `500`; the public readiness endpoint marked it ready, but completions were functionally unavailable | `0` |
| `g4-8g-b-20260624214500` | `6` requests, `5` OK, `1` timeout; three full `8192`-completion responses, one short early-stop response, and a tiny probe | `25130` |

The useful 8-GPU replica produced:

- input tokens: `438`
- output tokens: `24692`
- total tokens: `25130`
- long full-budget successes: three responses of `8280` total tokens each
  (`88` prompt + `8192` completion)
- long-response latency: roughly `146s` to `161s` per full decode

I inserted the exact direct GLM usage as one idempotent retro row in production
D1:

- idempotency key:
  `inference:served-tokens:retro.issue-6317.direct-glm.ramp-20260627T041023Z`
- provider/backend: `hydralisk-vllm-glm-5p2-reap-504b`
- model: `openagents/glm-5.2-reap-504b`
- usage truth: `exact`
- demand label: `internal_stress` / `glm-saturation` /
  `direct-hydralisk-stress-ramp`
- tokens: `438` input + `24692` output = `25130`

Public projection proof for that row:

- before backfill: `416298311`
- after backfill: `416323441`
- exact delta: `25130`

Follow-up public reads after the continuation:

- `GET /api/public/khala-tokens-served` returned `416324542` at
  `2026-06-27T04:21:08.122Z`.
- `GET /api/public/khala-tokens-served/model-mix?window=all` returned
  `GLM family = 75874053` tokens and `totalTokens = 416324542` at
  `2026-06-27T04:21:33.257Z`.

Continuation totals:

- public-gateway ramp tokens generated and counted: `23372`
- public-gateway GLM tokens: `0`
- direct GLM tokens generated: `25130`
- direct GLM tokens retro-recorded into the public counter: `25130`
- current-session direct GLM total including the earlier `4298`: `29428`

Most important finding: the problem is no longer whether stress traffic can
increment `/stats` and `/khala`; it can. The active blocker is GLM routing and
replica health. Public tagged stress traffic still falls through to Fireworks,
and the fleet readiness projection has at least one false-ready 4-GPU replica
that returns fast `500`s under completions.

## Next Stress Step

For the next stress pass, fix the GLM lane before increasing public-gateway
concurrency:

1. Make tagged `internal_stress` / `glm-saturation` Khala requests select the
   GLM pool in production, not Fireworks, unless every GLM candidate fails.
2. Mark or route around `g4-4g-b-20260625154532` until a real completion probe
   succeeds; `/health` + `/v1/models` readiness is not enough.
3. Retry within the GLM pool across ready replicas before falling back to
   non-GLM providers.
4. Re-run the public counter proof and require
   `openagents.worker === hydralisk-vllm-glm-5p2-reap-504b`.
5. Require the public counter delta to be at least `usage.total_tokens`, then
   scale concurrency.

Do not count direct Hydralisk tokens toward #6317 acceptance unless a separate
server-side recorder path writes the same public-safe served-token ledger rows.
