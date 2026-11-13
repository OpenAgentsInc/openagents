# 42. The authoring interview

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 41 writes a plugin by hand. This chapter lets a person who can't write
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

The interview is a program (chapter 27) that Coder runs. Its questions are
chosen to produce a test set. "What should it do?" becomes cases where the
plugin must act. "What shouldn't it do?" becomes cases where it must stay
quiet. Your answers are the specification, and the tests are your answers made
exact.

The output is a package: the plugin, its descriptor, the grant it needs and
its test set. The agent shows you the tests before it runs anything, because a
plugin that passes tests you didn't mean is worse than none.

If the tests pass, you have a working plugin for yourself. If you want
everyone to have it, the package goes on to the evidence chapters (43 to 52) and
then to the registry in chapter 54.

The agent has improved itself from chat before. [Episode
87](../../docs/transcripts/087.md) is a first upgrade, and in [Episode
192](../../docs/transcripts/192.md) OpenAgents changes its own code.

## What we build

A plan for what this chapter ships once the draft is approved:

- The offer: when no member fits, the front proposes building one.
- The interview program: questions, a drafted test set you approve, a drafted
  plugin, and a run.
- A package at the end that chapter 45 can measure without changes.

It builds on what exists today:

- [`coder/tui/coder.coax`](../../coder/tui/coder.coax)
- [`coax/lib/nostr/cj`](../../coax/lib/nostr/cj)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/vault.coax`](../../coax/lib/nostr/vault.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| The drafted test set and plugin | `openagents.eval.v1` `EvalDraft` ([spec 21.1](../../docs/protocol/openagents-protocol-spec.md#211-openagentsevalv1)) | In the conversation result (chapter 19) | Spec sketch; no `.proto` file |
| A login to another service | `openagents.vault.v1` `VaultObject` ([spec 15.3](../../docs/protocol/openagents-protocol-spec.md#153-openagentsvaultv1)) | Nostr: ciphertext in a kind 3188 envelope (NIP-VAULT) | Spec sketch; no `.proto` file. `coax/lib/nostr/vault.coax` builds the `user` tier |

### The chapter's check

- **Source:** the interview program in `openagents/eval/author/` and the
  offer the front makes when no member fits.
- **Trace:** on a fake model with recorded answers, `type composer "Turn my
  meeting notes into Linear tickets."`, then `tap composer.send`. `tree`
  shows the offer to build a plugin. The case answers each question, and
  `state` prints the drafted test set: should-fire cases from "one ticket per
  action" and should-not-fire cases from what it shouldn't do.
- **Refusal:** with the test set not yet approved, `tap interview.run`
  answers `interview.run can't be pressed now`, and nothing runs.
- **Independent check:** the package the interview writes is measured by
  the two-arm runner of chapter 45 with no changes, and its cases match a
  test set written by hand from the same answers.

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
