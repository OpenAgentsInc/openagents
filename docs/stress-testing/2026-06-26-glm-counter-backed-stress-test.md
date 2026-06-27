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
capacity. The first follow-up code fix excluded degraded replicas from reserved
external headroom and deployed as Worker version
`6c81fc7a-5d3c-49af-93a4-1f6876bbed07`.

The post-deploy proof against that Worker still failed #6318. In three bounded
attempts, each pair started one authenticated `internal_stress` stream and then
sent one authenticated external request. All three external requests returned
HTTP `200`, and each pair moved the public token counter by `1972` tokens at
request closeout, but none of the external responses included
`scheduler_preemption` metadata and all three stress streams completed normally.
That proved the remaining bug was not degraded-replica headroom alone: the
preemption/in-flight registry was still per Worker isolate, so an external
request landing on a different isolate could not see or abort the active stress
stream.

The current follow-up adds a SQLite Durable Object scheduler
(`GLM_STRESS_SCHEDULER`) that stores short-lived `internal_stress` leases and
lets external requests preempt stress across isolates. The DO stores only
request refs, timestamps, bounded reasons, and scheduler evidence refs; it does
not store prompts, completions, provider payloads, or raw traces. Post-deploy
proof must rerun the same external-wins probe and require scheduler-preemption
metadata before #6318 can close.

First live DO deploy proof against Worker version
`4537f061-8e94-4579-850a-3c61dbf0126b` showed the DO binding was live and
metadata flowed: the external request returned HTTP `200`, carried
`scheduler_preemption` metadata, and moved the public token counter by `614`
tokens. It also exposed a precision bug in the first coordinator wiring: the
`internal_stress` request had already been rejected by route admission before
the external request arrived, so it should not have registered a global stress
lease. The follow-up patch prevents route-admission-rejected stress from
creating a DO lease.

Final follow-up deploy `ac5af10d-ee04-437b-bfd9-7f7c56354105` proved the
in-flight scheduler path rather than only admission-yield. Live readiness was
still `degraded` / `blocked` with `10` configured replicas, `1` ready replica,
`9` draining replicas, and `readyMaxInflight=2`. The authenticated probe
admitted the `internal_stress` stream as
`chatcmpl_9d1bce603c054ad28f65fed0f8711866`; the external request returned
HTTP `200`, carried `scheduler_preemption.evidence_ref =
scheduler.preemption.internal_stress.chatcmpl_9d1bce603c054ad28f65fed0f8711866`,
and reported `target_outcome: preempted_yielded`. The public counter moved from
`416491612` to `416493494` (`+1882`) only after request closeout. The external
response's own `usage.total_tokens` was `614`, so the remainder came from
concurrent/paired live traffic, including the admitted stress response. This is
real in-flight cross-isolate preemption evidence. It still does not fully close
#6318 because the external response served through Fireworks after
`fallback_reason: empty_assistant_content`; the issue's acceptance requires no
premature overflow to a weaker lane under saturation.

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

## 2026-06-27 GLM Pool Fix And Fleet Triage

A live replica probe after the Fireworks fallthrough showed that the configured
10-replica GLM roster was not actually a 10-replica serving fleet:

| Replica | Zone | `/health` | `/v1/models` | Completion | Finding |
| --- | --- | ---: | ---: | ---: | --- |
| `g4-4g-b-20260625154532` | `us-central1-b` | `200` | `200` | `500` | false-ready |
| `g4-4g-central1f-spot-20260625203000` | `us-central1-f` | `000` | `000` | `000` | unreachable |
| `g4-4g-east1b-spot-20260625203000` | `us-east1-b` | `000` | `000` | `000` | unreachable |
| `g4-4g-east1d-spot-20260625203000` | `us-east1-d` | `000` | `000` | `000` | unreachable |
| `g4-4g-east5a-spot-20260625203000` | `us-east5-a` | `000` | `000` | `000` | unreachable |
| `g4-4g-east5b-spot-20260625203000` | `us-east5-b` | `000` | `000` | `000` | unreachable |
| `g4-4g-east5c-spot-20260625211500` | `us-east5-c` | `000` | `000` | `000` | unreachable |
| `g4-4g-south1b-spot-20260625211500` | `us-south1-b` | `000` | `000` | `000` | unreachable |
| `g4-4g-west1a-spot-20260625203000` | `us-west1-a` | `000` | `000` | `000` | unreachable |
| `g4-8g-b-20260624214500` | `us-central1-b` | `200` | `200` | `200` | serving |

