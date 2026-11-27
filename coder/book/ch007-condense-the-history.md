# 7. Condense the history

> **Status: partly implemented.** OpenAgents Core implements the five-layer
> compaction pipeline, 3-failure auto-compact circuit breaker, PTL
> head-truncation group drops, subagent cleanup isolation, task-budget
> accounting, and idle cache-expiry compaction triggers on CPU and in
> `oa-worker`. Controlled ablations measuring post-compaction coding accuracy
> and prefill savings across long real-repository tasks remain open.

Long coding sessions inevitably outgrow a model's effective context window
(32K on local Inkling 2 Mini; 200K+ on cloud models). Naively dropping random
messages breaks tool-use/tool-result pairing and erases critical file state;
naively retrying failed compactions triggers an infinite API retry storm.

OpenAgents implements structured compaction in
[`openagents/core/compaction.coax`](../../openagents/core/compaction.coax)
paired with cloud idle cache-expiry detection in
[`coax/lib/agent/engine/cache-policy.coax`](../../coax/lib/agent/engine/cache-policy.coax),
adopted directly by [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax):

- **Effective window & warning thresholds (`flawed_effective_context_window`,
  `flawed_auto_compact_threshold`, `flawed_calc_token_warning_state`):**
  reserves capped output headroom (`min(model_max_out, 20000)` -> `180,000` for
  a `200,000` window) and computes snip-adjusted warning, auto-compact
  (`167,000`), and blocking thresholds.
- **6-gate auto-compact + 3-failure circuit breaker (`flawed_should_auto_compact`,
  `flawed_autocompact_next_failures`):** prevents recursive compaction inside
  compaction/subagent utility calls and trips permanently after 3 consecutive
  failures so broken sessions never burn quota in a tight loop.
- **PTL head-truncation group drops (`flawed_ptl_drop_groups`):** drops whole
  leading API turn groups until the target token gap is reclaimed, preserving
  coherent tool-call/result boundaries.
- **Subagent cleanup isolation (`flawed_post_compact_cleanup_plan`):** a main
  thread compaction refreshes user context and memory caches, whereas a
  subagent compaction leaves the parent's shared caches untouched.
- **Task-budget continuity & idle compaction (`flawed_subtract_task_budget`,
  `cp_cache_status`):** subtracts pre-compaction context tokens across resets
  so global task budgets stay honest, and recommends `/compact` automatically
  when a 5-minute cloud cache TTL expires on a large context.

## Profile

This chapter's profile is `openagents-compaction-v1, bounded CPU`:

- Reserves output headroom and computes exact warning/auto/blocking states.
- Trips the auto-compact circuit breaker at exactly 3 consecutive failures.
- Drops minimum whole leading groups to meet a PTL recovery target.
- Isolates subagent post-compact cleanup from main-thread caches.
- Preserves task-budget subtraction across compaction resets and surfaces idle
  cache-expiry compaction eligibility.

## Worked example

The runnable example [compaction-pipeline.coax](../examples/compaction-pipeline.coax)
exercises all five compaction invariants directly against `openagents/core/compaction.coax`
and `coax/lib/agent/engine/cache-policy.coax`.

<!-- source: coder/examples/compaction-pipeline.coax example -->
```jai
main :: () {
  // 1. Effective context window reserves capped output headroom (200k - 20k = 180k)
  eff := flawed_effective_context_window(200000, 64000, 0);
  auto_thresh := flawed_auto_compact_threshold(eff, 0);
  warn := flawed_calc_token_warning_state(170000, 0, eff, true, 0);
  print("window: effective=% auto_threshold=% above_auto=% at_blocking=%\n",
    eff, auto_thresh, warn.is_above_auto_threshold, warn.is_at_blocking_limit);

  // 2. 6-gate auto-compact trigger + 3-failure circuit breaker (stops retry storms)
  fire_ok := flawed_should_auto_compact(FLAWED_QSRC_MAIN_OR_SDK, false, true, false, false, 0, 170000, 0, eff, 0);
  f1 := flawed_autocompact_next_failures(0, false, false);
  f2 := flawed_autocompact_next_failures(f1, false, false);
  f3 := flawed_autocompact_next_failures(f2, false, false);
  fire_tripped := flawed_should_auto_compact(FLAWED_QSRC_MAIN_OR_SDK, false, true, false, false, f3, 170000, 0, eff, 0);
  print("autocompact: initial_fire=% failures_after_3=% tripped_fire=%\n", fire_ok, f3, fire_tripped);

  // 3. PTL head-truncation drops oldest coherent API groups until target gap is recovered
  groups := [5] int.[12000, 18000, 15000, 9000, 6000];
  dropped := flawed_ptl_drop_groups(groups[..], 5, 25000);
  print("ptl head-drop: groups_dropped=% of 5 (reclaimed 30000 >= 25000)\n", dropped);

  // 4. Subagent compaction isolation: main thread clears shared caches; subagent leaves parent intact
  main_plan := flawed_post_compact_cleanup_plan(FLAWED_QSRC_MAIN_OR_SDK);
  sub_plan := flawed_post_compact_cleanup_plan(FLAWED_QSRC_SUBAGENT);
  print("cleanup isolation: main_clears_user_ctx=% subagent_clears_user_ctx=%\n",
    main_plan.clear_user_context_and_memory, sub_plan.clear_user_context_and_memory);

  // 5. Task budget subtraction accounts for pre-compact tokens across resets
  rem := flawed_subtract_task_budget(false, 0, 100000, 40000);
  rem2 := flawed_subtract_task_budget(true, rem, 100000, 15000);
  idle_rec := cp_cache_status(1000, 310000, 0, 18000, 32000, true);
  print("task budget: after_compact1=% after_compact2=% idle_compact_recommended=%\n",
    rem, rem2, idle_rec == CP_CACHE_EXPIRED_COMPACT);
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/compaction-pipeline.coax
```

```text
window: effective=180000 auto_threshold=167000 above_auto=true at_blocking=false
autocompact: initial_fire=true failures_after_3=3 tripped_fire=false
ptl head-drop: groups_dropped=2 of 5 (reclaimed 30000 >= 25000)
cleanup isolation: main_clears_user_ctx=true subagent_clears_user_ctx=false
task budget: after_compact1=60000 after_compact2=45000 idle_compact_recommended=true
```

The checked fixture is [compaction-pipeline.out](../examples/compaction-pipeline.out).

## What remains

- Long-horizon coding-task ablations comparing five-layer compaction quality
  and prefill latency on both 32K local Inkling 2 Mini and cloud frontier models.

## Done means

- [x] Five-layer compaction thresholds, 3-failure breaker, PTL head-truncation,
  subagent cleanup isolation, and budget subtraction pass `#authority` checks
  and run inside `openagents/runtime/oa-worker.coax`.
- [x] `coder/examples/compaction-pipeline.coax` matches
  `coder/examples/compaction-pipeline.out` in debug and release.
- [ ] Post-compaction task-success and prefill-cost ablations are measured on
  held-out multi-hour repository sessions.
