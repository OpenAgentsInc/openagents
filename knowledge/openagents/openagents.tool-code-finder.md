---
id: openagents.tool-code-finder
version: 2
kind: product
title: "Code finder"
summary: >-
  Finds the lines of code people marked for follow-up: TODO, FIXME, XXX, and HACK notes, grouped by file.
tags: [gym, tool, code-finder, code-search, extension]
applies_when: >-
  The user asks what Code finder is or does, or whether it helps Coder.
answer: >-
  Code finder finds the notes people leave in code for follow-up (TODO, FIXME,
  XXX, and HACK) and shows Coder the matching lines grouped by file. Its Wasm
  searches a project for up to 16 patterns and skips binary, lock, and
  minified files. In the Gym, a test set for it measures whether Coder does
  better with it than without it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/plugin-code-search/Cargo.toml
    - crates/plugin-code-search/package.json
    - docs/extensions/plugins.md
    - docs/product/2026-09-28-app-wireframe.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
  - "2026-10-01: version 2 (#10090) takes its summary, its one line in the chat and on its Gym card, from the plugin's `package.json`; a test keeps the two equal. Its answer says what the plugin's workflow looks for, the notes people leave in code (`crates/plugin-code-search/package.json`)."
---

## Answer

Code finder finds the notes people leave in code for follow-up (TODO, FIXME, XXX, and HACK) and shows Coder the matching lines grouped by file. Its Wasm searches a project for up to 16 patterns and skips binary, lock, and minified files. In the Gym, a test set for it measures whether Coder does better with it than without it.

## Details

- Its engineering name is the `code_search` evidence guest, operation `search`.
- A pattern is literal text where `*` matches any run within a line.

## Sources

- `crates/plugin-code-search/Cargo.toml`
- `crates/plugin-code-search/package.json`
- `docs/extensions/plugins.md`
- `docs/product/2026-09-28-app-wireframe.md`
