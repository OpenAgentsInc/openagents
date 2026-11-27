# 24. Coder as a member

> **Status: partly implemented.** The unified pure-Coax OpenAgents Core runtime
> ([`openagents/core/core.coax`](../../openagents/core/core.coax),
> [`query_loop.coax`](../../openagents/core/query_loop.coax),
> [`wire.coax`](../../openagents/core/wire.coax)), `#authority` member pack
> dispatch and bypass-immune permission policy
> ([`openagents/core/members.coax`](../../openagents/core/members.coax), `#14057`),
> and native Desktop worker ([`coder/tui/oa-worker.coax`](../../coder/tui/oa-worker.coax))
> run on `cpu`, grounded by [`openagents/examples/coder-member-dispatch.coax`](../examples/coder-member-dispatch.coax)
> and [`openagents/examples/coder-member-dispatch.out`](../examples/coder-member-dispatch.out),
> and are adopted by [`apps/openagents-desktop/Window+Runs.swift`](../../apps/openagents-desktop/Window+Runs.swift).
> Remaining: standalone fixture-repo worktree/push adapter and `openagents.exec.v1`
> protobuf codec in `openagents/agent/members/coder.coax`.

## Premise

The agent can talk and answer. This chapter gives it its first member that
does work: when you ask about your repositories, the front hands the request
to Coder.

## Draft

[Episode 289](../../docs/transcripts/289.md): ask it to interact with your
repos, and it loads the Coder agent. This is promise P3, and Coder is the
first specialist member.

The split is the one the Coder book's plan sets out. OpenAgents is the general
agent. It speaks as "we", owns identity, the wallet, knowledge and routing,
and writes every reply in shared channels. Coder takes one repository task
from start to finish: it reads the code, runs commands, edits, tests and opens
the pull request. Coder speaks as "I" and has opinions about code.

Coding is the first member for a reason. It is the job this project has
returned to since Faerie read its first issue in [Episode
20](../../docs/transcripts/020.md). Code is also a general way to act on
digital information, and coding agents check their own work with tests. That
makes Coder the strongest executor we have, and the general agent's answer to
the first pressure, execution.

The interface is small. A task goes in with a grant for one repository. A
result comes out: a change, the checks that ran, and whether they passed. A
red result pushes nothing and says why.

This book does not rebuild Coder. The Coder book does that, in 43 chapters.
This chapter builds the adapter that makes Coder a member, and the route that
sends it work.

## What we build

A plan for what this chapter ships once the draft is approved:

- A member adapter over Coder: task in, result and checks out, under a
  repository grant.
- The issue flow as a member call: its own worktree, the repository's checks,
  and a push only when they pass.
- Labeled rows that separate "work on my repository" from a question about
  code.

It builds on what exists today:

