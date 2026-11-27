# 14. A typed plan

> **Status: partly implemented.** The typed `openagents.router.v2alpha1`
> `RoutePlan` schema ([`coax/lib/agent/router/wire.coax`](../../coax/lib/agent/router/wire.coax)),
> `#authority` eligibility and plan-selection policy
> ([`coax/lib/agent/router/policy.coax`](../../coax/lib/agent/router/policy.coax)),
> and mock/offline decision runner ([`coax/lib/agent/router/decide.coax`](../../coax/lib/agent/router/decide.coax),
> [`router.coax`](../../coax/lib/agent/router/router.coax), `#14056`, `#14071`)
> run on `cpu`, grounded by [`openagents/examples/typed-route-plan.coax`](../examples/typed-route-plan.coax)
> and [`openagents/examples/typed-route-plan.out`](../examples/typed-route-plan.out),
> and are consumed by [`coder/tui/oa-worker.coax`](../../coder/tui/oa-worker.coax)
> and [`apps/openagents-desktop/page/agent_settings.coax`](../../apps/openagents-desktop/page/agent_settings.coax).
> Remaining: multi-step grant-carrying program plans and step-log replay in
> `openagents/agent/plan.coax`.

## Premise

With members in place, the front needs a way to say which ones a request gets.
This chapter makes that answer a typed plan that code can run, log and refuse.
It is never prose.

## Draft

In the 2023 loop, the model's text was the plan. Something had to read that
text and act on it, usually another model call, and the agent became a chain
of models reading each other. An error anywhere in the chain was hard to find.

Our front outputs a value with a fixed shape. A plan names the members a
request needs, the grant each one gets, and any limit that applies.
Deterministic code reads the plan and carries it out. Nothing parses prose.

A typed plan has three properties the rest of the book depends on. You can log
it, so anyone can see why a request went where it went. You can replay it, so
two runs of the same plan can be compared. And you can refuse it: a plan that
asks for a grant you haven't given stops with a reason you can read.

Coax checks help here. A plan type can carry its rules, such as "a member with
a money effect always has an approval step", and the compiler runs those
checks on every build.

Most plans are one step: answer this from the knowledge base, or hand this to
Coder. Some are several steps, and chapter 27 turns those into programs.

## What we build

A plan for what this chapter ships once the draft is approved:

- A plan type: steps, the member for each step, its grant, and limits.
- An executor that runs a plan and writes each step to the log.
- Coax checks for the plan's rules, and refusals with readable reasons.

It builds on what exists today:

