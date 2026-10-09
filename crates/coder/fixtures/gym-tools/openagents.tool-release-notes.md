---
id: openagents.tool-release-notes
version: 2
kind: product
title: "Release notes"
summary: >-
  Turns the commits between two releases into grouped, user-facing release notes (breaking changes, features, fixes), each line citing its commit.
tags: [gym, tool, release-notes, plugin, extension]
applies_when: >-
  The user asks what Release notes is or does, or whether it helps Coder.
answer: >-
  Release notes reads a git log you paste or save to a file and groups the commits into user-facing release notes: breaking changes, features, fixes, and the rest, each line citing its commit, with merges left out. In the Gym, a test set for it measures whether Coder does better with it than without it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/plugin-release-notes/Cargo.toml
    - docs/plugins/examples/release-notes.md
evidence:
  - "2026-10-01: written from the cited plugin and its example page (#10086)."
  - "2026-10-01: version 2 (#10090) takes its summary, its one line in the chat and on its Gym card, from the plugin's `package.json`; a test keeps the two equal."
---

## Answer

Release notes reads a git log you paste or save to a file and groups the commits into user-facing release notes: breaking changes, features, fixes, and the rest, each line citing its commit, with merges left out. In the Gym, a test set for it measures whether Coder does better with it than without it.

## Details

- It is an example plugin: Wasm (`crates/plugin-release-notes`), the workflow that runs it, and its own test set, written to be copied.

## Sources

- `crates/plugin-release-notes/Cargo.toml`
- `docs/plugins/examples/release-notes.md`