`gcloud` VM-level repair was blocked in this shell by expired non-interactive
Google auth, so the immediate gateway fix is route safety:

- The Hydralisk pool now retries inside the GLM replica pool before overflowing
  to non-GLM Khala fallbacks. A retryable selected-replica failure
  (`500`, `503`, `429`, transport) excludes that replica for the current
  request and selects the next eligible replica.
- Connect-time `streamSse` failures use the same in-pool retry path; mid-stream
  failures still cannot be replayed safely after bytes have been emitted.
- The nine non-serving replicas are marked `DRAINING` in Worker config while
  staying listed for readiness/accounting.
- The serving `g4-8g-b-20260624214500` replica is given a temporary
  gateway-admission budget of `maxInflight=2` so `internal_stress` is not
  rejected solely by the reserved-headroom guard while the rest of the fleet is
  drained.

Verification before deploy:

- `hydralisk-adapter.test.ts`: `32` passed, including new non-streaming and
  `streamSse` pool-failover regressions.
- `model-serving-policy.test.ts` + `glm-fleet-readiness.test.ts`: `55` passed.
- `chat-completions-routes.test.ts -t "GLM saturation|GLM"`: `13` passed.
- `model-router.test.ts`: `45` passed.
- API `typecheck`: exit `0`; it still prints pre-existing Effect lint messages
  in `src/trace-store-routes.ts`.

The fix was committed and pushed to `main` as
`6b0412b59a3a2b32ab2ccae886c2c06e2751944dc`, then deployed through
`bun run --cwd apps/openagents.com/workers/api deploy:safe` as Worker version
`6b38199d-2517-427c-9085-05802a6ed79a`.

Post-deploy readiness:

- `GET /v1/gateway/readiness`: ready, with one Hydralisk servable model.
- `GET /v1/gateway/glm-fleet/readiness`: `status=degraded`,
  `totalReplicaCount=10`, `readyReplicaCount=1`, `readyMaxInflight=2`,
  `drainingReplicaCount=9`, `disabledReplicaCount=9`.
- The only serving replica was `g4-8g-b-20260624214500`.
- The nine non-serving replicas stayed configured for accounting but were
  drained from route selection.

`gcloud` VM-level repair was still blocked in this shell by expired
non-interactive Google auth, so this pass made all configured gateway traffic
safe by draining bad replicas. It did not repair the unreachable/false-ready
VMs themselves.

## 2026-06-27 Post-Deploy Saturation

Public-gateway stress used `POST /v1/chat/completions`, model
`openagents/khala`, and the demand labels
`internal_stress` / `glm-saturation`. Exact usage came from response
`usage.total_tokens`; public counter deltas sometimes include concurrent live
traffic, so the hard per-run totals below use response usage.

| Run | Result | GLM tokens | Non-GLM tokens | Finding |
| --- | --- | ---: | ---: | --- |
| Smoke | Hydralisk GLM, finish `stop` | `711` | `0` | Counter moved by exact `+711`. |
| Two concurrent `2048` requests | One GLM, one Fireworks fallback | `2644` | `2648` | Two public requests already exceed the one healthy GLM lane. |
| Single public `8192` request | Fireworks fallback | `0` | `8803` | Public 8192 is not stable through the current GLM gateway. |
| Single public `4096` request | Hydralisk GLM, finish `length` | `4688` | `0` | Stable single-flight public GLM at 4096 completion tokens. |
| Five-request public `4096` loop | Two GLM, two Fireworks, one `429` | `9396` | `9402` | Back-to-back stress needs cooldown with only one live replica. |
| Tiny auth sweep | Two Hydralisk GLM one-token probes | `1121` | `0` | `codex-loopwright` and `raynor` creds reached inference; others lacked credits or auth. |
| Single public `4096` after auth sweep | Hydralisk GLM, finish `length` | `4740` | `0` | GLM route remained healthy when not contending with another long decode. |
| Paired public `4096` while direct load ran | Two Fireworks responses | `0` | `4862` | Under direct origin pressure, public gateway fell through to Fireworks. |
| Recovery public `4096` after direct load cleared | Hydralisk GLM, finish `length` | `4747` | `0` | Public route recovered back to GLM after pressure cleared. |

