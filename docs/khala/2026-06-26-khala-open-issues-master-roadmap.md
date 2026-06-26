# Khala Open-Issues Master Roadmap — One Solve Sequence

> Status: **internal execution roadmap, 2026-06-26.** Direction-setting, not public
> claim copy and not a product promise. It orders the active
> `OpenAgentsInc/openagents` Khala issues plus their newly closed dependencies into a
> single dependency-aware sequence agents should work top-to-bottom. It flips no
> promise state and ships no code itself.

## How to use this

Work the phases in order. Within a phase, issues marked **‖ parallel** can run
concurrently; issues marked **→ after** must wait for their named dependency. Two GitHub
master trackers already exist and this roadmap threads through both:

- **#6316** — Maximize GLM-5.2-REAP usage in Khala (serving: Phases 0–3)
- **#6303** — Khala GTM push (demand: Phases 4–5)

The ordering principle: **stop the live breakage → make serving reliable → maximize
throughput → automate it → prove quality → drive adoption.** You cannot honestly grow
demand (Phase 4–5) on a coding surface that is hard-down (Phase 0), so adoption sequences
last even though it is the business goal.

The hard invariant throughout (#6318): **real external requests always win** — internal
stress/benchmark load is best-effort, preemptible, instantly-yielding, and tagged
`internal_stress`. It must still be counted in the public Khala token counter
when it is Khala-orchestrated; the tag exists for routing, trace filtering, and
external-wins admission, not for hiding usage.

## Current status snapshot

Refreshed from GitHub issue state, `origin/main`, live counter/proof reads, and
the local Pylon state on **2026-06-26 ~16:55Z**. This table is the
operator view of what remains, not a public product claim.

| Issue | State | Current status / next action |
| --- | --- | --- |
| #6310 | **Closed** | P0 OpenCode/tool-calling outage is no longer open. Keep its repro in regression coverage and do not let demand docs outrun the actual tool-call path. |
| #6323 | **Open** | Decision artifact for the full `nvidia/GLM-5.2-NVFP4` single-host pilot landed. The delegated Pylon/Codex retry (`assignment.public.khala_coding.chatcmpl_6a9906e622ad43caa1c9cc3fb2f20d00`, 4,371,243 exact tokens) has now been reviewed, tightened for public-safety, merged in `c7a86d7d06`, and deployed via `deploy:safe` as Worker `12066869-4f81-4b32-ba41-ba3c50b07595`. Still open: run the owner-armed isolated 8x-host pilot and record measured tool-call/quality/max-context/tok-s results. |
| #6319 | **Closed** | Reliability hardening/fallback-chain repair is closed. Treat empty responses and dead fallback lanes as regression risks in later serving work. |
| #6313 | **Closed** | Real OpenRouter fallback lane is closed. It is now a dependency assumption for further reliability and benchmark runs. |
| #6311 | **Open** | Partial readiness/watchdog projection work landed, and the Khala -> Pylon -> Codex diagnostic slices are now on `main`: `36ee76689c` (`assignment.public.khala_coding.chatcmpl_b5a0d831027a4c779b1105be73217f29`, 6,415,202 exact tokens) and `7f94c0556a` (`assignment.public.khala_coding.chatcmpl_c7358576d7464620ae3da33ed2f473a0`, 7,393,396 exact tokens). Current `main` including that cron-ledger slice is deployed as Worker `7460419e-2779-4e2e-a809-ec7646b6664a`, but production still reports zero `glm-pool-heartbeat` rows as of `2026-06-26T16:54Z`. Still open because the broad durable-fleet goal remains: prove live `glm-pool-heartbeat` rows, then non-Spot capacity, all-replica keep-warm/watchdog, auto-replace, reserve, and quota. |
| #6259 | **Closed** | Khala -> GLM served-worker disclosure + counter smoke is closed. |
| #6315 | **Closed** | Zero-debit receipt-ref fix for #6259 is closed. |
| #6320 | **Open** | A bounded routed slice landed in `85ca837413` and deployed as Worker `228ac0f9-c891-4ad2-b05f-0dd8894f3c86`: typed throughput-sweep metadata for `max-num-seqs`, prefix cache, chunked prefill, speculative decode, quant gates. Delegated slices landed on `main` in `a8c12aff42` (`assignment.public.khala_coding.chatcmpl_5ccffa5593b84cc09e414d3ad358b9b0`, 2,202,625 exact tokens) and `03b6ffa094` (`assignment.public.khala_coding.chatcmpl_7033a7cd0fff4afaaad412e783bab29a`, 4,949,572 exact tokens): typed throughput rollout recommendation/flag selection and owner-armed rollout artifact/guardrails. Current `main` is deployed as Worker `7460419e-2779-4e2e-a809-ec7646b6664a`; still open for actual live engine rollout and measured throughput lift. |
| #6318 | **Open** | Multiple partials landed (`a26ca1e`, `8ff2e47`, `4de477190c`) covering typed `internal_stress` attribution, route-level admission coverage, and live-headroom admission that rejects stress when reserved external headroom is unavailable. The current Pylon/Codex batch accepted a verified patch for external-wins preemption (`assignment.public.khala_coding.chatcmpl_a5425fa595d642d3831f1670ffd6bb49`, 4,244,593 exact tokens). Overseer review landed only the production-relevant steering slice in `2f467e3476`: direct agent-owned Pylon dispatch now survives transient OpenAuth link-read failure, while OpenAuth-only dispatch still fails closed with typed `coding_delegation_store_unavailable`. That commit is deployed as Worker `7460419e-2779-4e2e-a809-ec7646b6664a`. The delegated workspace's standalone preemption registry was not landed because it was not wired into the live runtime. Still open for real external-wins preemption proof. |
| #6317 | **Open** | Stress/saturation harness waits on #6318 and the live #6320 rollout. The current Pylon/Codex batch accepted a verified preparatory stress-harness patch (`assignment.public.khala_coding.chatcmpl_f06a11f5a625470bbc235a6d6ed952df`, 4,455,618 exact tokens), but that patch is still uncommitted in `workspace.pylon.codex_agent_task.2b0dec89bef6e319dbdf6aa8`; it remains pending overseer review/integration. |
| #6312 | **Open** | Decision-grade aggregate max tokens/sec benchmark waits on the stress harness. |
| #6321 | **Open** | Artanis fleet-overseer automation waits on the scheduler/stress/reliability pieces. |
| #6253 | **Open** | Isolated Terminal-Bench 2.0 black-box runner, bounded real measurement, and replication path landed in `da472748c5`. The separately owned full Harbor run must not be disturbed. Still open for decision-grade replicate-and-beat evidence. |
| #6307 | **Open** | Owner-armed real sweep harness and Khala-side run landed in `ff89ecf498`; spendful Fireworks/Vertex comparison remains owner-gated. Still open for the first `decisionGrade:true` full report. |
| #6308 | **Open** | Recurring external head-to-head publication layer landed in `2f2d011c64`. Still open for recurring decision-grade data from #6307-style owner-armed runs. |
| #6309 | **Open** | Gym ladder publication/projection layer landed in `1accb3573b`. Still open for decision-grade rung data and recurrence evidence. |
| #6305 | **Closed** | OpenCode -> Khala checklist/recipe is closed. Keep it honest if serving regresses. |
| #6306 | **Closed** | Next ecosystem recipes are closed. Keep them as docs/recipe artifacts, not proof that Phase 4 benchmarks are complete. |
| #6303 | **Open** | GTM umbrella remains open: recipe issues are closed and benchmark publication layers exist, but the real decision-grade benchmark/quality evidence and adoption scoreboard are not complete. |
| #6316 | **Open** | Serving umbrella remains open: #6320/#6318 have partial slices and fresh Pylon/Codex proofs, but #6323 pilot, #6311 live heartbeat/durability, #6318 live preemption, #6317 stress, #6312 aggregate benchmark, and #6321 overseer are not complete. |
| #6325 | **Closed** | Pylon/Codex delegated sessions are persisted as private traces and exact token events (`c92a5652ab`). |
| #6326 | **Closed** | Complete raw Codex SDK event streams persist privately for Pylon/Codex Khala delegation (`48e43cee02`, deploy `4d1de2d8-6285-41fa-bd9f-7a5a88cf8275`). |
| #6331 | **Closed** | The Pylon coding-delegation 500/unavailable path is fixed with typed diagnostics and proof surfaces. |

## Execution notes

- 2026-06-26: Supervising agents may briefly prioritize Khala -> Pylon -> Codex
  steering blockers ahead of the next phase item when the blocker prevents honest
  delegation, token attribution, or trace verification. This does not reorder the
  product backlog; it keeps the execution lane usable.
- Pylon/Codex steering is a usable lane as of `7057e61e0b`:
  `assignment run-no-spend` auto-selects a ready connected Codex account when no
  explicit account is provided, while still supporting `--account` /
  `--account-ref`. On 2026-06-26, `pylon accounts list --json` showed five ready
  Codex accounts (`codex`, `codex-2`, `codex-3`, `codex-4`, and default).
- Closed steering fixes that should stay assumed in future work: #6331 typed
  target-Pylon unavailable diagnostics, #6332 Codex reconnect/link refresh,
  #6333 one-shot `presence heartbeat --json`, #6334 runtime lifecycle streaming,
  #6335 verifier sanitizer false-positive fix, #6336 capacity-gate link repair,
  #6339 pinned checkout/verifier defaults, #6340 live runtime progress, #6341
  `provider go-online` pylonRef/heartbeat proof, #6349 no silent fixture
  fallback, and #6350 proof readout without direct D1 queries.
- #6331 is closed, but its invariant stays live: a targeted linked Pylon whose
  assignment dispatch gate is full must return a typed, diagnosable
  `target_pylon_unavailable` response with gate evidence, not fall through to a
  generic unavailable/500 path.
- #6325/#6326 are closed. Pylon/Codex delegated turns must still be verified by
  exact `token_usage_events` rows, private `agent_traces`, and
  `pylon_codex_raw_events` rows. Verification has a first-class owner-scoped read
  path (`GET /api/pylon/codex/proof?assignmentRef=...` and
  `pylon khala proof --assignment-ref <assignmentRef> --json`). Counter movement
  alone is never proof because other agents may be running.
- Pylon state at refresh: `provider go-online --json` reported
  `pylon.33afd48282a649047e3a`, lifecycle `online`, Codex ready, and
  `ownCapacityDispatch.maxCodexAssignments: 5` /
  `availableCodexAssignments: 5`. After the latest batch, busy was reset to
  `0` and available Codex capacity was again `5`. `pylon accounts list --json`
  showed ready present accounts for `codex`, `codex-2`, `codex-3`, `codex-4`,
  the default Codex homes, `claude-pylon-2`, and `claude-pylon-3`, plus two
  stale/missing Codex registry refs. The practical result: multi-account Codex
  dispatch can work, but the remaining steering work must keep proving the
  assignment gate uses the coding-capacity projection, not the legacy one-flight
  policy field.
- Prior paused delegation: assignment
  `assignment.public.khala_coding.chatcmpl_ffe4aef49ef94614be78bc9c8c7b3b62`
  completed locally on `codex-3` against `91edb870c3` with accepted closeout
  `assignment.closeout.de5c448aa8a73c1639aaff89`. The patch was reviewed, tested,
  merged as `856fc636d023f821480480f11654988ade65e9ca`, pushed to `main`, and
  deployed through `deploy:safe` as Worker version
  `785a7379-10ca-4b1c-9e86-bc734b11e2ec`. Verification was
  `typecheck:web`, `typecheck:api`, focused GLM heartbeat/readiness tests, and
  `check:deploy`, followed by live `/` and `/khala` 200 smokes.
- Owner-scoped proof for that assignment is now good when called with the
  explicit flag form:
  `pylon khala proof --assignment-ref assignment.public.khala_coding.chatcmpl_ffe4aef49ef94614be78bc9c8c7b3b62 --json`.
  It reports one exact token row (`total_tokens: 869982`,
  `provider: pylon-codex-own-capacity`,
  `demand_source: khala_coding_delegation`), one owner-only ATIF trace, and one
  owner-only raw-event row with 71 Codex SDK events / 212,420 bytes.
- Latest multi-account Pylon/Codex batch before pause: four accepted assignments
  completed across `codex-2`, `codex-3`, and `codex-4` with exact owner-capacity
  token rows, owner-only ATIF traces, and owner-only raw Codex event rows:
  `assignment.public.khala_coding.chatcmpl_6a9906e622ad43caa1c9cc3fb2f20d00`
  (#6323 retry: 4,371,243 tokens, 150 raw events / 2,578,167 bytes);
  `assignment.public.khala_coding.chatcmpl_b5a0d831027a4c779b1105be73217f29`
  (#6311: 6,415,202 tokens, 213 raw events / 1,159,118 bytes);
  `assignment.public.khala_coding.chatcmpl_5ccffa5593b84cc09e414d3ad358b9b0`
  (#6320: 2,202,625 tokens, 111 raw events / 1,609,106 bytes); and
  `assignment.public.khala_coding.chatcmpl_29c527d6cc154a52b05279a36fb93e34`
  (Pylon proof/steering: 4,496,928 tokens, 172 raw events / 915,208 bytes).
  Exact accepted-batch total: **17,485,998 Khala-attributed tokens**. A first
  #6323 attempt on `codex` was rejected with
  `blocker.assignment.codex_agent_execution_refused`; use `codex-2+` for the
  next stress batch until `codex` is revalidated.
- Mainline changes from that batch: `a8c12aff42` (#6320 rollout selector),
  `7bf68b4652` (Pylon own-capacity dispatch proof readout),
  `36ee76689c` (#6311 bounded heartbeat probes), plus concurrent mainline
  commits `efaa53424b` and `96c9b91599`. The #6323 retry originally left its
  generated pilot harness dirty in the delegated workspace; that patch has now
  been reviewed, tightened to keep API keys env-only and URL-like refs
  fail-closed, merged as `c7a86d7d06`, commented on #6323, and deployed through
  `deploy:safe` as Worker version `12066869-4f81-4b32-ba41-ba3c50b07595`.
- Latest post-refresh stress batch, launched from clean `origin/main` at
  `e048048a2a` and verified at `2026-06-26T16:42Z`, accepted four useful
  Pylon/Codex assignments and rejected one unhealthy plain-`codex` assignment:
  - #6311: `assignment.public.khala_coding.chatcmpl_c7358576d7464620ae3da33ed2f473a0`
    on `codex-2`, merged/pushed as `7f94c0556a`, exact tokens
    `7,393,396`, trace `da4439ad-06be-45e0-9210-f0d70ee74035`, raw events
    `201` / `2,952,802` bytes
    (`raw.pylon_codex.f2f41231823f31265798c5ebfb29b6a9`).
  - #6320: `assignment.public.khala_coding.chatcmpl_7033a7cd0fff4afaaad412e783bab29a`
    on `codex-4`, merged/pushed as `03b6ffa094`, exact tokens
    `4,949,572`, trace `6276803a-0a42-44a5-9e4c-8682be1cfb64`, raw events
    `165` / `4,646,052` bytes
    (`raw.pylon_codex.2e4f044c933398aa783aa0b4823edb7e`).
  - #6318: `assignment.public.khala_coding.chatcmpl_a5425fa595d642d3831f1670ffd6bb49`
    on `codex-3`, accepted with exact tokens `4,244,593`, trace
    `f0061431-d335-4cbb-b9a2-b09997001f1e`, raw events `176` /
    `3,839,138` bytes
    (`raw.pylon_codex.e4b6da057344f9fe9f11e226642762cb`). Overseer review
    merged only the live steering-safe part as `2f467e3476`; the workspace's
    standalone registry/harness files were not landed because they were not
    wired into production routing.
  - #6317: `assignment.public.khala_coding.chatcmpl_f06a11f5a625470bbc235a6d6ed952df`
    on `codex-2` concurrently with #6311, accepted with exact tokens
    `4,455,618`, trace `ab31c0b5-eff2-4cf3-b627-25db01b4f163`, raw events
    `149` / `1,906,799` bytes
    (`raw.pylon_codex.55dcd51577b7e5afadba8664d7c29b70`), but still
    uncommitted in
    `/Users/christopherdavid/.openagents/pylon/cache/codex-agent-tasks/workspace.pylon.codex_agent_task.2b0dec89bef6e319dbdf6aa8`.
  - #6323 plain-`codex` retry
    `assignment.public.khala_coding.chatcmpl_6ea9caf0863f43488dbba5b2aaa30481`
    was rejected with `blocker.assignment.codex_agent_execution_refused`.
  This batch proves at least three connected Codex accounts can run, and one
  account (`codex-2`) can run two assignments in parallel. Treat the plain
  `codex` account as unhealthy until revalidated.
- Public counter state at refresh:
  `/api/public/khala-tokens-served` returned `237,288,370` at
  `2026-06-26T16:53:23.096Z`. In the corrected 45-minute window,
  `token_usage_events` had six `khala_coding_delegation` rows totaling
  `25,461,387` tokens, with the latest at `2026-06-26T16:42:00.830Z`; after
  that, the small visible ticks were heartbeat/canary rows. The current Pylon/
  Codex behavior is closeout-based: the public counter updates when each Codex
  turn posts its exact `token_usage_events` row, not continuously for every
  streamed SDK event while the turn is still running. The exact Pylon/Codex
  attribution must still come from
  `pylon khala proof --assignment-ref ... --json` or token rows filtered to
  `provider='pylon-codex-own-capacity'`, because public counter movement is
  aggregate and other agents may be running.
- Current serving observability gap: the #6311 GLM readiness route can project
  readiness from persisted routed-completion fallback rows, but canonical
  scheduled `glm-pool-heartbeat` rows have still not been observed after arming.
  The deployed `856fc636d0` patch covers skipped/disabled/unarmed diagnostics,
  the `36ee76689c` patch bounds hung replica probes, and the `7f94c0556a`
  patch wires cron ledger proof emission. Current `main` including those slices
  is deployed as Worker `7460419e-2779-4e2e-a809-ec7646b6664a`, but live row
  proof is still absent
  (`SELECT COUNT(*) FROM token_usage_events WHERE demand_source='glm-pool-heartbeat'`
  returned zero rows at `2026-06-26T16:54Z`). Do not rely on scheduled
  watchdog evidence for all replicas until the latest main is deployed and live
  rows appear and are inspected.
- Deployment/live smoke after `2f467e3476`: `deploy:safe` verified local
  `main` matched `origin/main`, ran `check:deploy`, applied zero pending D1
  migrations, verified pending migrations were zero, built web assets, and
  uploaded Worker version `7460419e-2779-4e2e-a809-ec7646b6664a`. Live
  `https://openagents.com/`, `https://openagents.com/khala`, and
  `/assets/index-DZ-c2BZu.js` returned HTTP 200.
- Deployment/live smoke after `c7a86d7d06`: `deploy:safe` applied zero pending
  D1 migrations, verified pending migrations were zero, built web assets, and
  uploaded Worker version `12066869-4f81-4b32-ba41-ba3c50b07595`. Live
  `https://openagents.com/`, `https://openagents.com/khala`, and
  `/assets/index-DZ-c2BZu.js` returned HTTP 200.

---

## Phase 0 — STOP THE BLEEDING (P0; the OpenCode wedge is hard-down NOW)

Real external users (via OpenCode) currently get ~100% `provider_error` on tool calls.
This is the only phase that is an active outage. Do it first.

1. **#6310 — GLM tool-calling broken (P0).** Tool requests to the primary GLM lane return
   `provider_error` ~100% of the time; every OpenCode coding request fails. Two acceptable
   resolutions (do the faster one immediately, then the durable one):
   - **Immediate mitigation:** route tool-bearing / coding requests OFF GLM to a working
     tool-caller (DeepSeek-V4 / a healthy GPT-OSS-120B / frontier), keep plain chat on GLM.
   - **Durable fix:** correct the GLM-5.2-REAP vLLM tool path — the `--tool-call-parser`
     value ↔ `--reasoning-parser` interaction ↔ chat template for this checkpoint (the
     parser is set but errors on tool requests).
   - **Done when:** a scripted OpenCode-style tool loop round-trips real `tool_calls` with
     **0 `provider_error`** over N consecutive requests.
   - **Status (2026-06-26): CLOSED.** Treat as a regression gate, not the current
     active work item.
2. **#6319 (fallback-chain repair slice) ‖ parallel with #6310.** The fallback chain is
   itself broken — **GPT-OSS-120B (fallback #2) returns 404**, GPT-OSS-20B (#3) returns
   empty — so GLM overflow degrades two dead hops before a serving lane. At minimum, in
   Phase 0: repair/replace the dead lanes + treat empty content as a failure so a 200 is
   never an empty/no-tool response. (Full #6319 program continues in Phase 1.)
   - **Status (2026-06-26): CLOSED.** The full reliability issue is also closed;
     downstream work should preserve these checks as serving regressions.
3. **#6323 — pilot `nvidia/GLM-5.2-NVFP4` (full 753B) on the 8× host ‖ parallel with #6310,
   as a candidate FIX for it.** Our REAP-504B already uses the canonical `glm47`/`glm45`
   parsers, so #6310 is the pruned checkpoint, not config. NVIDIA's full 753B NVFP4
   (near-FP8, agentic-tool-use-validated, MIT) fits our one `g4-standard-384` 8× RTX PRO 6000
   host (TP-8, ~381 GB weights in 768 GB). Deploy it there and test: does it tool-call clean
   where REAP `provider_error`s? If yes, it's both the #6310 fix and the quality upgrade —
   route the GLM coding lane to it, keep REAP-504B on the 4× hosts. (Eval:
   `docs/inference/2026-06-26-nvidia-glm-5.2-nvfp4-evaluation.md`. Scaling the full model
   beyond one host depends on 8× Blackwell quota/capacity — #6311.)
   - **Decision artifact landed (2026-06-26): GO for a bounded single-host pilot.** Feasible
     today on the one `g4-standard-384` 8× host (NVFP4-capable Blackwell, ~381 GB weights +
     unquantized shared expert + KV in 768 GB at TP-8); **not** feasible on the 4× hosts. The
     full model is a **credible #6310 fix** (same `glm47`/`glm45` parsers as REAP → fault is
     the pruned checkpoint, not config; full model is agentic-validated). Pilot = isolated
     endpoint on the 8× host with the card's exact flags + a measured `--max-model-len` (96 GB
     cards, not B200/B300 — prove the KV ceiling), primary test = OpenCode tool loop +
     #6310 repro with **0 `provider_error`**, then quality + tok/s vs REAP's ~47 tok/s.
     **Rollback is trivial** (separate endpoint; live `openagents/khala` stays on REAP the
     whole pilot). Owner / serving-lane executes the run; this lane stayed **doc/decision-only**
     (no live fleet/gateway/Pylon changes). Full plan + success criteria + conditional routing
     precedence: the "Decision artifact (#6323)" section of the eval doc.
   - **Status (2026-06-26): OPEN.** The delegated Pylon/Codex retry produced an
     executable owner-armed pilot harness, now merged in `c7a86d7d06` and
     deployed. Next action is the actual isolated 8x-host pilot run with owner
     endpoint/approval refs, then a measured tool-call/quality/max-context/
     throughput decision, not another planning doc.

## Phase 1 — Reliable serving foundation

Make the fleet trustworthy before pushing load through it.

3. **#6319 — reliability hardening program (full).** Per-replica health + circuit-breaker,
   empty-fallback-as-failure, SLO-based shedding, request hedging, and failure telemetry
   (provider_error / empty / fallback / invalid-tool rates) so the next breakage is visible
   without a user mailing screenshots. → continues from Phase 0 slice.
   - **Status (2026-06-26): CLOSED.**
4. **#6313 — real OpenRouter fallback lane ‖ parallel.** No OpenRouter inference lane
   exists today (only resale/identity refs + a key). Build the adapter + registration +
   plan entry, wired as a real fallback tier with fail-over tests. Gives the chain a
   working terminal hop.
   - **Status (2026-06-26): CLOSED.**
5. **#6311 — durable (non-Spot) GLM fleet + keep-warm ‖ parallel.** All 10 replicas are
   Spot; 8 lack the STOP-watchdog. Add keep-warm on every replica, multi-region
   auto-replace, an on-demand reserve, and the us-central1 quota increase. (Cross-refs
   hydralisk #95 durable host, #99 prebake-weights image.)
   - **Status (2026-06-26): OPEN.** Partial route/projection work has landed, but
     the issue remains broad. The Pylon/Codex diagnostic slice for canonical
     scheduled-skip GLM pool heartbeat rows landed in `856fc636d0` and deployed
     safely, and the newer bounded-probe slice landed in `36ee76689c` and was
     included in the `12066869-4f81-4b32-ba41-ba3c50b07595` deploy. The latest
     cron-ledger-proof slice is merged in `7f94c0556a` but not yet deployed by
     the overseer. Live `glm-pool-heartbeat` rows have not yet appeared.
     Current next actions: deploy safely, prove live scheduled rows, then continue the real
     durability/non-Spot/reserve/quota scope or explicitly split it.
6. **#6259 + #6315 — green end-to-end GLM-serving smoke. → after #6310.** Get the
   Khala→GLM verification smoke passing for real (served-worker disclosure + counter
   increment); #6315 is the receipt-ref fix for the zero-debit operator-exempt token.
   This is the regression gate the rest of the work leans on.
   - **Status (2026-06-26): CLOSED.**

## Phase 2 — Maximize throughput (tokens/sec)

7. **#6320 — inference-engineering throughput optimizations. → after #6319.** THE lever:
   replicas run single-flight, so continuous batching is OFF — raise `--max-num-seqs` to
   unlock it (a multiple, not a percent), then stack chunked prefill + engine-side prefix
   caching + speculative/MTP decode + eval-gated quantization. Biggest tok/s win in the
   whole roadmap; do it before stress-testing so you measure the real ceiling.
   - **Status (2026-06-26): OPEN.** Bounded sweep metadata landed in
     `85ca837413`, and the delegated rollout-recommendation/flag-selector slice
     landed in `a8c12aff42`. A newer owner-armed rollout artifact landed in
     `03b6ffa094`, but it has not yet had an overseer `deploy:safe`, and live
     engine flags plus measured throughput lift have not happened. Keep it before
     #6317/#6312.
8. **#6318 — external-wins admission/priority scheduler. → before #6317.** Internal load
   must be preemptible and yield to external demand. This MUST land before any continuous
   stress so the stress harness can never starve a real user.
   - **Status (2026-06-26): OPEN.** Admission/attribution slices landed and were
     deployed. A verified Pylon/Codex patch for external-wins preemption exists
     but is still uncommitted in
     `workspace.pylon.codex_agent_task.1eec845eb03329342e73630b`; the issue
     remains the hard gate before stress load until that patch is reviewed,
     merged, deployed, and live mid-flight preemption is proven.
9. **#6317 — continuous max-capacity stress/saturation harness. → after #6318, #6320.**
   The self-driving load that saturates the fleet, ramps concurrency to the ceiling, and
   auto-backs-off on external pressure.
   - **Status (2026-06-26): OPEN; blocked by #6318 and live #6320 rollout.**
     A verified preparatory Pylon/Codex patch exists but is still uncommitted in
     `workspace.pylon.codex_agent_task.2b0dec89bef6e319dbdf6aa8`; integrate it
     only after the external-wins guard is real.
10. **#6312 — max tokens-per-second benchmark. → after #6317.** The decision-grade
    aggregate-throughput number, read from the harness (concurrency sweep, per-replica +
    aggregate tok/s, TTFT, P50/P90/P99, saturation point, in-cloud vs WAN).
    - **Status (2026-06-26): OPEN; blocked by #6317.**

## Phase 3 — Autonomous operation

11. **#6321 — Artanis fleet-overseer automation. → after #6317, #6318, #6319.** The
    autonomous control loop (on `artanis-administrator-tick`, approval-gated): watches fleet
    health + throughput + external demand, orchestrates the stress load (start/scale/
    back-off keyed on external pressure), and triggers heal/scale/quarantine — money +
    destructive actions stay owner-gated via `artanis-approval-gates`. This is the layer
    that runs Phases 1–2 continuously without a human.
    - **Status (2026-06-26): OPEN; blocked by #6318/#6317 and should incorporate
      the already-closed #6319 reliability signals.**

## Phase 4 — Prove quality (now that serving is reliable + instrumented)

12. **#6253 — replicate + beat GLM-REAP's 69.1% on Terminal-Bench 2.0. → after Phase 1.**
    The competitive goal: a decision-grade Khala-routed run (not the raw-GLM pilot),
    inference-method comparison, beat the baseline.
    - **Status (2026-06-26): OPEN.** Black-box runner and bounded public-safe
      measurement path landed in `da472748c5`. A separate agent may own a live
      Harbor run; do not interrupt it. Remaining work is decision-grade
      replicate-and-beat evidence.
13. **#6307 — owner-armed real sweep: first `decisionGrade:true` Khala-vs-Fireworks/Vertex
    report ‖ parallel.** The minimum decision suite, run for real over realistic traffic.
    - **Status (2026-06-26): OPEN.** Harness/seam and Khala-side run landed in
      `ff89ecf498`; spendful external lanes remain owner-gated.
14. **#6308 — external head-to-head (recurring quality bar). → after #6307.** Khala vs the
    tools/models developers would otherwise use, on our axes (cost-per-accepted-outcome,
    verified-rate).
    - **Status (2026-06-26): OPEN.** Publication layer landed in `2f2d011c64`;
      decision-grade recurring data still depends on #6307.
15. **#6309 — gym benchmark ladder as a recurring leaderboard. → after #6307.** Big Pickle
    → free models → paid frontier, published and re-scored on every change.
    - **Status (2026-06-26): OPEN.** Ladder publication/projection layer landed
      in `1accb3573b`; decision-grade rung data still depends on #6307.

## Phase 5 — Drive adoption (the demand side; GTM #6303)

16. **#6305 — OpenCode → Khala verification checklist + publish. → HARD-after #6310.** Do
    NOT publish the OpenCode recipe until tool-calling actually works; publishing a broken
    coding agent burns the wedge. This is the first external "point your tool at us" win.
    - **Status (2026-06-26): CLOSED.**
17. **#6306 — next ecosystem recipes (Aider, Cline/Continue, Vercel AI SDK, LiteLLM,
    LangChain). → after #6305.** One tool at a time, each with its test checklist.
    - **Status (2026-06-26): CLOSED.**
18. **#6303 — GTM push tracking (umbrella).** Closes when 16–17 + the Phase-4 benchmarks
    land; keep it updated as the demand-side scoreboard.
    - **Status (2026-06-26): OPEN.** Recipe work and publication layers are
      present, but benchmark evidence and adoption scoreboard evidence are not
      complete.

---

## The single sequence (flat list)

Historical full sequence:

`#6310` [closed] ‖ `#6323`(full-model candidate fix) →
`#6319(chain-repair)` [closed] → `#6319(full)` [closed] ‖ `#6313` [closed] ‖
`#6311` → `#6259/#6315` [closed] → `#6320` → `#6318` → `#6317` → `#6312` →
`#6321` → `#6253` ‖ `#6307` → `#6308` ‖ `#6309` → `#6305` [closed] →
`#6306` [closed] → close `#6303`.

Remaining active sequence after the 2026-06-26 ~16:55Z refresh:

`#6323`(run the owner-armed full-model pilot with the landed harness) ‖
`#6311`(prove live heartbeat rows, then durability/non-Spot/reserve/quota) →
`#6320`(live engine rollout + measured lift) →
`#6318`(finish real external-wins preemption proof; do not count the unwired delegated registry as live) →
`#6317`(review/integrate accepted stress-prep patch after #6318) → `#6312` → `#6321` →
`#6253`(decision-grade replicate/beat) ‖ `#6307`(owner-armed full comparison) →
`#6308` ‖ `#6309`(recurring evidence) → close `#6316` / `#6303`.

(#6323 remains at the front because it could still become the quality/tool-call
upgrade path even though #6310 itself is closed. If the full model tool-calls
cleanly and beats REAP quality/throughput expectations, route it as the premium
GLM coding lane and leave REAP-504B on the 4x hosts.)

## Dependency rationale (the non-obvious edges)

- **#6318 before #6317** — never run a saturation load without the external-yield guard, or
  you DoS your own paying users.
- **#6320 before #6312/#6317** — measuring throughput before turning on continuous batching
  measures the wrong (artificially low) ceiling.
- **#6310 before #6305** — the OpenCode publish is gated on a working tool-caller.
- **Phase 1 before Phase 4** — benchmark numbers are only decision-grade over reliable,
  realistic serving; benchmarking a flaky lane produces noise, not receipts.
- **#6321 last in the serving track** — automation should orchestrate systems that already
  work (the harness, the scheduler, the hardening), not paper over their absence.

## Notes

- Master trackers: **#6316** (serving, Phases 0–3) and **#6303** (demand, Phases 4–5).
- Cross-repo: hydralisk **#95** (durable host) and **#99** (prebake-weights image) back
  #6311; coordinate the fleet-side work there.
- The owner-gated honesty bar still applies: any published benchmark number must come from
  the owner-armed real seam over realistic traffic (`decisionGrade:true`); internal
  dogfood/stress tokens stay segmented (#6298 demand tags) and out of external metrics.
- Khala -> Pylon -> Codex worker status at refresh: delegation was paused by
  operator request after the current batch. Accepted work completed across
  multiple Codex accounts with exact owner-capacity token rows, traces, and
  raw-event refs, and `codex-2` proved same-account parallelism by running two
  assignments concurrently. Future launches should record assignment refs
  immediately, verify `pylon khala proof --assignment-ref ... --json`, and
  compare exact token rows instead of relying on public counter movement. The
  next steering gaps to settle are (1) why the plain `codex` account refused one
  assignment while `codex-2+` succeeded, and (2) whether the public counter
  should increment continuously from streamed SDK events or remain
  closeout-based over exact turn usage rows.
