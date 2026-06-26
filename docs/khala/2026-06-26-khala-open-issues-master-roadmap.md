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
`internal_stress` (excluded from the public counter).

## Current status snapshot

Refreshed from GitHub issue state, `origin/main`, and the local Pylon state on
**2026-06-26 ~15:35Z** while live delegation was paused. This table is the
operator view of what remains, not a public product claim.

| Issue | State | Current status / next action |
| --- | --- | --- |
| #6310 | **Closed** | P0 OpenCode/tool-calling outage is no longer open. Keep its repro in regression coverage and do not let demand docs outrun the actual tool-call path. |
| #6323 | **Open** | Decision artifact for the full `nvidia/GLM-5.2-NVFP4` single-host pilot landed; the remaining work is the owner/fleet execution of the isolated 8x-host pilot and measured tool-call/quality/tok-s result. |
| #6319 | **Closed** | Reliability hardening/fallback-chain repair is closed. Treat empty responses and dead fallback lanes as regression risks in later serving work. |
| #6313 | **Closed** | Real OpenRouter fallback lane is closed. It is now a dependency assumption for further reliability and benchmark runs. |
| #6311 | **Open** | Partial readiness/watchdog projection work landed, but the broad durable-fleet goal remains open: non-Spot capacity, all-replica keep-warm/watchdog, auto-replace, reserve, quota. A Khala -> Pylon -> Codex assignment completed a candidate canonical scheduled-skip diagnostic patch locally (3 files, not yet reviewed/merged); do not treat it as landed. |
| #6259 | **Closed** | Khala -> GLM served-worker disclosure + counter smoke is closed. |
| #6315 | **Closed** | Zero-debit receipt-ref fix for #6259 is closed. |
| #6320 | **Open** | A bounded routed slice landed in `85ca837413` and deployed as Worker `228ac0f9-c891-4ad2-b05f-0dd8894f3c86`: typed throughput-sweep metadata for `max-num-seqs`, prefix cache, chunked prefill, speculative decode, quant gates. Still open for actual live engine rollout and measured throughput lift. |
| #6318 | **Open** | Multiple partials landed (`a26ca1e`, `8ff2e47`, `4de477190c`) covering typed `internal_stress` attribution, route-level admission coverage, and live-headroom admission that rejects stress when reserved external headroom is unavailable. Still open for live mid-flight preemption proof under the continuous stress harness. |
| #6317 | **Open** | Stress/saturation harness waits on #6318 and ideally #6320. |
| #6312 | **Open** | Decision-grade aggregate max tokens/sec benchmark waits on the stress harness. |
| #6321 | **Open** | Artanis fleet-overseer automation waits on the scheduler/stress/reliability pieces. |
| #6253 | **Open** | Isolated Terminal-Bench 2.0 black-box runner, bounded real measurement, and replication path landed in `da472748c5`. The separately owned full Harbor run must not be disturbed. Still open for decision-grade replicate-and-beat evidence. |
| #6307 | **Open** | Owner-armed real sweep harness and Khala-side run landed in `ff89ecf498`; spendful Fireworks/Vertex comparison remains owner-gated. Still open for the first `decisionGrade:true` full report. |
| #6308 | **Open** | Recurring external head-to-head publication layer landed in `2f2d011c64`. Still open for recurring decision-grade data from #6307-style owner-armed runs. |
| #6309 | **Open** | Gym ladder publication/projection layer landed in `1accb3573b`. Still open for decision-grade rung data and recurrence evidence. |
| #6305 | **Closed** | OpenCode -> Khala checklist/recipe is closed. Keep it honest if serving regresses. |
| #6306 | **Closed** | Next ecosystem recipes are closed. Keep them as docs/recipe artifacts, not proof that Phase 4 benchmarks are complete. |
| #6303 | **Open** | GTM umbrella remains open: recipe issues are closed and benchmark publication layers exist, but the real decision-grade benchmark/quality evidence and adoption scoreboard are not complete. |
| #6316 | **Open** | Serving umbrella remains open: #6320/#6318 have partial deployed slices, but #6323 pilot, #6311 durability, #6317 stress, #6312 aggregate benchmark, and #6321 overseer are not complete. |
| #6325 | **Closed** | Pylon/Codex delegated sessions are persisted as private traces and exact token events. |
| #6326 | **Closed** | Complete raw Codex SDK event streams persist privately for Pylon/Codex Khala delegation. |
| #6331 | **Closed** | The Pylon coding-delegation 500/unavailable path is fixed with typed diagnostics and proof surfaces. |

## Execution notes

- 2026-06-26: Supervising agents may briefly prioritize Khala -> Pylon -> Codex
  steering blockers ahead of the next phase item when the blocker prevents honest
  delegation, token attribution, or trace verification. This does not reorder the
  product backlog; it keeps the execution lane usable.
