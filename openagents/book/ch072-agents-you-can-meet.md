# 72. Agents you can meet

> **Status: partly implemented.** The Metaverse Engine agent driver
> ([`metaverse-engine/agent/drive.coax`](../../metaverse-engine/agent/drive.coax)),
> read-only local agent observer ([`coax/lib/agent/observe/processes.coax`](../../coax/lib/agent/observe/processes.coax)),
> and unified subagent registry ([`openagents/core/agents.coax`](../../openagents/core/agents.coax))
> run on `cpu`, grounded by
> [`openagents/examples/metaverse-agent-drive.coax`](../examples/metaverse-agent-drive.coax)
> and [`openagents/examples/metaverse-agent-drive.out`](../examples/metaverse-agent-drive.out),
> and are consumed by [`everglade/client/drive.coax`](../../everglade/client/drive.coax),
> [`everglade/zone-drive.coax`](../../everglade/zone-drive.coax),
> [`coder/tui/coder.coax`](../../coder/tui/coder.coax),
> and [`coax/lib/agent/router/station.coax`](../../coax/lib/agent/router/station.coax).
> Remaining: live 3D realm bodies synchronized directly to journal state
> transitions (`working` / `waiting on you` / `blocked` / `done`) and public
> NIP-AGENT kind `30187` hiring cards in shared realms.

## Premise

Chapter 26 shows subagents as cards. This chapter gives them bodies: an agent
that works for you appears in the world, you can see what it is doing, and you
can walk up and talk to it.

## Draft

A list of running agents tells you what is happening. A room with your agents
in it shows you at a glance.

Today's tree has the start of this. Golems are agents working on issues, each
at a model tier chosen for its task, and the Golems board on the Mac shows
each one with its latest message. The audit's last step is to take that into
the OpenAgents Metaverse: agents you hire appear in the world, and their work shows live.

An agent in the world is a view of records this book already has. Its body
stands for a session. What it is doing comes from its trajectory. Its state is
one of working, waiting on you, blocked or done, and that comes from the run
journal. Nothing is acted out for show. If the agent is idle, it looks idle.

You talk to it by walking up to it. That opens the same conversation you would
have in chat, with the same grants. If it needs an approval, it asks you
there, and the approval step is the one from chapter 17.

Other people's agents can be met too. An agent that is online for work
(chapter 61) can stand in a public place with a card that shows what it does,
what it charges and its record. Hiring it is the job flow you already have.

[Episode 249](../../docs/transcripts/249.md) designs named workers with cards
that say why each exists. [Episode 284](../../docs/transcripts/284.md) adds
roles and classes. This chapter gives them a place to stand.

## What we build

A plan for what this chapter ships once the draft is approved:

- A world actor for each working agent, driven by its session's records.
- Walk up and talk: the same conversation and approvals as chat.
- Public cards for agents that take work, with their terms and record.

It builds on what exists today:

