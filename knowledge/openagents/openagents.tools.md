---
id: openagents.tools
version: 5
kind: product
title: "Plugins"
summary: >-
  A plugin is anything you add to OpenAgents, such as Project map, with the
  tests that show whether it helps; the Gym tests whether a plugin makes
  Coder better.
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
  tests that show whether it helps. Project map is
  one; ask us which plugins the Gym has to see them all. In the Gym we test a plugin by running the same tests with it
  and without it, so you can see whether Coder does better with it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/plugins/README.md
    - docs/extensions/evaluation.md
    - docs/product/2026-09-28-app-wireframe.md
    - docs/extensions/plugins.md
    - knowledge/openagents/openagents.plugin-list.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
  - "2026-09-29: version 2 (#9958) says capability, the on-screen word decided in #9957; the note's id and tags stay."
  - "2026-10-01: version 3 (#10087) says plugin, the owner's one word for anything a person adds, and names its parts from docs/plugins/README.md; the note's id and tags stay."
  - "2026-10-01: version 4 (#10090) names no hand-kept list of plugins: `openagents.plugin-list`, generated from the hosted runner's catalog, lists them all."
  - "2026-10-08: version 5 (#11031) stops claiming 'what tools do you have' questions (meta.tools answers those plainly) and drops the internal 'we don't call it a tool' terminology note, which the chat repeated to users as a confusing explanation."
---

## Answer

A plugin is anything you add to OpenAgents. It can contain skills (instructions Coder reads), workflows (typed step-by-step programs), knowledge (cited reference entries), Wasm (small sandboxed code), and the tests that show whether it helps. Project map is one; ask us which plugins the Gym has to see them all. In the Gym we test a plugin by running the same tests with it and without it, so you can see whether Coder does better with it.

## Details

- Wasm is the only code a plugin can carry, and it runs in a sandbox with no network and bounded reads.
- A plugin made in chat is a skill: plain instructions Coder follows, which may turn on plugins such as Project map. Plugins with new code are made with Coder on a connected computer.
- Coder and the coding agents it can use (Codex, Claude Code, and others) are not plugins: they are what plugins plug into.
- Engineering documents ship a plugin as a NIP-EXT extension package and call a measured improvement a capability; the app and this chat say plugin.

## Sources

- `docs/plugins/README.md`
- `docs/extensions/evaluation.md`
- `docs/product/2026-09-28-app-wireframe.md`
- `docs/extensions/plugins.md`
- `knowledge/openagents/openagents.plugin-list.md`
