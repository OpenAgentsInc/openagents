# 8. Programs as tool calls

> **Status: partly implemented.** OpenAgents Core implements hook-vs-rule
> permission arbitration, the `StreamingToolExecutor` 3-level AbortController
> tree, Stage A/B output spill budgeting, and multi-pack member tool dispatch
> under `#authority` in `openagents/runtime/oa-worker.coax`. Running bounded Coax/Wasm
> model-authored mini-programs as single batched tool calls inside `oa-worker`
> remains open (tracked alongside plugin Wasm execution in [Chapter 15](ch015-plugins-with-owned-lifecycles.md)).

When a model issues multiple tool calls in a single turn—or chains tool calls
together across workspace, CLI, wallet, and metaverse packs—the execution
harness must preserve four invariants simultaneously:

1. **Hook-vs-rule precedence (`openagents/core/tool_exec.coax`, inc-4788):**
   a `PreToolUse` hook returning `ALLOW` must NEVER override an explicit user
   or policy `DENY` rule. `flawed_resolve_hook_permission` enforces
   `FLAWED_RULE_DENY -> FLAWED_ARB_DENY` unconditionally.
2. **3-level AbortController isolation (`FlawedStreamingExecutor`):**
   concurrent read-only or Bash calls stream as soon as their arguments arrive;
   a Bash failure cancels sibling calls in the same batch (`sibling_aborted = true`)
   without aborting the entire turn (`turn_aborted = false`), whereas a
   permission rejection (`FLAWED_ABORT_PERM_REJECT`) bubbles upward to halt the
   turn cleanly.
3. **Two-stage output spill budgeting (`openagents/core/tool_spill.coax`):**
   Stage A (`flawed_classify_stage_a_spill`) spills oversized command outputs
   to disk while keeping unbounded `Read` outputs inline (preventing infinite
   `Read -> spill -> Read` loops); Stage B (`flawed_enforce_message_budget`)
   freezes per-message replacement decisions in `FlawedReplacementState` so
   prompt-cache bytes never shift across later turns.
4. **Closed multi-pack dispatch (`openagents/core/members.coax`):**
   every member tool (`GameBuilder`, `OpenAgentsCli`, `WalletPay`,
   `CapabilityDraft`, `CapabilityEval`) enforces its own closed action
   allowlist and read-only vs. mutating classification before execution.

## Profile

This chapter's profile is `openagents-tool-orchestration-v1, bounded CPU`:

- `PreToolUse` `ALLOW` cannot override `FLAWED_RULE_DENY`.
- Mid-batch Bash errors cancel siblings without aborting the turn.
- Stage A preserves `FLAWED_RESULT_UNBOUNDED` reads while spilling oversized
  command outputs; Stage B enforces per-message character budgets and records
  frozen replacement decisions.
- Multi-pack `GameBuilder` actions allow `inspect`, prompt on `build`, and deny
  unlisted actions even in `BYPASS` mode.

## Worked example

The runnable example [tool-orchestration.coax](../examples/tool-orchestration.coax)
exercises hook arbitration, `StreamingToolExecutor` sibling cancellation, Stage
A/B spill budgeting, and `GameBuilder` member dispatch directly against
`openagents/core/`.

