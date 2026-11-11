# 25. Write a plugin

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The interface, the sandbox and the grants exist. This chapter writes a plugin
from start to finish, and then ports the nine that shipped in the old tree as
worked examples.

## Draft

You write a plugin in four steps. You say what it does: one operation, with
typed input and output. You say what it needs: its effects and the grant it
will ask for. You write the code. And you write its tests: cases where it
should act, and cases where it should stay quiet.

The first example is small. It takes meeting notes and returns the action
items in them, one per action. It needs no grant, because it only reads the
text you give it. The old tree shipped it, and Episode 289's demo builds its
cousin, the one that files tickets.

The old tree shipped nine plugins, and this chapter ports them as the book's
examples:

- a map of a repository;
- code search;
- a test report;
- a dependency check;
- an explanation of an error;
- a hook that guards tool calls;
- an outline;
- release notes;
- action items.

They range from pure text work to plugins that read a repository under a
grant. One of them matters more than the rest. The repository map is the
plugin that went from a claim to adoption on 2026-09-29, and Part V measures
it again.

The earliest plugins in the archive were small as well: a URL extractor in
[Episode 68](../../docs/transcripts/068.md), a scraper in [Episode
69](../../docs/transcripts/069.md), and key-value storage on Nostr in [Episode
88](../../docs/transcripts/088.md).

A plugin you write here is a candidate. It works, and you can use it yourself.
Whether everyone should get it is a question for Part V.

## What we build

A plan for what this chapter ships once the draft is approved:

- A walk-through plugin: action items from meeting notes, with its tests.
- Ports of the nine old plugins, each with its descriptor, grant and tests.
- A `plugin new` command that makes the skeleton. This is a plan.

It builds on what exists today:

- [`coder/plugins/hello`](../../coder/plugins/hello)
- [`coder/tui/plugins.coax`](../../coder/tui/plugins.coax)
- [`coax/lib/text`](../../coax/lib/text)
- [`coax/lib/json`](../../coax/lib/json)

Planned paths: `openagents/plugins/examples/`.

## Sources

- **Episodes:**
  - [Episode 54, *Uploading a Plugin*](../../docs/transcripts/054.md)
  - [Episode 68, *URL Extractor Plugin*](../../docs/transcripts/068.md): a URL extractor
  - [Episode 69, *URL Scraper Plugin*](../../docs/transcripts/069.md): a scraper
  - [Episode 88, *Nostr KV Storage Plugin*](../../docs/transcripts/088.md): key-value storage on Nostr
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:30, one ticket per action
- **Old code at `coder-v1.0.0`, crates:** `plugin-pdk`, `plugin-repo-map`, `plugin-code-search`, `plugin-test-report`, `plugin-dependency-check`, `plugin-explain-error`, `plugin-hook-guard`, `plugin-outline`, `plugin-release-notes`, `plugin-action-items`
- **NIPs:** [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-SKILL](../../nostr/nips/openagents/NIP-SKILL.md)
- **Also:**
  - `plugins/meeting-action-items` and `plugins/skills` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P5

## Open questions for the owner

1. Which of the nine do we port first after the repository map?