Post-deploy public-gateway exact GLM tokens generated: `28047`.

Known post-deploy public-gateway non-GLM fallback tokens generated during these
stress attempts: `25715`.

Direct Hydralisk stress against the one serving 8-GPU replica:

| Run | Requests | Result | Input | Output | Total |
| --- | ---: | --- | ---: | ---: | ---: |
| `direct-glm-8192-loop-20260627T045037Z` | `3` | three HTTP `200`, all full `8192` completion-token `length` stops | `201` | `24576` | `24777` |
| `direct-glm-paired-8192-20260627T050036Z` | `2` | two HTTP `200`, both early `stop` while public gateway pressure ran | `214` | `10032` | `10246` |

New exact direct GLM tokens generated in this pass: `35023`.

I inserted those two direct runs into production D1 as idempotent exact
served-token rows:

- `inference:served-tokens:retro.issue-6317.direct-glm.8192-loop-20260627T045037Z`
  = `24777` tokens.
- `inference:served-tokens:retro.issue-6317.direct-glm.paired-8192-20260627T050036Z`
  = `10246` tokens.

Verification:

- D1 returned both rows with `demand_kind=internal_stress`,
  `demand_source=glm-saturation`, and exact input/output splits.
- `/api/public/khala-tokens-served` returned `416602183` after those rows were
  inserted.
- The all-time model mix returned `GLM family = 75943187` tokens at that read.

Hard post-deploy GLM total from this continuation:

- public-gateway GLM: `28047`
- direct GLM, retro-recorded: `35023`
- total exact GLM generated: `63070`

What this taught us:

- The GLM-first route and in-pool failover fix works for single-flight public
  `openagents/khala` stress, and those requests increment the public counter
  used by `/stats` and `/khala`.
- The actual serving fleet is still one healthy 8-GPU replica, not ten working
  replicas. Route safety is fixed; VM capacity is not.
- Public `4096` completion-token requests are the useful current stress shape.
  Public `8192` requests and public concurrency tend to fall through to
  Fireworks or yield with only one live GLM replica.
- Direct single-flight 8192-token GLM generation is reliable on the 8-GPU
  replica, but it is slow and bypasses the gateway recorder unless exact usage
  is explicitly backfilled.
- Under concurrent direct-origin pressure, the gateway chose Fireworks for
  public stress and the direct GLM responses returned early. More repaired GLM
  replicas, or a stricter GLM-only stress admission mode, are needed before
  increasing public GLM concurrency without fallback.

## 2026-06-27 GCE Repair And 8-Replica Saturation

After non-interactive `gcloud` auth was restored, the fleet was repaired at the
VM/service level before the next stress pass:

- Recovered and verified serving for `8` replicas: seven 4-GPU G4 replicas plus
  `g4-8g-b-20260624214500` with `maxInflight=2`.
- Kept `g4-4g-central1f-spot-20260625203000` and
  `g4-4g-south1b-spot-20260625211500` drained because GCE returned
  `ZONE_RESOURCE_POOL_EXHAUSTED_WITH_DETAILS` on start attempts.
- Restored missing proxy/systemd wiring on the six restarted spot VMs and
  restarted the existing Hydralisk containers; the 4-GPU central1-b replica was
  reset from a false-ready/stopping state and came back serving.
- Deployed the undrained Worker config from `12cb5a75f2` as Worker version
  `ad237cc5-be2b-4677-8eae-e99d632cc92d`.

Live readiness after deploy:

- `readyReplicaCount=8`
- `readyMaxInflight=9`
- `drainingReplicaCount=2`
- `disabledReplicaCount=2`
- overall `status=degraded` only because the two stockout replicas remain
  configured but intentionally drained.

