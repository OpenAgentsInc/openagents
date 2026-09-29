---
id: openagents.tool-code-finder
version: 1
kind: product
title: "Code finder"
summary: >-
  Finds the lines in a project that match what Coder is looking for, grouped
  by file.
tags: [gym, tool, code-finder, code-search, extension]
applies_when: >-
  The user asks what Code finder is or does, or whether it helps Coder.
answer: >-
  Code finder searches a project for up to 16 patterns and shows Coder the
  matching lines grouped by file, with the files that match more patterns
  first. It skips binary, lock, and minified files. In the Gym, a test set for
  it measures whether Coder does better with it than without it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/plugin-code-search/Cargo.toml
    - docs/extensions/plugins.md
    - docs/product/2026-09-28-app-wireframe.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
---

## Answer

Code finder searches a project for up to 16 patterns and shows Coder the matching lines grouped by file, with the files that match more patterns first. It skips binary, lock, and minified files. In the Gym, a test set for it measures whether Coder does better with it than without it.

## Details

- Its engineering name is the `code_search` evidence guest, operation `search`.
- A pattern is literal text where `*` matches any run within a line.

## Sources

- `crates/plugin-code-search/Cargo.toml`
- `docs/extensions/plugins.md`
- `docs/product/2026-09-28-app-wireframe.md`
