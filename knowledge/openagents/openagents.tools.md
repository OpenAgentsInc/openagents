---
id: openagents.tools
version: 7
kind: product
title: "Plugins"
summary: >-
  A plugin is anything you add to OpenAgents, with the tests that show whether
  it helps; Coder's built-in plugins hand tasks to Claude Code, Codex, Cursor,
  and Grok Build.
tags: [gym, tools, extensions, plugins]
applies_when: >-
  The user asks what a plugin is (or a capability or extension), what a
  plugin contains, or how plugins relate to Coder and the Gym. Not when the
  user asks what tools or abilities we have; answer that plainly with what
  this chat and Coder can do.
answer: >-
  A plugin is anything you add to OpenAgents. It can contain skills
  (instructions Coder reads), workflows (typed step-by-step programs),
  knowledge (cited reference entries), Wasm (small sandboxed code), and the
  tests that show whether it helps. Coder's built-in plugins hand tasks to
  Claude Code, Codex, Cursor, and Grok Build on your computer. In the Gym a
  plugin is tested by running the same tests with it and without it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/plugins/README.md
    - docs/extensions/evaluation.md
    - docs/extensions/plugins.md
    - knowledge/openagents/openagents.plugin-list.md
    - crates/coder/src/builtin_plugins.rs
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
  - "2026-09-29: version 2 (#9958) says capability, the on-screen word decided in #9957; the note's id and tags stay."
  - "2026-10-01: version 3 (#10087) says plugin, the owner's one word for anything a person adds, and names its parts from docs/plugins/README.md; the note's id and tags stay."
  - "2026-10-01: version 4 (#10090) names no hand-kept list of plugins: `openagents.plugin-list`, generated from the hosted runner's catalog, lists them all."
  - "2026-10-08: version 5 (#11031) stops claiming 'what tools do you have' questions (meta.tools answers those plainly) and drops the internal 'we don't call it a tool' terminology note, which the chat repeated to users as a confusing explanation."
  - "2026-10-09: version 6 says where plugins run (with Coder on your computer), so an answer on the website never suggests running one there; the website answers 'which plugin should I try' with the catalog's plugin cards (docs/web/plugin-card.md)."
  - "2026-10-09: version 7 names the plugins that work today, Coder's ACP delegation to Claude Code, Codex, Cursor, and Grok Build (crates/coder/src/builtin_plugins.rs), instead of the Gym's sample plugins, which are no longer shown."
---

## Answer

A plugin is anything you add to OpenAgents. It can contain skills (instructions Coder reads), workflows (typed step-by-step programs), knowledge (cited reference entries), Wasm (small sandboxed code), and the tests that show whether it helps. Coder's built-in plugins hand tasks to Claude Code, Codex, Cursor, and Grok Build on your computer. In the Gym a plugin is tested by running the same tests with it and without it.

## Details

- Wasm is the only code a plugin can carry, and it runs in a sandbox with no network and bounded reads.
- A plugin made in chat is a skill: plain instructions Coder follows. Plugins with new code are made with Coder on a connected computer.
- Coder's built-in plugins live in the `openagents` terminal, where `/plugins` turns each on or off. Coder finds Claude Code, Codex, Cursor, and Grok Build when they are installed and hands them tasks.
- Plugins run with Coder on your computer. The openagents.com website can't run a plugin; it shows each built-in plugin with a link to get Coder.

## Sources

- `docs/plugins/README.md`
- `docs/extensions/evaluation.md`
- `docs/extensions/plugins.md`
- `knowledge/openagents/openagents.plugin-list.md`
- `crates/coder/src/builtin_plugins.rs`