Counter-backed smoke after the repair:

| Probe | Worker | Selected replica | Exact GLM tokens | Counter path |
| --- | --- | --- | ---: | --- |
| non-streaming smoke | `hydralisk-vllm-glm-5p2-reap-504b` | `g4-4g-b-20260625154532` | `698` | D1 row + public counter |
| streaming smoke | `hydralisk-vllm-glm-5p2-reap-504b` | `g4-4g-b-20260625154532` | `694` | D1 row + public counter |

Main public-gateway saturation run:

- run id: `issue6317-8replica-saturation-20260627T053819Z`
- endpoint: `POST https://openagents.com/api/v1/chat/completions`
- model: `openagents/khala`
- labels: `internal_stress` / `glm-saturation`
- prompt class: public-safe synthetic marker text only
- local result artifact:
  `/tmp/issue6317-8replica-saturation-20260627T053819Z.json`

Stage results:

| Stage | Internal concurrency | External probes | GLM calls | GLM tokens | Non-GLM tokens | Failures | External failures | Observed GLM tok/s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `ramp-9x4096` | `9` | `6` | `29` | `137025` | `13394` | `0` | `0` | `349.38` |
| `overfill-12x4096` | `12` | `12` | `45` | `212625` | `43674` | `3` | `3` | `496.38` |
| `saturate-15x4096` | `15` | `12` | `44` | `207944` | `73988` | `0` | `0` | `484.34` |

Hard D1-scoped totals for this run:

| Demand/client scope | Provider | Rows | Tokens | Output tokens |
| --- | --- | ---: | ---: | ---: |
| main internal stress run | GLM / Hydralisk | `118` | `557594` | `483328` |
| main internal stress fallback | Fireworks DeepSeek | `22` | `103832` | `90112` |
| external probe traffic | Fireworks DeepSeek | `27` | `17784` | `1728` |

The hard public-gateway GLM total generated before the GLM-only routing fix was
`558986` tokens:

- `557594` from the main saturation run;
- `698` from the non-streaming post-repair smoke;
- `694` from the streaming post-repair smoke.

Latency and per-replica rollup for the main run:

- aggregate GLM observed throughput: `446.08` total tokens/sec over the run
  wall window.
- aggregate GLM completion goodput by request wall time: `59.85` output
  tokens/sec.
- TTFT: p50 `2573ms`, p90 `5195ms`, p99 `13131ms` across `118` GLM calls.
- SSE delta-gap ITL proxy: p50 `43ms`, p90 `71ms`, p99 `122ms`.
- All `8` ready replicas served traffic. Per-replica totals ranged from
  `66156` to `70881` tokens; per-replica output goodput clustered around
  `59`-`61` output tokens/sec.

What this taught us:

- The repaired 8-replica fleet can sustain useful public-gateway GLM pressure.
  The best observed aggregate GLM token/sec in this bounded run was at
  concurrency `12`, but that tier also produced the only external probe
  failures.
- The practical safe knee is still around the deployed active budget of `9`
  internal streams: it had zero failures and hit every ready replica.
- The issue #6317 external-wins acceptance is not green yet. During
  `overfill-12x4096`, the first external burst returned three HTTP `500`s, so
  measured external failure rate was not `0`.
- Several internal stress streams returned HTTP `200` with SSE frames but no
  terminal usage/openagents metadata. Those did not create served-token rows.
  Treat them as partial/yielded stress work, not counted output.
- Tagged `internal_stress` / `glm-saturation` still overflowed some internal
  requests to Fireworks on GLM rate limiting. That is bad stress hygiene:
  it moves the topline counter, but it does not stress GLM.

Follow-up fix from this run:

- `internal_stress` / `glm-saturation` Khala traffic is now pinned to the GLM
  adapter when the production lane plan contains the GLM adapter.
- Under GLM saturation, stress traffic should now return/yield on GLM
  saturation instead of overflowing to Fireworks.
- Tool-bearing Khala requests keep their existing GLM-first overflow behavior;
  only the explicit GLM saturation stress label is GLM-only.