- [`coax/lib/agent/router/wire.coax`](../../coax/lib/agent/router/wire.coax) (`openagents.router.v2alpha1` `Rtr2PlanRequest` and `Rtr2Plan` JSON/protobuf codec, `#14056`)
- [`coax/lib/agent/router/policy.coax`](../../coax/lib/agent/router/policy.coax) (`#authority` `rtr2_eligible` and `rtr2_pick` plan rules)
- [`coax/lib/agent/router/decide.coax`](../../coax/lib/agent/router/decide.coax) and [`coax/lib/agent/router/router.coax`](../../coax/lib/agent/router/router.coax) (`rtr2_decide_mock`, `rtr2_decide_offline`, and `router.coax plan`)
- [`openagents/core/query_loop.coax`](../../openagents/core/query_loop.coax)
- [`openagents/core/members.coax`](../../openagents/core/members.coax) (`#authority` member packs and bypass-immune money approval checks, `#14057`)
- [`coax/lib/nostr/cj`](../../coax/lib/nostr/cj)
- [`coax/lib/nostr/pol`](../../coax/lib/nostr/pol)
- [`coax/lib/bounded.coax`](../../coax/lib/bounded.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Plan | None yet. The nearest are `openagents.chat.v1` `ConversationResult` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) and `openagents.program.v1` `Step` ([spec 20.1](../../docs/protocol/openagents-protocol-spec.md#201-openagentsprogramv1)) | Planned: the stream a surface draws ([chapter 64](ch064-the-openagents-api.md)) | Gap. The plan needs a schema in `proto/` before any surface draws it ([chapter 66](ch066-the-compliance-rule.md)) |

### The chapter's check

- **Source:** [`coax/lib/agent/router/wire.coax`](../../coax/lib/agent/router/wire.coax),
  [`coax/lib/agent/router/policy.coax`](../../coax/lib/agent/router/policy.coax),
  [`coax/lib/agent/router/decide.coax`](../../coax/lib/agent/router/decide.coax),
  [`openagents/examples/typed-route-plan.coax`](../examples/typed-route-plan.coax),
  and [`openagents/examples/typed-route-plan.out`](../examples/typed-route-plan.out)
  (remaining: multi-step grant-carrying executor in `openagents/agent/plan.coax`).
- **Trace:** constructs a typed `Rtr2Plan` for `"What did we decide in episode 289?"`
  (`pack=knowledge`, `tier=local_inkling`, `delegate=none`, `effort=low`,
  `difficulty=1`), round-trips it through `rtr2_plan_write` and `rtr2_plan_read`
  with byte-identical replay (`replay_exact=true`), and selects a deterministic
  offline fallback plan (`door=fallback`, `pack=coder`, `tier=local_inkling`,
  `reasons=door_unavailable,user_pref_local`) when `user_compute_pref="local"`.
- **Refusal:** an empty task fails closed with `no_task`; missing compute tiers
  fail closed with `no_compute`; a door safety block fails closed with
  `refused_by_door`; an unopted-in external delegate (`allow_local_harnesses=false`)
  refuses delegation (`none/delegate_unavailable`); and an unknown pack in JSON
  is refused by `rtr2_plan_request_read`.
- **Independent check:** `coax/bin/coax run openagents/examples/typed-route-plan.coax`
  and `--release` match [`openagents/examples/typed-route-plan.out`](../examples/typed-route-plan.out),
  alongside `#authority` mutation checks and `#forall` fuzzing across 512 readiness
  and preference combinations in [`coax/lib/agent/router/policy.coax`](../../coax/lib/agent/router/policy.coax)
  and [`coax/lib/agent/router/decide.coax`](../../coax/lib/agent/router/decide.coax).

Planned paths: `openagents/agent/plan.coax`.

## Sources

- **Episodes:**
  - [Episode 114, *Planning & Execution*](../../docs/transcripts/114.md): planning and execution as separate steps
  - [Episode 157, *Chains of Thought and Action*](../../docs/transcripts/157.md)
  - [Episode 286, *System One in Coding Agents*](../../docs/transcripts/286.md): explicit state in place of a growing chat
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:00, messages mapped to routes
- **Old code at `coder-v1.0.0`, crates:** `route-contract`, `program-launch`, `workflow-run`
- **NIPs:** [NIP-CJ](../../nostr/nips/openagents/NIP-CJ.md), [NIP-PRG](../../nostr/nips/openagents/NIP-PRG.md), [NIP-POL](../../nostr/nips/openagents/NIP-POL.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P7

## Open questions for the owner

1. The audit's plan carries a budget. Our rule is that we impose no limits on
   a run and show any limit the user sets. Should a plan carry only limits the
   user set?

## Source listing

Run the Chapter 14 example in [`openagents/examples/typed-route-plan.coax`](../examples/typed-route-plan.coax) (committed transcript [`openagents/examples/typed-route-plan.out`](../examples/typed-route-plan.out)):

<!-- source: openagents/examples/typed-route-plan.coax typed-route-plan -->
```jai
trp_run :: () -> bool {
  q: [1] Rtr2PlanRequest;
  p: [1] Rtr2Plan;
  w: [1] Rtr2Work;

  // 1. Typed RoutePlan for a knowledge question + byte-identical JSON replay.
  q[0] = Rtr2PlanRequest.{
    text = "What did we decide in episode 289?",
    local_inkling_ready = true,
    cloud_ready = true
  };
  rtr2_decide_mock(
    q[..],
    "primary_pack=knowledge,compute_tier=local_inkling,explicit_delegate=none,difficulty=1,effort=low,missing_capability=50,missing_capability_kind=none",
    0, p[..], w[..]
  );
  j1: [RTR2_PLAN_MAX] u8;
  n1 := rtr2_plan_write(p[..], j1[..]);
  s1 := cast(string) j1[0..<n1];

  nodes: [1024] JvNode;
  wire: [RTR2_PLAN_MAX] u8;
  p2: [1] Rtr2Plan;
  rd_ok := rtr2_plan_read(s1, nodes[..], wire[..], p2[..]);
  j2: [RTR2_PLAN_MAX] u8;
  n2 := rtr2_plan_write(p2[..], j2[..]);
  replay_exact := rd_ok && n1 > 0 && n1 == n2 && s1 == cast(string) j2[0..<n2];
  print("plan1 ok=% pack=% tier=% delegate=% effort=% difficulty=% replay_exact=%\n",
    p[0].ok, p[0].capability_packs[0], p[0].compute_tier, p[0].explicit_delegate,
    p[0].effort, p[0].difficulty, replay_exact);

  // 2. Deterministic offline fallback + user_compute_pref="local" override.
  q[0] = Rtr2PlanRequest.{
    text = "fix the bug in app.coax",
    repo = "openagents",
    local_inkling_ready = true,
    cloud_ready = true,
    user_compute_pref = "local"
  };
  rtr2_decide_offline(q[..], p[..]);
  print("offline ok=% door=% pack=% tier=% reasons=%,%\n",
    p[0].ok, p[0].door, p[0].capability_packs[0], p[0].compute_tier,
    p[0].reasons[0], p[0].reasons[1]);

  // 3. Fail-closed refusals: empty task, no compute, safety block, unopted delegate, bad JSON pack.
  q_empty: [1] Rtr2PlanRequest;
  q_empty[0] = Rtr2PlanRequest.{local_inkling_ready = true, cloud_ready = true};
  p_empty: [1] Rtr2Plan;
  rtr2_decide_mock(q_empty[..], "", 0, p_empty[..], w[..]);

  q_nocpu: [1] Rtr2PlanRequest;
  q_nocpu[0] = Rtr2PlanRequest.{text = "hi"};
  p_nocpu: [1] Rtr2Plan;
  rtr2_decide_mock(q_nocpu[..], "", 0, p_nocpu[..], w[..]);

  q_safe: [1] Rtr2PlanRequest;
  q_safe[0] = Rtr2PlanRequest.{text = "blocked input", local_inkling_ready = true, cloud_ready = true};
  p_safe: [1] Rtr2Plan;
  rtr2_decide_mock(q_safe[..], "fail=safety", 0, p_safe[..], w[..]);

  q_del: [1] Rtr2PlanRequest;
  q_del[0] = Rtr2PlanRequest.{
    text = "use claude to fix this",
    local_inkling_ready = true,
    cloud_ready = true,
    allow_local_harnesses = false
  };
  q_del[0].available_delegates[0] = "claude";
  q_del[0].navailable_delegates = 1;
  p_del: [1] Rtr2Plan;
  rtr2_decide_mock(q_del[..], "primary_pack=coder,explicit_delegate=claude,difficulty=2,effort=medium", 0, p_del[..], w[..]);

  q_bad: [1] Rtr2PlanRequest;
  bad_pack_ok := rtr2_plan_request_read("{\"text\":\"hi\",\"available_packs\":[\"bogus\"]}", nodes[..], wire[..], q_bad[..]);

  print("refuse_empty=% refuse_compute=% refuse_safety=% refuse_delegate=%/% refuse_bad_pack=%\n",
    p_empty[0].error, p_nocpu[0].error, p_safe[0].error,
    p_del[0].explicit_delegate, p_del[0].reasons[0], !bad_pack_ok);

  return replay_exact && p[0].ok && p[0].capability_packs[0] == "coder" && p[0].compute_tier == "local_inkling" &&
         !p_empty[0].ok && p_empty[0].error == "no_task" &&
         !p_nocpu[0].ok && p_nocpu[0].error == "no_compute" &&
         !p_safe[0].ok && p_safe[0].error == "refused_by_door" &&
         p_del[0].ok && p_del[0].explicit_delegate == "none" && p_del[0].reasons[0] == "delegate_unavailable" &&
         !bad_pack_ok;
}
```

Output (`coax/bin/coax run openagents/examples/typed-route-plan.coax`, identical in `--release`):

```text
plan1 ok=true pack=knowledge tier=local_inkling delegate=none effort=low difficulty=1 replay_exact=true
offline ok=true door=fallback pack=coder tier=local_inkling reasons=door_unavailable,user_pref_local
refuse_empty=no_task refuse_compute=no_compute refuse_safety=refused_by_door refuse_delegate=none/delegate_unavailable refuse_bad_pack=true
ok=true
```
