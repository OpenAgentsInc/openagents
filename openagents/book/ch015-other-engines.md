# 15. Other engines

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Coder is one member. This chapter lets you name another: "do it with Codex"
sends the same task to a different engine, under the same grant and the same
record.

## Draft

[Episode 289](../../docs/transcripts/289.md) shows the phrase on screen: "Do
it with Codex." The front reads that you named an engine and honors it. Coder
then drives that engine and does not use its own loop.

The reason is neutrality. [Episode 278](../../docs/transcripts/278.md) argues
that an agent tied to no single provider should coordinate the competing model
families. The archive shows why. It integrates outside coding agents,
criticizes them, replaces them and returns to them, and [Episode
204](../../docs/transcripts/204.md) records what happens when a provider
changes its rules under a workflow you depend on. Your work should not belong
to one supplier.

It is also practical. Many people already pay for a coding agent. Driving the
one you have costs you nothing extra, and it lets you compare engines on your
own tasks.

Engines behave differently. They have different tools, prices, limits and ways
of failing. So the choice follows a policy you can read: the engine you named,
then what the computer's owner allows, then which engine has capacity. If an
engine is at its limit, the next one takes over, and the log records which
engine ran.

The old tree delegated to five engines. Today's Coax tree drives Claude Code,
Codex and Cursor, alongside our own Microcoder. Chapter 16 gives each task a
model tier.

## What we build

A plan for what this chapter ships once the draft is approved:

- An engine field in the plan, filled when the person names one.
- A policy for choosing and failing over between engines, with the choice
  logged.
- A check that every engine's result comes back in the same shape as Coder's
  own.

It builds on what exists today:

- [`coax/lib/agent/claude`](../../coax/lib/agent/claude)
- [`coax/lib/agent/codex`](../../coax/lib/agent/codex)
- [`coax/lib/agent/cursor`](../../coax/lib/agent/cursor)
- [`coax/lib/agent/tier.coax`](../../coax/lib/agent/tier.coax)
- [`coder/tui/availability-policy.coax`](../../coder/tui/availability-policy.coax)

Planned paths: `openagents/agent/members/engines.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:00, "Do it with Codex"
  - [Episode 278, *Coder Commands Codex & Claude*](../../docs/transcripts/278.md): commanding Codex and Claude
  - [Episode 185, *Hello Claude Code*](../../docs/transcripts/185.md): first Claude Code integration
  - [Episode 193, *Codex & Claude Code On Your Phone*](../../docs/transcripts/193.md): Codex and Claude Code from the phone
  - [Episode 204, *DO NOT BREAK USERSPACE*](../../docs/transcripts/204.md)
  - [Episode 243, *Khala in OpenCode*](../../docs/transcripts/243.md): orchestration inside OpenCode
  - [Episode 244, *Codex in Khala*](../../docs/transcripts/244.md): Codex inside the orchestrator
- **Old code at `coder-v1.0.0`, crates:** `coder-delegate`, `codex-transport`, `claude_agent_sdk`, `acp-client`, `coder-engine-status`
- **NIPs:** [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md), [NIP-WORK](../../nostr/nips/openagents/NIP-WORK.md), [NIP-PRG](../../nostr/nips/openagents/NIP-PRG.md)
- **Also:**
  - Coder book chapters [10](../../coder/book/ch010-drive-claude-code.md), [11](../../coder/book/ch011-drive-codex.md), [13](../../coder/book/ch013-choose-an-engine.md) and [38](../../coder/book/ch038-drive-cursor.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P3

## Open questions for the owner

1. Which engines does 1.0 name? The Coax tree drives Claude Code, Codex and
   Cursor. The old tree also drove Grok Build, OpenCode and Devin.