Verification for that fix:

- `chat-completions-routes.test.ts -t "GLM saturation|external|internal_stress"`:
  `14` passed.
- Full `chat-completions-routes.test.ts`: `160` passed.
- `glm-fleet-readiness.test.ts`, `model-serving-policy.test.ts`, and
  `benchmark/stress-saturation-plan.test.ts`: `61` passed.

The fix was committed to `main` as `fce0033bdfae` and deployed as Worker
version `b7874868-4f72-4c30-9e7e-da9070f36b62`.

Post-deploy GLM-only verification:

- run id: `issue6317-glm-only-verify-20260627T060545Z`
- shape: `12` concurrent public-gateway `internal_stress` /
  `glm-saturation` requests, `max_tokens=1024`
- result: `8` GLM HTTP `200` responses, `4` HTTP `502 provider_error`
  responses, `0` Fireworks fallbacks
- exact D1 rows: `8`
- exact D1 GLM tokens: `6590`
- public counter delta during the verification: `+6590`

That post-deploy proof means the explicit GLM saturation stress label now fails
closed on GLM pressure instead of silently overflowing to Fireworks. The hard
public-gateway GLM total generated in this continuation, including the final
verification, is `565576` tokens.

## 2026-06-27 South1B Recovery And Counted Saturation Continuation

After local `gcloud` auth was repaired, a later continuation restored one more
live GLM serving instance before resuming #6317 stress:

- `g4-4g-south1b-spot-20260625211500` was started successfully in
  `us-south1-b`.
- The south1b Hydralisk container was restarted, the missing private-proxy
  systemd unit/run script was installed, and `/v1/models` returned HTTP `200`
  through the private proxy.
- `g4-4g-b-20260625154532` and
  `g4-4g-central1f-spot-20260625203000` could not be started or recovered
  because GCE returned `ZONE_RESOURCE_POOL_EXHAUSTED_WITH_DETAILS`; central1f
  remained draining/disabled and central1-b was later reported as reclaimed.
- The south1b drain flag was removed from Worker config in
  `6a832a4af1125460bd3c7fef16596dcb3503e91e`, and `deploy:safe` deployed
  Worker version `353c304a-4407-443b-956f-914a74691ba4`.

Focused verification before deploy:

- `bun run --cwd apps/openagents.com/workers/api test -- src/inference/glm-fleet-readiness.test.ts src/inference/model-serving-policy.test.ts src/inference/hydralisk-adapter.test.ts`
- result: `87` tests passed.

Live readiness before the stress pass:

- `readyReplicaCount=8`
- `readyMaxInflight=9`
- `disabledReplicaCount=1`
- `drainingReplicaCount=1`
- `unavailableReplicaCount=1`

Final readiness after the stress pass remained degraded but usable:

- `readyReplicaCount=8`
- `readyMaxInflight=9`
- `reclaimedReplicaCount=1` (`g4-4g-b-20260625154532`)
- `disabledReplicaCount=1` / `drainingReplicaCount=1`
  (`g4-4g-central1f-spot-20260625203000`)
- `g4-4g-south1b-spot-20260625211500` was ready and served traffic.

Counter/path smoke:

- run id: `issue6317-smoke-20260627T0903Z`
- endpoint: `POST https://openagents.com/api/v1/chat/completions`
- model: `openagents/khala`
- labels: `internal_stress` / `glm-saturation`
- exact D1 row: `1`
- exact D1 tokens: `854` (`598` input, `256` output)
- provider/model: `hydralisk-vllm-glm-5p2-reap-504b` /
  `openagents/glm-5.2-reap-504b`

The smoke confirmed that tagged GLM stress rows land in `token_usage_events` as
`usage_truth=exact`, `demand_kind=internal_stress`,
`demand_source=glm-saturation`, and the public counter path includes them.
`/khala` and `/stats` both returned HTTP `200`; both consume the same
live-at-read scalar endpoint, `GET /api/public/khala-tokens-served`.

Main counted stress runs from this continuation:

