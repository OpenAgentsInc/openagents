# 28. The command line behind the agent

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Members so far were written as members. This chapter turns something that
already exists into members: every command in the `openagents` command line
becomes something the front can call.

## Draft

[Episode 289](../../docs/transcripts/289.md): "Our whole CLI, everything,
everything, everything behind this single agent, accessible through anything."
You say "show the Test-Time Capabilities deck", and a deck opens. This is
promise P8.

The command line is the most complete description of what OpenAgents can do.
It has commands for your identity, relays, realms, projects, models, Pylons,
skills, a vault and Coder. If the agent can call each one, then everything the
product does can be reached from a sentence on any surface.

Each command gets a member descriptor: its name, what it does, its typed
arguments and its effects. The descriptors are generated from the command
table itself. The command line and the agent can't drift apart, because they
read the same table.

The front then treats "show the deck" like any other route. The decision picks
the member, and the plan carries the arguments. A deterministic parser reads
bounded fields such as an amount, an ID or a date, and it runs only after the
route is chosen.

Effects still need grants. A command that only shows something runs at once. A
command that changes something asks first. A few commands should never be
reachable from chat at all, such as exporting a key.

The same descriptors serve other agents. The command line already speaks MCP,
so a person's own agent can call the same commands.

## What we build

A plan for what this chapter ships once the draft is approved:

- Member descriptors generated from the command line's command table.
- Routes for the first commands, starting with opening a deck and showing a
  project.
- A deny list for commands that chat can never call.

It builds on what exists today:

- [`apps/openagents-cli/commands.coax`](../../apps/openagents-cli/commands.coax)
- [`apps/openagents-cli/help-generated.coax`](../../apps/openagents-cli/help-generated.coax)
- [`apps/openagents-cli/mcp.coax`](../../apps/openagents-cli/mcp.coax)
- [`apps/openagents-cli/rules.coax`](../../apps/openagents-cli/rules.coax)
- [`docs/cli/2026-11-01-openagents-cli-v1.md`](../../docs/cli/2026-11-01-openagents-cli-v1.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A command as a member | `openagents.capability.v1` `Definition` ([spec 19.3](../../docs/protocol/openagents-protocol-spec.md#193-openagentscapabilityv1)), generated from the command table | Nostr kind 30180 when published; MCP for other agents | Spec sketch; no `.proto` file. MCP is an external format |
| A command's `--json` output | None yet ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step C8) | Standard output | Hand-written JSON |

### The chapter's check

- **Source:** the generated descriptors in
  `openagents/agent/members/cli.coax`, read from
  `apps/openagents-cli/commands.coax`, with a fake decision service.
- **Trace:** "show the Test-Time Capabilities deck" routes to the deck
  member, the parser reads the deck's name, and `state` shows that deck
  open. The trace prints one descriptor for each command in the table.
- **Refusal:** "export my key" hits the deny list and prints `refused: key
  export can't run from chat`. A command that changes something waits for
  approval and does not run before it.
- **Independent check:** the descriptor names match the commands that
  `apps/openagents-cli/mcp.coax` lists over MCP and the commands in
  `apps/openagents-cli/help-generated.coax`.

Planned paths: `openagents/agent/members/cli.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:00 to 1:30, the whole CLI and the deck
  - [Episode 170, *Commander*](../../docs/transcripts/170.md)
  - [Episode 165, *Integrating MCP via AI SDK*](../../docs/transcripts/165.md): tools through MCP
  - [Episode 168, *Remote MCP Server & Client*](../../docs/transcripts/168.md): a remote MCP server and client
- **Old code at `coder-v1.0.0`, crates:** `openagents-cli`, `openagents-deck`, `openagents-terminal`, `coder-control`
- **NIPs:** [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md), [NIP-CTRL](../../nostr/nips/openagents/NIP-CTRL.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P8

## Open questions for the owner

1. Which commands can chat never call? Proposed: key export, admin commands,
   and anything that deletes data.
