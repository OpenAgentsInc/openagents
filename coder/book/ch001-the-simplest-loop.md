# 1. The simplest loop

An agent is a model in a bounded loop with tools. Everything else in this book
exists to keep that loop honest, auditable, fast, and confined: the session log
in [chapter 2](ch002-model-visible-means-logged.md), the prompt-cache boundary
in [chapter 3](ch003-keep-the-prefix-stable.md), typed capabilities in
[chapter 4](ch004-tools-as-typed-capabilities.md), and the permission and
sandbox walls in [chapter 5](ch005-a-sandbox-and-approvals.md).

In OpenAgents, the production turn loop lives in
[`openagents/core/query_loop.coax`](../../openagents/core/query_loop.coax)
(compiled directly into [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax)
via [`openagents/core/core.coax`](../../openagents/core/core.coax)) and pairs
with the native Linux journaled worker profile (`coder-owned-v1` in
[`coax/lib/agent/native-profile.coax`](../../coax/lib/agent/native-profile.coax)).

## Profile

This chapter's profile is `openagents-query-loop-v1, bounded CPU; coder-owned-v1, native Linux x86_64`:

- **Flat whole-object state (`FlawedQueryState`):** one explicit struct owns
  `turn_count`, `has_attempted_reactive_compact`, `max_output_tokens_recovery_count`,
  `stop_hook_active`, `task_budget_remaining`, and `transition`. No hidden
  mutable globals or nested recursive loops.
- **Seven typed continue transitions (`FLAWED_CONT_*`) and ten typed terminal
  exits (`FLAWED_TERM_*`):** every step returns a deterministic
  `FlawedStepDecision` naming either the next transition or the exact terminal
  reason (`completed`, `max_turns`, `blocking_limit`, `prompt_too_long`,
  `aborted_streaming`, `aborted_tools`, `stop_hook_prevented`, `hook_stopped`,
  `image_error`, `model_error`).
- **Single turn counter increment site:** only `FLAWED_CONT_NEXT_TURN`
  increments `turn_count`. Error-recovery retries (`collapse_drain`,
  `reactive_compact`, `max_tokens_escalate`, `max_tokens_recovery`) preserve
  `turn_count` so a recoverable API error never silently burns a user's
  `max_turns` budget.
- **Withheld 4-stage error recovery ladder:** `flawed_should_withhold_stream_error`
  holds recoverable `PROMPT_TOO_LONG`, `MEDIA_SIZE`, and `MAX_OUTPUT_TOKENS`
  errors inside the loop so context collapse, reactive compaction, and token
  escalation run before surfacing a terminal failure.
- **Death-spiral latches:** `has_attempted_reactive_compact` and
  `stop_hook_active` latch across recovery retries so a failing compaction or
  blocking stop hook cannot spin indefinitely; `flawed_check_token_budget`
  additionally stops diminishing continuations (`< 500` delta tokens across 3+
  turns).
- **Native host ownership (`coder-owned-v1`):** `coax/lib/agent/journal-flow.coax`,
  `journal-file.coax`, and `native-profile.coax` bind each turn's intent and
  command execution to a durable hash-chained journal on Linux x86_64.

## Worked example

The runnable example [query-loop.coax](../examples/query-loop.coax) drives
`openagents/core/query_loop.coax` directly through four steps—tool rollovers,
withheld `PROMPT_TOO_LONG` reactive compaction without incrementing
`turn_count`, clean completion with `CacheSafeParams` capture, and a separate
`max_turns = 2` cap refusal.