<!-- source: coder/examples/tool-orchestration.coax example -->
```jai
main :: () {
  // 1. inc-4788 invariant: PreToolUse hook ALLOW never overrides an explicit settings DENY rule
  a_deny := flawed_resolve_hook_permission(FLAWED_HOOK_PERM_ALLOW, false, false, false, FLAWED_RULE_DENY);
  a_allow := flawed_resolve_hook_permission(FLAWED_HOOK_PERM_ALLOW, false, false, false, FLAWED_RULE_NONE);
  print("hook arbitration: allow_vs_deny=% allow_vs_none=%\n",
    arb_name(a_deny.action), arb_name(a_allow.action));

  // 2. StreamingToolExecutor: two concurrent Bash calls; error in slot 0 cancels sibling
  //    without aborting the entire turn (3-level AbortController isolation)
  ste: [1] FlawedStreamingExecutor;
  _ = flawed_ste_add(ste[..], 1, true, true, true, FLAWED_INTERRUPT_BLOCK);
  _ = flawed_ste_add(ste[..], 2, true, true, true, FLAWED_INTERRUPT_BLOCK);
  started := flawed_ste_process_queue(ste[..]);
  _ = flawed_ste_complete(ste[..], 0, true);
  print("streaming executor: started=% sibling_aborted=% turn_aborted=% slot1_aborted=%\n",
    started, ste[0].sibling_aborted, ste[0].turn_aborted, ste[0].slots[1].aborted);

  // 3. Stage A spill opt-out (unbounded Read keeps full bytes) vs Stage B per-message budget
  stage_a_read := flawed_classify_stage_a_spill(500000, false, FLAWED_RESULT_UNBOUNDED, 1000);
  stage_a_bash := flawed_classify_stage_a_spill(500000, false, 30000, 1000);
  rs: [1] FlawedReplacementState;
  cands: [2] FlawedBudgetCandidate;
  cands[0] = FlawedBudgetCandidate.{tool_use_id = 10, size_chars = 150000, is_unbounded = true};
  cands[1] = FlawedBudgetCandidate.{tool_use_id = 11, size_chars = 250000, is_unbounded = false};
  replaced := flawed_enforce_message_budget(rs[..], cands[..], 2, FLAWED_DEFAULT_MSG_BUDGET_CHARS, FLAWED_PREVIEW_SIZE_BYTES);
  print("spill budget: read_keep=% bash_spill=% stage_b_replaced=% id10=% id11=%\n",
    stage_a_read == FLAWED_SPILL_KEEP, stage_a_bash == FLAWED_SPILL_PERSIST,
    replaced, dec_name(flawed_rstate_lookup(rs[..], 10)), dec_name(flawed_rstate_lookup(rs[..], 11)));

  // 4. Multi-pack orchestration: read-only GameBuilder inspect vs mutating build in default mode
  gb_inspect := oa_member_tool_permission("GameBuilder", "inspect realm-1", OA_MP_PMODE_DEFAULT, false, false);
  gb_build := oa_member_tool_permission("GameBuilder", "build realm-1", OA_MP_PMODE_DEFAULT, false, false);
  gb_bad := oa_member_tool_permission("GameBuilder", "drop-table", OA_MP_PMODE_BYPASS, true, true);
  print("member dispatch: inspect_allow=% build_ask=% unknown_deny=%\n",
    gb_inspect == OA_MP_PBEH_ALLOW, gb_build == OA_MP_PBEH_ASK, gb_bad == OA_MP_PBEH_DENY);
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/tool-orchestration.coax
```

```text
hook arbitration: allow_vs_deny=deny allow_vs_none=allow
streaming executor: started=2 sibling_aborted=true turn_aborted=false slot1_aborted=true
spill budget: read_keep=true bash_spill=true stage_b_replaced=1 id10=kept id11=replaced
member dispatch: inspect_allow=true build_ask=true unknown_deny=true
```

The checked fixture is [tool-orchestration.out](../examples/tool-orchestration.out).

## What remains

- Integrating single-turn bounded Wasm/Coax script execution directly as a tool
  type inside `oa-worker`, measured against ordinary multi-turn tool calls in
  [Chapter 21](ch021-measure-the-harness.md).

## Done means

- [x] Hook-vs-rule arbitration, `StreamingToolExecutor` 3-level aborts, Stage
  A/B output spilling, and closed member-pack dispatch pass `#authority` checks
  and run in `openagents/runtime/oa-worker.coax`.
- [x] `coder/examples/tool-orchestration.coax` matches
  `coder/examples/tool-orchestration.out` in debug and release.
- [ ] Bounded model-written programs as single tool calls are wired into
  `oa-worker` and benchmarked against round-trip tool loops.
