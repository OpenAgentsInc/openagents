# 38. The plugin interface

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Every member so far was written by us. This chapter defines the interface that
lets anyone add one: what a plugin declares, what it may register, and how it
is packaged.

## Draft

Episode 289 says of the member list: "This is all going to be added to, first
by us and then by you." A plugin is how you add to it.

This is the oldest network argument in the archive. [Episode
48](../../docs/transcripts/048.md) plans a plugin system so that community
developers can supply agent logic and integrations. [Episode
58](../../docs/transcripts/058.md) has an agent call one. The reusable unit
moved from a whole agent to a piece another agent can use.

A plugin declares its operations. Each has a name, a summary, typed input and
output, its effects, and how it can be tested. That is the same descriptor
every member has, so the front treats a plugin's operation like any other
route.

Everything a plugin registers belongs to it. When the plugin is turned off,
its operations, hooks and commands go with it. Its state is always visible:
installed, enabled, admitted, active, failed or stopping. The Coder book
builds this ownership model for a coding session, and this chapter widens it
to the whole agent.

A package can carry four kinds of contribution: a plugin with code, a program,
a skill, which is written instructions the agent loads when needed, and
knowledge entries. A package can also carry the test set that measures it.

Episode 289 says where the shape came from: we adapted it from Claude Code's
plugins and its way of helping you write evals. People already write plugins
and skills in that form, so reading those formats is worth doing.

## What we build

A plan for what this chapter ships once the draft is approved:

- A plugin manifest and operation descriptor, shared with chapter 13's member
  descriptor.
- Owned registration: turning a plugin off removes everything it registered.
- A package format that carries plugins, programs, skills, knowledge entries
  and test sets.

It builds on what exists today:

- [`coder/tui/plugins.coax`](../../coder/tui/plugins.coax)
- [`coder/plugins/hello`](../../coder/plugins/hello)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/nostr/cap`](../../coax/lib/nostr/cap)
- [`apps/openagents-cli/skill.coax`](../../apps/openagents-cli/skill.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Manifest and operation descriptor | `openagents.extension.v1` `Manifest`, `OperationDescriptor` ([spec 20.2](../../docs/protocol/openagents-protocol-spec.md#202-openagentsextensionv1)) | Inside a release event (chapter 54) | Spec sketch; no `.proto` file. `coax/lib/nostr/ext` handles the NIP's JSON. Coder's plugin manifests are plain files or signed NIP-78 events today |
| Skill listing | `openagents.skill.v1` `Listing` ([spec 20.3](../../docs/protocol/openagents-protocol-spec.md#203-openagentsskillv1)) | Nostr kind 30188 (NIP-SKILL) | Spec sketch; no `.proto` file |

Planned paths: `openagents/plugins/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, plugins adapted from Claude Code
  - [Episode 48, *Brainstorming a Plugin System*](../../docs/transcripts/048.md): planning a plugin system
  - [Episode 53, *Loading WASM Plugins*](../../docs/transcripts/053.md)
  - [Episode 58, *Agent Uses Plugin*](../../docs/transcripts/058.md): an agent uses a plugin
  - [Episode 102, *Agent Plugins UI*](../../docs/transcripts/102.md): a plugins UI
  - [Episode 165, *Integrating MCP via AI SDK*](../../docs/transcripts/165.md): a tool marketplace tied to evaluation
- **Old code at `coder-v1.0.0`, crates:** `plugin`, `plugin-pdk`, `capability`
- **NIPs:** [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-SKILL](../../nostr/nips/openagents/NIP-SKILL.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md), [NIP-PRG](../../nostr/nips/openagents/NIP-PRG.md), [NIP-KB](../../nostr/nips/openagents/NIP-KB.md)
- **Also:**
  - [Coder book, chapter 15: Plugins with owned lifecycles](../../coder/book/ch015-plugins-with-owned-lifecycles.md)
  - `plugins/` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P4, P5

## Open questions for the owner

1. Do we read Claude Code's plugin and skill formats directly, or only our own
   package format?