<!-- source: coder/examples/query-loop.coax example -->
```jai
main :: () {
  cfg := FlawedQueryConfig.{
    auto_compact_enabled = true,
    reactive_compact_enabled = true,
    otk_slot_enabled = true,
    is_main_or_sdk_source = true,
    max_turns = 3,
  };
  st: [1] FlawedQueryState;
  st[0] = flawed_query_state_init(10000);
  bt: [1] FlawedBudgetTracker;

  // Step 1: model requests 2 tools -> advance turn_count 1 -> 2
  d1 := flawed_query_step(st[..], bt[..], cfg, FlawedStepObservation.{observed_tool_uses = 2});
  print("step 1 (2 tools): cont=% turn=% terminal=%\n", cont_name(d1.cont), st[0].turn_count, d1.is_terminal);

  // Step 2: withheld Prompt-Too-Long error -> reactive compact retry; turn_count stays 2
  withheld := flawed_should_withhold_stream_error(cfg, FLAWED_SERR_PROMPT_TOO_LONG);
  d2 := flawed_query_step(st[..], bt[..], cfg, FlawedStepObservation.{
    stream_error = FLAWED_SERR_PROMPT_TOO_LONG,
    reactive_compact_ok = true,
    reactive_reclaimed_tokens = 2500,
  });
  print("step 2 (withheld PTL=%): cont=% turn=% task_budget=%\n",
    withheld, cont_name(d2.cont), st[0].turn_count, st[0].task_budget_remaining);

  // Step 3: tool rollover advances turn_count 2 -> 3
  d3 := flawed_query_step(st[..], bt[..], cfg, FlawedStepObservation.{observed_tool_uses = 1});
  print("step 3 (1 tool): cont=% turn=% terminal=%\n", cont_name(d3.cont), st[0].turn_count, d3.is_terminal);

  // Step 4: clean reply terminates and saves CacheSafeParams
  d4 := flawed_query_step(st[..], bt[..], cfg, FlawedStepObservation.{});
  print("step 4 (reply): terminal=% reason=% save_cache_safe=% notify_done=%\n",
    d4.is_terminal, term_name(d4.terminal), d4.save_cache_safe_params, d4.notify_commands_completed);

  // Separate bounded loop hitting max_turns = 2 cleanly refuses infinite rollovers
  cap_cfg := cfg;
  cap_cfg.max_turns = 2;
  cap_st: [1] FlawedQueryState;
  cap_st[0] = flawed_query_state_init(0);
  cap_bt: [1] FlawedBudgetTracker;
  _ = flawed_query_step(cap_st[..], cap_bt[..], cap_cfg, FlawedStepObservation.{observed_tool_uses = 1});
  d_cap := flawed_query_step(cap_st[..], cap_bt[..], cap_cfg, FlawedStepObservation.{observed_tool_uses = 1});
  print("max_turns cap (2): terminal=% reason=% turn=%\n",
    d_cap.is_terminal, term_name(d_cap.terminal), cap_st[0].turn_count);
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/query-loop.coax
```

```text
step 1 (2 tools): cont=next_turn turn=2 terminal=false
step 2 (withheld PTL=true): cont=reactive_compact turn=2 task_budget=7500
step 3 (1 tool): cont=next_turn turn=3 terminal=false
step 4 (reply): terminal=true reason=completed save_cache_safe=true notify_done=true
max_turns cap (2): terminal=true reason=max_turns turn=3
```

The checked fixture is [query-loop.out](../examples/query-loop.out), matching
byte-for-byte across debug and release builds.

## Checks and production adoption

- `#authority` unit & mutation checks in
  [`openagents/core/query_loop.coax`](../../openagents/core/query_loop.coax)
  (`coax/bin/coax test openagents/core/query_loop.coax --library`).
- End-to-end integration test in
  [`openagents/core/core.coax`](../../openagents/core/core.coax) and live
  runtime adoption in [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax).
- Native journaled worker verification in
  [`coax/lib/agent/native-profile.coax`](../../coax/lib/agent/native-profile.coax)
  and [`coder/tests/journal-worker.coax`](../../coder/tests/journal-worker.coax).

## Done means

- [x] Every turn transition and terminal exit is explicitly typed and checked
  under `#authority` in `openagents/core/query_loop.coax`.
- [x] `coder/examples/query-loop.coax` matches `coder/examples/query-loop.out`
  in both debug and release.
- [x] `openagents/runtime/oa-worker.coax` runs the shared `openagents/core/` loop as
  OpenAgents Desktop's primary agent worker.
