---
id: openagents.plugin-list
version: 2
kind: product
title: "Which plugins there are"
summary: >-
  Coder's built-in plugins: Claude Code, Codex, Cursor, Grok Build, and OpenRouter.
tags: [plugins, catalog, coder]
applies_when: >-
  The user asks which plugins there are, which plugins Coder has, or which plugins they can use or test with Coder.
answer: >-
  Coder comes with five built-in plugins: Claude Code, Codex, Cursor, Grok Build, and OpenRouter. With the coding agents, Coder hands a task to Claude Code, Codex, Cursor, or Grok Build on your computer, when you have it, and shows its progress as it works. With OpenRouter, Coder uses OpenRouter models with your own API key. In the openagents terminal, /plugins turns each on or off.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/coder/src/builtin_plugins.rs
    - crates/coder-new/src/acp_discovery.rs
    - crates/coder-new/src/bundled_runtime.rs
    - crates/coder-new/src/delegation_events.rs
    - crates/coder-new/tests/bundled_plugins.rs
    - crates/coder-new/src/plugin_definition.rs
evidence:
  - "Generated from Coder's built-in plugins (crates/coder/src/builtin_plugins.rs) by crates/coder/tests/plugin_catalog.rs; PLUGIN_LIST_WRITE=1 rewrites it, and its version moves when its words do. The hosted runner's sample plugins are test fixtures and are not listed."
---

## Answer

Coder comes with five built-in plugins: Claude Code, Codex, Cursor, Grok Build, and OpenRouter. With the coding agents, Coder hands a task to Claude Code, Codex, Cursor, or Grok Build on your computer, when you have it, and shows its progress as it works. With OpenRouter, Coder uses OpenRouter models with your own API key. In the openagents terminal, /plugins turns each on or off.

## Details

- **Claude Code**: Coder hands a task to Claude Code on your computer and shows its progress as it works.
- **Codex**: Coder hands a task to Codex on your computer and shows its progress as it works.
- **Cursor**: Coder hands a task to Cursor's agent on your computer and shows its progress as it works.
- **Grok Build**: Coder hands a task to Grok Build on your computer and shows its progress as it works.
- **OpenRouter**: Use OpenRouter models in Coder with your own API key.

## Sources

- `crates/coder/src/builtin_plugins.rs`
- `crates/coder-new/src/acp_discovery.rs`
- `crates/coder-new/src/bundled_runtime.rs`
- `crates/coder-new/src/delegation_events.rs`
- `crates/coder-new/tests/bundled_plugins.rs`
- `crates/coder-new/src/plugin_definition.rs`
