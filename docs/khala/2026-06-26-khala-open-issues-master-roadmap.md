# Khala Open-Issues Master Roadmap — One Solve Sequence

> Status: **internal execution roadmap, 2026-06-26.** Direction-setting, not public
> claim copy and not a product promise. It orders **every open `OpenAgentsInc/openagents`
> issue** into a single dependency-aware sequence agents should work top-to-bottom. It
> flips no promise state and ships no code itself.

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
2. **#6319 (fallback-chain repair slice) ‖ parallel with #6310.** The fallback chain is
   itself broken — **GPT-OSS-120B (fallback #2) returns 404**, GPT-OSS-20B (#3) returns
   empty — so GLM overflow degrades two dead hops before a serving lane. At minimum, in
   Phase 0: repair/replace the dead lanes + treat empty content as a failure so a 200 is
   never an empty/no-tool response. (Full #6319 program continues in Phase 1.)
3. **#6323 — pilot `nvidia/GLM-5.2-NVFP4` (full 753B) on the 8× host ‖ parallel with #6310,
   as a candidate FIX for it.** Our REAP-504B already uses the canonical `glm47`/`glm45`
   parsers, so #6310 is the pruned checkpoint, not config. NVIDIA's full 753B NVFP4
   (near-FP8, agentic-tool-use-validated, MIT) fits our one `g4-standard-384` 8× RTX PRO 6000
   host (TP-8, ~381 GB weights in 768 GB). Deploy it there and test: does it tool-call clean
   where REAP `provider_error`s? If yes, it's both the #6310 fix and the quality upgrade —
   route the GLM coding lane to it, keep REAP-504B on the 4× hosts. (Eval:
   `docs/inference/2026-06-26-nvidia-glm-5.2-nvfp4-evaluation.md`. Scaling the full model
   beyond one host depends on 8× Blackwell quota/capacity — #6311.)

## Phase 1 — Reliable serving foundation

Make the fleet trustworthy before pushing load through it.

3. **#6319 — reliability hardening program (full).** Per-replica health + circuit-breaker,
   empty-fallback-as-failure, SLO-based shedding, request hedging, and failure telemetry
   (provider_error / empty / fallback / invalid-tool rates) so the next breakage is visible
   without a user mailing screenshots. → continues from Phase 0 slice.
4. **#6313 — real OpenRouter fallback lane ‖ parallel.** No OpenRouter inference lane
   exists today (only resale/identity refs + a key). Build the adapter + registration +
   plan entry, wired as a real fallback tier with fail-over tests. Gives the chain a
   working terminal hop.
5. **#6311 — durable (non-Spot) GLM fleet + keep-warm ‖ parallel.** All 10 replicas are
   Spot; 8 lack the STOP-watchdog. Add keep-warm on every replica, multi-region
   auto-replace, an on-demand reserve, and the us-central1 quota increase. (Cross-refs
   hydralisk #95 durable host, #99 prebake-weights image.)
6. **#6259 + #6315 — green end-to-end GLM-serving smoke. → after #6310.** Get the
   Khala→GLM verification smoke passing for real (served-worker disclosure + counter
   increment); #6315 is the receipt-ref fix for the zero-debit operator-exempt token.
   This is the regression gate the rest of the work leans on.

## Phase 2 — Maximize throughput (tokens/sec)

7. **#6320 — inference-engineering throughput optimizations. → after #6319.** THE lever:
   replicas run single-flight, so continuous batching is OFF — raise `--max-num-seqs` to
   unlock it (a multiple, not a percent), then stack chunked prefill + engine-side prefix
   caching + speculative/MTP decode + eval-gated quantization. Biggest tok/s win in the
   whole roadmap; do it before stress-testing so you measure the real ceiling.
8. **#6318 — external-wins admission/priority scheduler. → before #6317.** Internal load
   must be preemptible and yield to external demand. This MUST land before any continuous
   stress so the stress harness can never starve a real user.
9. **#6317 — continuous max-capacity stress/saturation harness. → after #6318, #6320.**
   The self-driving load that saturates the fleet, ramps concurrency to the ceiling, and
   auto-backs-off on external pressure.
10. **#6312 — max tokens-per-second benchmark. → after #6317.** The decision-grade
    aggregate-throughput number, read from the harness (concurrency sweep, per-replica +
    aggregate tok/s, TTFT, P50/P90/P99, saturation point, in-cloud vs WAN).

## Phase 3 — Autonomous operation

11. **#6321 — Artanis fleet-overseer automation. → after #6317, #6318, #6319.** The
    autonomous control loop (on `artanis-administrator-tick`, approval-gated): watches fleet
    health + throughput + external demand, orchestrates the stress load (start/scale/
    back-off keyed on external pressure), and triggers heal/scale/quarantine — money +
    destructive actions stay owner-gated via `artanis-approval-gates`. This is the layer
    that runs Phases 1–2 continuously without a human.

## Phase 4 — Prove quality (now that serving is reliable + instrumented)

12. **#6253 — replicate + beat GLM-REAP's 69.1% on Terminal-Bench 2.0. → after Phase 1.**
    The competitive goal: a decision-grade Khala-routed run (not the raw-GLM pilot),
    inference-method comparison, beat the baseline.
13. **#6307 — owner-armed real sweep: first `decisionGrade:true` Khala-vs-Fireworks/Vertex
    report ‖ parallel.** The minimum decision suite, run for real over realistic traffic.
14. **#6308 — external head-to-head (recurring quality bar). → after #6307.** Khala vs the
    tools/models developers would otherwise use, on our axes (cost-per-accepted-outcome,
    verified-rate).
15. **#6309 — gym benchmark ladder as a recurring leaderboard. → after #6307.** Big Pickle
    → free models → paid frontier, published and re-scored on every change.

## Phase 5 — Drive adoption (the demand side; GTM #6303)

16. **#6305 — OpenCode → Khala verification checklist + publish. → HARD-after #6310.** Do
    NOT publish the OpenCode recipe until tool-calling actually works; publishing a broken
    coding agent burns the wedge. This is the first external "point your tool at us" win.
17. **#6306 — next ecosystem recipes (Aider, Cline/Continue, Vercel AI SDK, LiteLLM,
    LangChain). → after #6305.** One tool at a time, each with its test checklist.
18. **#6303 — GTM push tracking (umbrella).** Closes when 16–17 + the Phase-4 benchmarks
    land; keep it updated as the demand-side scoreboard.

---

## The single sequence (flat list)

`#6310` ‖ `#6323`(full-model candidate fix) → `#6319(chain-repair)` → `#6319(full)` ‖
`#6313` ‖ `#6311` → `#6259/#6315` → `#6320` → `#6318` → `#6317` → `#6312` → `#6321` →
`#6253` ‖ `#6307` → `#6308` ‖ `#6309` → `#6305` → `#6306` → close `#6303`.

(#6323 runs in parallel at the very front because it could *resolve* #6310 outright; if the
full model tool-calls cleanly, it short-circuits much of the GLM tool-path debugging.)

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
