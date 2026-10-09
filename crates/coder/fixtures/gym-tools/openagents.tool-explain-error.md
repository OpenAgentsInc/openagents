---
id: openagents.tool-explain-error
version: 2
kind: product
title: "Explain this error"
summary: >-
  Reads a failing command's output, finds the file and line in your project it points at, and explains the likely cause and a likely fix.
tags: [gym, tool, explain-error, plugin, extension]
applies_when: >-
  The user asks what Explain this error is or does, or whether it helps Coder.
answer: >-
  Explain this error reads a failing command's output (a compiler error, a failing test, or a stack trace) that you paste or save to a log, finds the file and line in the project it points at, shows the code there, and says the likely cause and a likely fix. In the Gym, a test set for it measures whether Coder does better with it than without it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/plugin-explain-error/Cargo.toml
    - docs/plugins/examples/explain-this-error.md
evidence:
  - "2026-10-01: written from the cited plugin and its example page (#10086)."
  - "2026-10-01: version 2 (#10090) takes its summary, its one line in the chat and on its Gym card, from the plugin's `package.json`; a test keeps the two equal."
---

## Answer

Explain this error reads a failing command's output (a compiler error, a failing test, or a stack trace) that you paste or save to a log, finds the file and line in the project it points at, shows the code there, and says the likely cause and a likely fix. In the Gym, a test set for it measures whether Coder does better with it than without it.

## Details

- It is an example plugin: Wasm (`crates/plugin-explain-error`), the workflow that runs it, and its own test set, written to be copied.

## Sources

- `crates/plugin-explain-error/Cargo.toml`
- `docs/plugins/examples/explain-this-error.md`
