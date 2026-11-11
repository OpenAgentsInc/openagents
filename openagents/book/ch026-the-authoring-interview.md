# 26. The authoring interview

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 25 writes a plugin by hand. This chapter lets a person who can't write
code get one by answering questions: the agent interviews you, drafts the
plugin and its tests, and runs them.

## Draft

[Episode 289](../../docs/transcripts/289.md) shows the exchange. You ask:
"Turn my meeting notes into Linear tickets." The agent answers that we can't
do that yet, and asks whether you want to build it as a plugin. You say yes.
It asks what the plugin should do and what it should not do. You say: one
ticket per action. It drafts tests and asks whether they look right. You say
they do. It runs them.

This is promise P9, and it changes who can extend the agent. The front already
knows when it has no member for a request. That moment is where a missing
capability is found. Most products apologize there. This one offers to build.

The interview is a program (chapter 17) that Coder runs. Its questions are
chosen to produce a test set. "What should it do?" becomes cases where the
plugin must act. "What shouldn't it do?" becomes cases where it must stay
quiet. Your answers are the specification, and the tests are your answers made
exact.

The output is a package: the plugin, its descriptor, the grant it needs and
its test set. The agent shows you the tests before it runs anything, because a
plugin that passes tests you didn't mean is worse than none.

If the tests pass, you have a working plugin for yourself. If you want
everyone to have it, the package goes on to Part V and then to the registry in
Part VI.

The agent has improved itself from chat before. [Episode
87](../../docs/transcripts/087.md) is a first upgrade, and in [Episode
192](../../docs/transcripts/192.md) OpenAgents changes its own code.

## What we build

A plan for what this chapter ships once the draft is approved:

- The offer: when no member fits, the front proposes building one.
- The interview program: questions, a drafted test set you approve, a drafted
  plugin, and a run.
- A package at the end that Part V can measure without changes.

It builds on what exists today:

- [`coder/tui/coder.coax`](../../coder/tui/coder.coax)
- [`coax/lib/nostr/cj`](../../coax/lib/nostr/cj)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/vault.coax`](../../coax/lib/nostr/vault.coax)

Planned paths: `openagents/eval/author/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:30, building a missing plugin from chat
  - [Episode 87, *Meta-Agent's First Upgrade*](../../docs/transcripts/087.md): an agent's first upgrade
  - [Episode 192, *OpenAgents Upgrades Itself*](../../docs/transcripts/192.md)
  - [Episode 245, *Khala Code*](../../docs/transcripts/245.md): plugins derived from real work
- **Old code at `coder-v1.0.0`, crates:** `ext-eval`, `openagents-chat`, `brainstorm-client`, `plugin-action-items`
- **NIPs:** [NIP-CJ](../../nostr/nips/openagents/NIP-CJ.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-VAULT](../../nostr/nips/openagents/NIP-VAULT.md)
- **Also:**
  - `plugins/meeting-action-items` and `plugins/brainstorm` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P9

## Open questions for the owner

1. Who pays for the compute an authoring run uses: the person, us for a first
   run, or the first paid use of the plugin?
2. A plugin that files tickets needs the person's login to another service.
   Does 1.0 include a vault for such logins, or do first plugins avoid them?