- [`openagents/core/members.coax`](../../openagents/core/members.coax) (`#authority` member packs and bypass-immune `WalletPay`, vault, and Nostr signing guards, `#14057`)
- [`openagents/core/core.coax`](../../openagents/core/core.coax), [`openagents/core/query_loop.coax`](../../openagents/core/query_loop.coax), and [`openagents/core/wire.coax`](../../openagents/core/wire.coax) (the unified 24-layer pure-Coax runtime, `#14050`–`#14057`)
- [`coder/tui/oa-worker.coax`](../../coder/tui/oa-worker.coax) (native OpenAgents Core worker replacing deprecated `coax/lib/microcoder`, `#13983`)
- [`coax/lib/inky/`](../../coax/lib/inky) (local `Inkling-2-Mini` coding agent loop)
- [`coder/tui/coder.coax`](../../coder/tui/coder.coax)
- [`coax/lib/agent`](../../coax/lib/agent)
- [`coder/book/README.md`](../../coder/book/README.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A task and its result | `openagents.exec.v1` `ExecRequest`, `ExecResult` ([spec 17.2](../../docs/protocol/openagents-protocol-spec.md#172-openagentsexecv1)) | Nostr kinds 25920 and 26920, encrypted (NIP-CJ); HTTP `POST /v1/jobs` | Spec sketch; no `.proto` file |
| Coder's session log and stream records | Planned `openagents.session.log.v1alpha1`; `coder.observability`, `coder.reply-stream`, `coder.route` ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), steps C1 to C3) | Files and a line stream | Hand-written JSON today. The compliance plan moves each to one codec |

### The chapter's check

- **Source:** [`openagents/core/members.coax`](../../openagents/core/members.coax),
  [`coax/lib/agent/router/decide.coax`](../../coax/lib/agent/router/decide.coax),
  [`openagents/examples/coder-member-dispatch.coax`](../examples/coder-member-dispatch.coax),
  and [`openagents/examples/coder-member-dispatch.out`](../examples/coder-member-dispatch.out)
  (remaining: standalone fixture-repo push adapter in `openagents/agent/members/coder.coax`).
- **Trace:** routes `"fix the typo in README.md"` (`repo=openagents`) to native
  `pack=coder` (`tier=cloud_fast`, `delegate=none`), constructs the full 7-pack
  member bitmask (`127`, including worktree agent tools), allows read-only
  `WalletStatus`, `KnowledgeSearch`, and `OpenAgentsCli tasks list` in `PLAN`
  mode, and gates mutating `OpenAgentsCli tasks create` behind `ask` in `DEFAULT`
  and `deny` in `PLAN`.
- **Refusal:** `WalletPay` is bypass-immune (`immune=true`), refusing auto-approval
  with `ask` even under `BYPASS` + `bypass_danger` + `session_allow` and `deny`
  in `PLAN` and `DONT_ASK`; `OpenAgentsCli vault get` and `nostr publish` enforce
  the same bypass-immune boundary; and disallowed subcommands (`rm -rf /`) or
  newline-injected arguments (`tasks list\nrm`) are refused with `deny` even in
  `BYPASS`.
- **Independent check:** `coax/bin/coax run openagents/examples/coder-member-dispatch.coax`
  and `--release` match [`openagents/examples/coder-member-dispatch.out`](../examples/coder-member-dispatch.out),
  alongside `#forall` proofs and zero-surviving-mutant `#authority` checks in
  [`openagents/core/members.coax`](../../openagents/core/members.coax).

Planned paths: `openagents/agent/members/coder.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, loading Coder
  - [Episode 20, *Planning a GitHub Agent*](../../docs/transcripts/020.md): planning the first GitHub agent
  - [Episode 30, *Faerie Debugs Failing Tests*](../../docs/transcripts/030.md): the agent debugs failing tests
  - [Episode 123, *GitHub Issues to Pull Requests*](../../docs/transcripts/123.md): issues to pull requests
  - [Episode 161, *Basic Coding Loop*](../../docs/transcripts/161.md): a basic coding loop
  - [Episode 275, *Coder*](../../docs/transcripts/275.md)
  - [Episode 277, *Coder Terminal*](../../docs/transcripts/277.md): the Coder terminal
- **Old code at `coder-v1.0.0`, crates:** `coder`, `coder-one`, `microcoder`, `microcoder-loop`, `briefed-agent`, `supervise`, `coder-project`
- **NIPs:** [NIP-WORK](../../nostr/nips/openagents/NIP-WORK.md), [NIP-RUN](../../nostr/nips/openagents/NIP-RUN.md), [NIP-HOST](../../nostr/nips/openagents/NIP-HOST.md)
- **Also:**
  - `programs/solve-issue.json` at `coder-v1.0.0`
  - [Coder book, plan: OpenAgents and Coder](../../coder/book/PLAN.md#openagents-and-coder)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P3

## Source listing

Run the Chapter 24 example in [`openagents/examples/coder-member-dispatch.coax`](../examples/coder-member-dispatch.coax) (committed transcript [`openagents/examples/coder-member-dispatch.out`](../examples/coder-member-dispatch.out)):

<!-- source: openagents/examples/coder-member-dispatch.coax coder-member-dispatch -->
```jai
cmd_run :: () -> bool {
  // 1. Route a repository task to native OpenAgents Core Coder member pack.
  q: [1] Rtr2PlanRequest;
  p: [1] Rtr2Plan;
  w: [1] Rtr2Work;
  q[0] = Rtr2PlanRequest.{
    text = "fix the typo in README.md",
    repo = "openagents",
    local_inkling_ready = true,
    cloud_ready = true
  };
  rtr2_decide_mock(
    q[..],
    "primary_pack=coder,compute_tier=cloud_fast,explicit_delegate=none,difficulty=1,effort=low,missing_capability=50,missing_capability_kind=none",
    0, p[..], w[..]
  );
  base_mask := oa_member_packs_from_flags(true, true, true, true);
  full_mask := base_mask | OA_PACK_AGENTS | OA_PACK_KNOWLEDGE | OA_PACK_CAPABILITY;
  print("route ok=% pack=% tier=% delegate=% packs_mask=% agent_wt=%\n",
    p[0].ok, p[0].capability_packs[0], p[0].compute_tier, p[0].explicit_delegate,
    full_mask, oa_member_is_agent_tool("EnterWorktree"));

  // 2. Read-only member tools allow in PLAN; mutating CLI asks in DEFAULT and denies in PLAN.
  b_wstat := oa_member_tool_permission("WalletStatus", "", OA_MP_PMODE_PLAN, false, false);
  b_kb := oa_member_tool_permission("KnowledgeSearch", "episode 289", OA_MP_PMODE_PLAN, false, false);
  b_cli_ro := oa_member_tool_permission("OpenAgentsCli", "tasks list", OA_MP_PMODE_PLAN, false, false);
  b_cli_mut_def := oa_member_tool_permission("OpenAgentsCli", "tasks create fix", OA_MP_PMODE_DEFAULT, false, false);
  b_cli_mut_plan := oa_member_tool_permission("OpenAgentsCli", "tasks create fix", OA_MP_PMODE_PLAN, false, false);
  print("reads wallet_status=% kb_search=% cli_list=% cli_create_def=% cli_create_plan=%\n",
    cmd_beh_name(b_wstat), cmd_beh_name(b_kb), cmd_beh_name(b_cli_ro),
    cmd_beh_name(b_cli_mut_def), cmd_beh_name(b_cli_mut_plan));

  // 3. Bypass-immune WalletPay + vault/nostr signing + disallowed/injected CLI refusals.
  pay_immune := oa_member_is_bypass_immune("WalletPay", "lnbc100n1...");
  b_pay_bypass := oa_member_tool_permission("WalletPay", "lnbc100n1...", OA_MP_PMODE_BYPASS, true, true);
  b_pay_plan := oa_member_tool_permission("WalletPay", "lnbc100n1...", OA_MP_PMODE_PLAN, true, true);
  b_pay_dont := oa_member_tool_permission("WalletPay", "lnbc100n1...", OA_MP_PMODE_DONT_ASK, true, true);
  b_vault_bypass := oa_member_tool_permission("OpenAgentsCli", "vault get api_key", OA_MP_PMODE_BYPASS, true, true);
  b_nostr_plan := oa_member_tool_permission("OpenAgentsCli", "nostr publish hello", OA_MP_PMODE_PLAN, true, true);
  b_bad_sub := oa_member_tool_permission("OpenAgentsCli", "rm -rf /", OA_MP_PMODE_BYPASS, true, true);
  b_bad_nl := oa_member_tool_permission("OpenAgentsCli", "tasks list\nrm", OA_MP_PMODE_BYPASS, true, true);
  print("wallet_pay immune=% bypass=% plan=% dont_ask=%\n",
    pay_immune, cmd_beh_name(b_pay_bypass), cmd_beh_name(b_pay_plan), cmd_beh_name(b_pay_dont));
  print("cli_guards vault_bypass=% nostr_plan=% bad_sub=% bad_newline=%\n",
    cmd_beh_name(b_vault_bypass), cmd_beh_name(b_nostr_plan),
    cmd_beh_name(b_bad_sub), cmd_beh_name(b_bad_nl));

  return p[0].ok && p[0].capability_packs[0] == "coder" && p[0].explicit_delegate == "none" &&
         full_mask == 127 && b_wstat == OA_MP_PBEH_ALLOW && b_kb == OA_MP_PBEH_ALLOW &&
         b_cli_ro == OA_MP_PBEH_ALLOW && b_cli_mut_def == OA_MP_PBEH_ASK && b_cli_mut_plan == OA_MP_PBEH_DENY &&
         pay_immune && b_pay_bypass == OA_MP_PBEH_ASK && b_pay_plan == OA_MP_PBEH_DENY && b_pay_dont == OA_MP_PBEH_DENY &&
         b_vault_bypass == OA_MP_PBEH_ASK && b_nostr_plan == OA_MP_PBEH_DENY &&
         b_bad_sub == OA_MP_PBEH_DENY && b_bad_nl == OA_MP_PBEH_DENY;
}
```

Output (`coax/bin/coax run openagents/examples/coder-member-dispatch.coax`, identical in `--release`):

```text
route ok=true pack=coder tier=cloud_fast delegate=none packs_mask=127 agent_wt=true
reads wallet_status=allow kb_search=allow cli_list=allow cli_create_def=ask cli_create_plan=deny
wallet_pay immune=true bypass=ask plan=deny dont_ask=deny
cli_guards vault_bypass=ask nostr_plan=deny bad_sub=deny bad_newline=deny
ok=true
```
