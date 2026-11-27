# 5. A sandbox and approvals

Typed tools constrain *what* the model can name; the permission engine and
shell sandbox constrain *what may actually run* on the host. Neither wall may
be quietly weakened by user convenience modes or ambiguous shell syntax.

In OpenAgents, approval and confinement logic lives across three `#authority`
modules compiled into [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax)
(alongside CLI approval policy in [`coder/tui/approvals.coax`](../../coder/tui/approvals.coax)
and [`coax/lib/agent/engine/ceiling.coax`](../../coax/lib/agent/engine/ceiling.coax)):

- **Bypass-immune permission engine (`openagents/core/permissions.coax`):**
  evaluates the 7-step inner wall (1a–1g) before any mode shortcut applies.
  Editing protected paths such as `.git/hooks/pre-commit` triggers a
  bypass-immune `safetyCheck` that still yields `ASK` even in `BYPASS` mode;
  `FlawedResolveOnce` enforces two-phase `claim()`/`deliver()` so concurrent
  approval racers (UI card vs. hook vs. SDK) cannot double-deliver; and
  `FlawedDenialTracker` trips a fallback-to-`ASK` circuit breaker after 3
  consecutive auto-classifier denials.
- **Fail-closed shell AST & sandbox (`openagents/core/shell_sandbox.coax`):**
  treats parser aborts or overly complex commands as `FLAWED_AST_TOO_COMPLEX`,
  refuses to strip environment variables after wrapper commands (preventing
  HackerOne #3543050 env-prefix bypasses), gates per-command OS sandboxing via
  `flawed_should_use_sandbox`, and blocks bare-git config writes
  (`FLAWED_BAREGIT_DENY_WRITE` / `FLAWED_BAREGIT_POST_SCRUB`).
- **Bypass-immune money & secret boundaries (`openagents/core/members.coax`):**
  `WalletPay` (satoshi spending), `OpenAgentsCli vault` secret retrieval, and
  `nostr` signing/publishing are `#authority`-proven bypass-immune: they ALWAYS
  require an interactive `ASK` card in `BYPASS`/`AUTO`/session-allow modes and
  `DENY` in non-interactive `PLAN`/`DONT_ASK` modes.

## Profile

This chapter's profile is `openagents-sandbox-approvals-v1, bounded CPU; coder-owned-v1, native Linux x86_64`:

- Protected paths (`.git/hooks/...`) stay `ASK` even under `FLAWED_PMODE_BYPASS`,
  while managed worktree edits stay safe.
- `FlawedResolveOnce` allows exactly one racer to claim and deliver a verdict.
- Three consecutive denials trip `flawed_denial_should_fallback`.
- Shell AST aborts fail closed, wrapper env-stripping is blocked, sandboxing
  engages when configured, and bare-git escapes are denied or scrubbed.
- `WalletPay` and `OpenAgentsCli vault get` never auto-approve.

## Worked example

The runnable example [sandbox-approvals.coax](../examples/sandbox-approvals.coax)
drives `permissions.coax`, `shell_sandbox.coax`, and `members.coax` directly.

<!-- source: coder/examples/sandbox-approvals.coax example -->
```jai
main :: () {
  // 1. Protected path safety feeds bypass-immune 1g safetyCheck — even in BYPASS mode
  ps_hook := flawed_check_path_safety_for_auto_edit("/repo/.git/hooks/pre-commit");
  ps_wt := flawed_check_path_safety_for_auto_edit("/repo/.claude/worktrees/wt1/main.coax");
  dt: [1] FlawedDenialTracker;
  v_bypass := flawed_eval_permission_outer(FlawedOuterInput.{
    inner = FlawedInnerInput.{
      mode = FLAWED_PMODE_BYPASS,
      tool_behavior = FLAWED_PBEH_ASK,
      is_safety_check = true,
      classifier_approvable = ps_hook.classifier_approvable,
    },
  }, dt[..]);
  print("path safety: git_hook_safe=% worktree_safe=% bypass_verdict=%\n",
    ps_hook.safe, ps_wt.safe, beh_name(cast(int) v_bypass.behavior));

  // 2. Two-phase ResolveOnce prevents duplicate approval delivery across concurrent racers
  ro: [1] FlawedResolveOnce;
  c1 := flawed_resolve_claim(ro[..], 1);
  c2 := flawed_resolve_claim(ro[..], 2);
  d1 := flawed_resolve_deliver(ro[..], FLAWED_PBEH_ALLOW);
  print("resolve_once: racer1_claim=% racer2_claim=% delivered=%\n", c1, c2, d1);

  // 3. Denial circuit breaker trips after 3 consecutive auto-classifier blocks
  for i: 0..<3 { flawed_denial_record_block(dt[..]); }
  print("denial breaker: blocks=% fallback_to_ask=%\n",
    dt[0].consecutive, flawed_denial_should_fallback(dt[..]));

  // 4. Shell AST fail-closed parse, HackerOne #3543050 env strip, sandbox & bare-git guards
  ast := flawed_classify_parse_result(FLAWED_PARSE_ABORTED, 10, 5);
  strip_term := flawed_can_strip_env_var(FLAWED_ENVSTRIP_ALLOW, true, "TERM");
  use_sb := flawed_should_use_sandbox(true, false, false, false, true, true);
  bg_exist := flawed_bare_git_action(true);
  bg_new := flawed_bare_git_action(false);
  print("shell sandbox: aborted_too_complex=% strip_after_wrapper=% sandbox=% bare_git_deny=% bare_git_scrub=%\n",
    ast == FLAWED_AST_TOO_COMPLEX, strip_term, use_sb,
    bg_exist == FLAWED_BAREGIT_DENY_WRITE, bg_new == FLAWED_BAREGIT_POST_SCRUB);

  // 5. OpenAgents member pack bypass-immunity: WalletPay & vault get ALWAYS ask in BYPASS,
  //    and DENY in non-interactive PLAN/DONT_ASK modes
  wp_bypass := oa_member_tool_permission("WalletPay", "lnbc100n...", OA_MP_PMODE_BYPASS, true, true);
  wp_plan := oa_member_tool_permission("WalletPay", "lnbc100n...", OA_MP_PMODE_PLAN, true, true);
  vault_bypass := oa_member_tool_permission("OpenAgentsCli", "vault get api-key", OA_MP_PMODE_BYPASS, true, true);
  vault_status := oa_member_tool_permission("OpenAgentsCli", "vault status", OA_MP_PMODE_DEFAULT, false, false);
  print("member gates: WalletPay(bypass=% plan=%) vault(get_bypass=% status_default=%)\n",
    beh_name(wp_bypass), beh_name(wp_plan), beh_name(vault_bypass), beh_name(vault_status));
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/sandbox-approvals.coax
```

```text
path safety: git_hook_safe=false worktree_safe=true bypass_verdict=ask
resolve_once: racer1_claim=true racer2_claim=false delivered=true
denial breaker: blocks=3 fallback_to_ask=true
shell sandbox: aborted_too_complex=true strip_after_wrapper=false sandbox=true bare_git_deny=true bare_git_scrub=true
member gates: WalletPay(bypass=ask plan=deny) vault(get_bypass=ask status_default=allow)
```

The checked fixture is [sandbox-approvals.out](../examples/sandbox-approvals.out).

## Checks and production adoption

- `#authority` unit and mutation suites in
  [`openagents/core/permissions.coax`](../../openagents/core/permissions.coax),
  [`openagents/core/shell_sandbox.coax`](../../openagents/core/shell_sandbox.coax),
  and [`openagents/core/members.coax`](../../openagents/core/members.coax).
- Adopted live in [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax),
  [`coder/tui/coder.coax`](../../coder/tui/coder.coax), and
  [`apps/openagents-desktop/page.coax`](../../apps/openagents-desktop/page.coax),
  with native Linux confinement in
  [`coax/lib/agent/native-profile.coax`](../../coax/lib/agent/native-profile.coax).

## Done means

- [x] Bypass-immune path safety, ResolveOnce, denial circuit breaker, shell AST
  classification, bare-git defense, and `WalletPay`/vault gates pass `#authority`
  and mutation checks.
- [x] `coder/examples/sandbox-approvals.coax` matches
  `coder/examples/sandbox-approvals.out` in debug and release.
- [x] Native Linux worker confinement and desktop approval cards enforce the
  same boundaries in production.