- Pylon/Codex steering is now a usable lane as of `7057e61e0b`:
  `assignment run-no-spend` auto-selects a ready connected Codex account when no
  explicit account is provided, while still supporting `--account` /
  `--account-ref`. On 2026-06-26, `pylon accounts list --json` showed five ready
  Codex accounts (`codex`, `codex-2`, `codex-3`, `codex-4`, and default).
- #6331 is closed, but its invariant stays live: a targeted linked Pylon whose
  assignment dispatch gate is full must return a typed, diagnosable
  `target_pylon_unavailable` response with gate evidence, not fall through to a
  generic unavailable/500 path.
- #6325/#6326 are closed. Pylon/Codex delegated turns must still be verified by
  exact `token_usage_events` rows, private `agent_traces`, and
  `pylon_codex_raw_events` rows. Verification has a first-class owner-scoped read
  path (`GET /api/pylon/codex/proof?assignmentRef=...` and
  `pylon khala proof <assignmentRef> --json`). Counter movement alone is never
  proof because other agents may be running.
- Pylon state at refresh: `presence heartbeat --json` reported
  `pylon.33afd48282a649047e3a`, `registered: true`, `linked: true`,
  `stale: false`, heartbeat sequence `127`, and no blocker refs at
  `2026-06-26T15:33:52.321Z`. Local account inventory showed five ready Codex
  homes (`codex`, `codex-2`, `codex-3`, `codex-4`, and the default home) plus
  two stale/missing Codex registry refs. `provider go-online` still reported
  `maxInflight: 1` / `perBuyerMaxInflight: 1`, so parallel stress must first
  publish a higher Codex concurrency through the heartbeat/capacity path.
- Latest paused delegation: assignment
  `assignment.public.khala_coding.chatcmpl_ffe4aef49ef94614be78bc9c8c7b3b62`
  completed locally on `codex-3` against `91edb870c3` with accepted closeout
  `assignment.closeout.de5c448aa8a73c1639aaff89`, producing a candidate #6311
  patch that persists canonical scheduled-skip diagnostics. It is unreviewed and
  unmerged. The owner-scoped proof re-check in this shell returned
  `401 unauthorized`, so the roadmap must not claim exact trace/token proof for
  that assignment until proof is rerun successfully with valid auth.
- Public counter state at refresh:
  `/api/public/khala-tokens-served` returned `198,664,088` at
  `2026-06-26T15:33:09.463Z`. That movement is aggregate and intentionally not
  attributed to the paused #6311 assignment without the owner-scoped proof rows.
- Current serving observability gap: the #6311 GLM readiness route can project
  readiness from persisted routed-completion fallback rows, but canonical
  scheduled `glm-pool-heartbeat` rows have still not been observed after arming.
  The delegated candidate patch addresses skipped/disabled/unarmed diagnostics,
  but it still needs review, tests on current `origin/main`, merge, deploy via
  `deploy:safe`, and live row proof before relying on scheduled watchdog
  evidence for all replicas.

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
   - **Status (2026-06-26): OPEN.** Next action is the actual isolated 8x-host
     pilot run and measured tool-call/quality/throughput decision, not another
     planning doc.

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
     the issue remains broad. Current codeable follow-up: review and either
     land or reject the paused Pylon/Codex candidate patch for canonical
     scheduled-skip GLM pool heartbeat diagnostics, then prove live scheduled
     rows. Do not close until durability/non-Spot/reserve/quota scope is also
     satisfied or explicitly split.
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
     `85ca837413`, but live engine flags and measured throughput lift have not.
     Keep it before #6317/#6312.
8. **#6318 — external-wins admission/priority scheduler. → before #6317.** Internal load
   must be preemptible and yield to external demand. This MUST land before any continuous
   stress so the stress harness can never starve a real user.
   - **Status (2026-06-26): OPEN.** Admission/attribution slices landed and were
     deployed, but the issue remains the hard gate before stress load until
     live mid-flight preemption is proven.
9. **#6317 — continuous max-capacity stress/saturation harness. → after #6318, #6320.**
   The self-driving load that saturates the fleet, ramps concurrency to the ceiling, and
   auto-backs-off on external pressure.
   - **Status (2026-06-26): OPEN; blocked by #6318 and #6320.**
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

Remaining active sequence after the 2026-06-26 ~15:35Z refresh:

`#6323`(run the full-model pilot) ‖ `#6311`(review/land canonical diagnostics,
then durability) → `#6320`(live engine rollout + measured lift) →
`#6318`(finish live preemption proof) → `#6317` → `#6312` → `#6321` →
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
- Khala -> Pylon -> Codex worker status at refresh: delegation is paused by
  operator request, but the worker path is green on current `main` and should be
  used again for codeable roadmap items once resumed. Relevant steering issues:
  #6325/#6326 trace+raw-event persistence and #6331 typed gate diagnostics.
  Relevant steering commits include `5188a6c187` proof readout, `18c25e9f85`
  lifecycle streaming, and `7057e61e0b` default linked-account auto-routing.
