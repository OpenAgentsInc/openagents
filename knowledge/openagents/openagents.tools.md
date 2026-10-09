---
id: openagents.tools
version: 8
kind: product
title: "Plugins"
summary: >-
  A plugin adds an ability to Coder: its built-in plugins hand tasks to
  Claude Code, Codex, Cursor, and Grok Build, and OpenRouter adds more
  models; people can make their own, with tests.
tags: [gym, tools, extensions, plugins]
applies_when: >-
  The user asks what plugins are, what a plugin is (or a capability or
  extension), what plugins do, what a plugin contains, or how plugins
  relate to Coder and the Gym. Not when the user asks what tools or
  abilities we have; answer that plainly with what this chat and Coder
  can do.
answer: >-
  A plugin adds an ability to Coder, our coding agent, and you turn each
  one on or off with `/plugins` in Coder. Its built-in plugins hand a
  task to Claude Code, Codex, Cursor, or Grok Build on your computer and
  show its progress as it works, and OpenRouter lets you use more models
  with your own key. You can make your own plugin too: instructions,
  small sandboxed programs, and tests that show whether it helps.
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
    - crates/coder-new/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
  - "2026-09-29: version 2 (#9958) says capability, the on-screen word decided in #9957; the note's id and tags stay."
  - "2026-10-01: version 3 (#10087) says plugin, the owner's one word for anything a person adds, and names its parts from docs/plugins/README.md; the note's id and tags stay."
  - "2026-10-01: version 4 (#10090) names no hand-kept list of plugins: `openagents.plugin-list`, generated from the hosted runner's catalog, lists them all."
  - "2026-10-08: version 5 (#11031) stops claiming 'what tools do you have' questions (meta.tools answers those plainly) and drops the internal 'we don't call it a tool' terminology note, which the chat repeated to users as a confusing explanation."
  - "2026-10-09: version 6 says where plugins run (with Coder on your computer), so an answer on the website never suggests running one there; the website answers 'which plugin should I try' with the catalog's plugin cards (docs/web/plugin-card.md)."
  - "2026-10-09: version 7 names the plugins that work today, Coder's ACP delegation to Claude Code, Codex, Cursor, and Grok Build (crates/coder/src/builtin_plugins.rs), instead of the Gym's sample plugins, which are no longer shown."
  - "2026-10-09: version 8 (#11095) answers the starter question 'What are plugins?' and matches the prepared answer meta.plugins: what a plugin does for Coder, `/plugins` to turn one on or off, the built-in plugins (Claude Code, Codex, Cursor, Grok Build, OpenRouter), and making your own."
---

## Answer

A plugin adds an ability to Coder, our coding agent, and you turn each one on or off with `/plugins` in Coder. Its built-in plugins hand a task to Claude Code, Codex, Cursor, or Grok Build on your computer and show its progress as it works, and OpenRouter lets you use more models with your own key. You can make your own plugin too: instructions, small sandboxed programs, and tests that show whether it helps.

## Details

- `/plugins` in the Coder terminal lists its plugins; Space turns one on or off and Enter opens its settings. OpenRouter is off until you add a key.
- Coder finds Claude Code, Codex, Cursor, and Grok Build when they are installed on your computer and hands them tasks, showing their progress as they work.
- A plugin you make can contain skills (instructions Coder reads), workflows (typed step-by-step programs), knowledge (cited reference entries), Wasm (small sandboxed code with no network), and the tests that show whether it helps. A plugin made in chat is a skill.
- In the Gym a plugin is tested by running the same tests with it and without it.
- Plugins run with Coder on your computer. The openagents.com website can't run a plugin; it shows each built-in plugin with a link to get Coder.

## Sources

- `docs/plugins/README.md`
- `docs/extensions/evaluation.md`
- `docs/extensions/plugins.md`
- `knowledge/openagents/openagents.plugin-list.md`
- `crates/coder/src/builtin_plugins.rs`
- `crates/coder-new/README.md`