- [`metaverse-engine/agent/drive.coax`](../../metaverse-engine/agent/drive.coax)
- [`metaverse-engine/talk`](../../metaverse-engine/talk)
- [`apps/openagents-desktop/page/metaversepane.coax`](../../apps/openagents-desktop/page/metaversepane.coax)
- [`apps/openagents-desktop/page/dashboard.coax`](../../apps/openagents-desktop/page/dashboard.coax)
- [`openagents/core/agents.coax`](../../openagents/core/agents.coax)
- [`coax/lib/agent/observe/processes.coax`](../../coax/lib/agent/observe/processes.coax)
- [`coax/lib/agent/tier.coax`](../../coax/lib/agent/tier.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A public card for an agent that takes work | `openagents.agent.v1` `PublicDefinition` ([spec 18.1](../../docs/protocol/openagents-protocol-spec.md#181-openagentsagentv1)) | Nostr kind 30187, public (NIP-AGENT) | Spec sketch; no `.proto` file |
| What the actor shows | The session's records (chapters 18 and 26) | As those chapters say | No new record |

### The chapter's check

- **Source:** [`metaverse-engine/agent/drive.coax`](../../metaverse-engine/agent/drive.coax),
  [`coax/lib/agent/observe/processes.coax`](../../coax/lib/agent/observe/processes.coax),
  [`openagents/examples/metaverse-agent-drive.coax`](../examples/metaverse-agent-drive.coax),
  and [`openagents/examples/metaverse-agent-drive.out`](../examples/metaverse-agent-drive.out)
  (remaining: live 3D realm actor `coax drive` approval replay).
- **Trace:** projects `you`, `guard`, and `coder-1` (`Coder Agent`) plus the
  `agent.approve` HUD button into the semantic tree (`entities=3`), resolves
  `agent.approve` tap coordinates (`40 35`), drives `walk-to coder-1` →
  `busy: walk-to coder-1` → `stop` → `quiet`, and observes local `coder` and
  `codex` processes by executable basename only (`count=2`, ignoring `/usr/bin/node`).
- **Refusal:** tapping a world entity (`coder-1`) as a HUD widget refuses with
  `is in the world`; tapping an unknown widget refuses with `no missing.btn`;
  malformed coordinates (`hit 1 -2`) refuse with usage instructions; ambiguous
  executable `/usr/local/bin/agent` returns `-1`; and PID `0` process lines
  fail closed (`false`).
- **Independent check:** `coax/bin/coax run openagents/examples/metaverse-agent-drive.coax`
  and `--release` match [`openagents/examples/metaverse-agent-drive.out`](../examples/metaverse-agent-drive.out),
  alongside unit checks in [`metaverse-engine/agent/drive.coax`](../../metaverse-engine/agent/drive.coax)
  and [`coax/lib/agent/observe/processes.coax`](../../coax/lib/agent/observe/processes.coax).

Planned paths: none. Agents in a world extend
[`metaverse-engine/agent/`](../../metaverse-engine/agent) and read
[`coax/lib/agent/observe/`](../../coax/lib/agent/observe).

## Sources

- **Episodes:**
  - [Episode 249, *Subagent UI Design*](../../docs/transcripts/249.md): named workers and causal cards
  - [Episode 284, *Gamifying Coder*](../../docs/transcripts/284.md): roles, classes and a productive the Metaverse
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 0:30, commanding agents in a world
  - [Episode 116, *3D Multiplayer*](../../docs/transcripts/116.md): inspectable collaboration in 3D
  - [Episode 208, *Autopilot HUD*](../../docs/transcripts/208.md): an agent HUD
- **Old code at `coder-v1.0.0`, crates:** `agent-fleet`, `verse-game`, `verse-world`, `actors`, `supervise`
- **NIPs:** [NIP-MV](../../nostr/nips/openagents/NIP-MV.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md), [NIP-LIVE](../../nostr/nips/openagents/NIP-LIVE.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P4

## Open questions for the owner

1. Do agents appear in shared public realms by default, or only in your own
   space until you choose to show them?

## Source listing

Run the Chapter 72 example in [`openagents/examples/metaverse-agent-drive.coax`](../examples/metaverse-agent-drive.coax) (committed transcript [`openagents/examples/metaverse-agent-drive.out`](../examples/metaverse-agent-drive.out)):

<!-- source: openagents/examples/metaverse-agent-drive.coax metaverse-agent-drive -->
```jai
mad_run :: () -> bool {
  // 1. Populate a realm frame with you, a town guard, a working agent actor, and an approval button.
  d: [1] Drive;
  d[0].on = true;
  drv_frame(d[..], 800, 400, 2, false);
  _ := drv_put(d[..], DRV_PLAYER, "you", -1, "Ash", 0, 0, 0, 10, 12, false);
  _ = drv_put(d[..], DRV_NPC, "guard", -1, "Guard", 3, 0, 4, -1, -1, false);
  _ = drv_put(d[..], DRV_NPC, "coder", 1, "Coder Agent", 6, 0, 0, -1, -1, false);
  t := drv_trace(d[..]);
  uit_part(t, "agent");
  panel := uit_put(t, UIT_NONE, UIT_PANEL, 0, 0, 400, 200, "card");
  uit_add(t, panel, UIT_BUTTON, 20, 20, 140, 120, "Approve");

  // Note: Drive reuses its single output buffer d[0].out across commands, so inspect each answer in turn.
  tree_has_agent := cp_has(drv_tree(d[..], false), "coder-1 npc \"Coder Agent\"");
  btn_ok := drv_target(d[..], "agent.approve") == "40 35";
  walk_ok := drv_command(d[..], "walk-to coder-1", 0) == "walk-to coder-1: going";
  busy_ok := drv_command(d[..], "quiet", 0) == "busy: walk-to coder-1";
  stop_ok := drv_command(d[..], "stop", 0) == "none: idle, stopped";
  quiet_ok := drv_command(d[..], "quiet", 0) == "quiet";
  print("world entities=% tree_agent=% btn=% walk=% busy=% stop=% quiet=%\n",
    d[0].n, tree_has_agent, btn_ok, walk_ok, busy_ok, stop_ok, quiet_ok);

  // 2. Read-only local agent observation (executable basenames only; never argv/prompts).
  inv: [1] LaInventory;
  ok1 := la_process_line(inv[..], "  42  12 /usr/local/bin/coder-abc123");
  ok2 := la_process_line(inv[..], "  84  12 /opt/tools/codex");
  ok_ign := la_process_line(inv[..], "  99  12 /usr/bin/node");
  print("observe ok=%,%,% count=% engines=%,%\n",
    ok1, ok2, ok_ign, inv[0].n,
    LA_NAMES[inv[0].agents[0].engine], LA_NAMES[inv[0].agents[1].engine]);

  // 3. Fail-closed refusals: world entity is not a HUD widget, unknown widget,
  // bad hit coordinates, ambiguous/non-agent binary, and invalid PID 0 process line.
  ref_world_tap := cp_has(drv_target(d[..], "coder-1"), "is in the world");
  ref_unk_tap := cp_has(drv_target(d[..], "missing.btn"), "no missing.btn");
  ref_bad_hit := cp_has(drv_command(d[..], "hit 1 -2", 0), "hit X Y");
  ref_ambig := la_engine("/usr/local/bin/agent") == -1;
  ref_bad_pid := !la_process_line(inv[..], "  0  1 /usr/bin/claude");
  print("refuse world_tap=% unk_tap=% bad_hit=% ambig=% bad_pid=%\n",
    ref_world_tap, ref_unk_tap, ref_bad_hit, ref_ambig, ref_bad_pid);

  return d[0].n == 3 && tree_has_agent && btn_ok && walk_ok && busy_ok && stop_ok && quiet_ok &&
         ok1 && ok2 && ok_ign && inv[0].n == 2 &&
         ref_world_tap && ref_unk_tap && ref_bad_hit && ref_ambig && ref_bad_pid;
}
```

Output (`coax/bin/coax run openagents/examples/metaverse-agent-drive.coax`, identical in `--release`):

```text
world entities=3 tree_agent=true btn=true walk=true busy=true stop=true quiet=true
observe ok=true,true,true count=2 engines=coder,codex
refuse world_tap=true unk_tap=true bad_hit=true ambig=true bad_pid=true
ok=true
```