| Run id | Shape | D1 rows | Input tokens | Output tokens | Total exact GLM tokens |
| --- | --- | ---: | ---: | ---: | ---: |
| `issue6317-glm-saturation-20260627T090602Z` | ramp `9` -> `10`, brief overfill `11`, stopped after 11-way failure storm | `58` | `36830` | `233350` | `270180` |
| `issue6317-glm-knee10-20260627T091940Z` | fixed concurrency `10`, `4096` max completion tokens, 17.5m launch + drain | `111` | `68709` | `454656` | `523365` |
| `issue6317-smoke-20260627T0903Z` | single streaming smoke | `1` | `598` | `256` | `854` |

Hard D1-scoped total for this continuation:

| Provider | Model | Demand | Usage truth | Rows | Input | Output | Total |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| `hydralisk-vllm-glm-5p2-reap-504b` | `openagents/glm-5.2-reap-504b` | `internal_stress` / `glm-saturation` | `exact` | `170` | `106137` | `688262` | `794399` |

No rows for these run ids were served by Fireworks or any non-GLM provider.
That confirms the GLM-only saturation label now fails closed on GLM pressure
instead of silently overflowing counted stress to Fireworks.

Local run artifacts:

- `/tmp/issue6317-glm-saturation-20260627T090602Z.public.json`
- `/tmp/issue6317-glm-knee10-20260627T091940Z.public.json`

Public counter reads during this continuation:

- Before the second fixed-knee batch:
  `GET /api/public/khala-tokens-served` returned `452561702`.
- During a `/khala` and `/stats` confirmation read, both pages returned HTTP
  `200`, and the scalar endpoint returned `452864730`.
- Final scalar read after the run returned `462976825`.

The scalar moved by much more than this run's exact `794399` because other
Khala/Pylon/Codex traffic was concurrently inserting ledger rows. For stress
accounting, the hard source is therefore the run-scoped D1 query by
`demand_client`, not raw page delta.

Best sustained local slice:

- run id: `issue6317-glm-knee10-20260627T091940Z`
- fixed concurrency: `10`
- launched requests: `155`
- HTTP `200` streams: `113`
- HTTP `502` failures: `42`
- D1 exact rows: `111`
- exact GLM tokens: `523365`
- exact output tokens: `454656`
- receipt throughput: `460.2` total tokens/sec over run wall time
- output throughput: `399.79` output tokens/sec over run wall time
- TTFT: p50 `21669ms`, p90 `24064ms`, p99 `26209ms`
- request wall time: p50 `88604ms`, p90 `97408ms`, p99 `104363ms`

Per-replica token totals from the fixed-knee local artifact:

| Replica | Tokens |
| --- | ---: |
| `g4-4g-east1b-spot-20260625203000` | `70725` |
| `g4-4g-east1d-spot-20260625203000` | `70725` |
| `g4-4g-east5a-spot-20260625203000` | `70725` |
| `g4-4g-east5b-spot-20260625203000` | `66010` |
| `g4-4g-east5c-spot-20260625211500` | `66010` |
| `g4-4g-south1b-spot-20260625211500` | `61295` |
| `g4-4g-west1a-spot-20260625203000` | `61295` |
| `g4-8g-b-20260624214500` | `56580` |

What this taught us:

- The current counted public-gateway GLM stress ceiling is roughly
  `400`-`460` exact receipt tokens/sec with `8` ready replicas and
  `readyMaxInflight=9`.
- Concurrency `10` is the productive pressure point for this degraded fleet:
  it produces the best sustained token rate but still has a material HTTP `502`
  rate.
- Concurrency `11` is past the current knee. The first run's 11-way overfill
  phase rapidly drove failures from `23` to `49`, so continuing at that tier
  would have mostly measured rejected work.
- The 30-minute expectation of `5M` exact GLM tokens is not reachable with this
  fleet state. At the measured `460` total tokens/sec knee, 30 minutes is about
  `828k` theoretical tokens before failures; the actual exact D1 result was
  `794399`.
- The remaining reliability bug is not counter recording or Fireworks fallback:
  it is public-gateway `502` behavior under saturation. The stress path is now
  counted and GLM-only, but it still needs cleaner yield/backoff behavior at or
  above the concurrency knee.
