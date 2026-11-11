# 4. A typed plan

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

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
Coder. Some are several steps, and chapter 17 turns those into programs.

## What we build

A plan for what this chapter ships once the draft is approved:

- A plan type: steps, the member for each step, its grant, and limits.
- An executor that runs a plan and writes each step to the log.
- Coax checks for the plan's rules, and refusals with readable reasons.

It builds on what exists today:

- [`coax/lib/nostr/cj`](../../coax/lib/nostr/cj)
- [`coax/lib/nostr/pol`](../../coax/lib/nostr/pol)
- [`coax/lib/bounded.coax`](../../coax/lib/bounded.coax)

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
