---
id: openagents.tool-release-notes
version: 1
kind: product
title: "Release notes"
summary: >-
  Turns a list of commits into grouped, user-facing release notes, each line citing its commit.
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
---

## Answer

Release notes reads a git log you paste or save to a file and groups the commits into user-facing release notes: breaking changes, features, fixes, and the rest, each line citing its commit, with merges left out. In the Gym, a test set for it measures whether Coder does better with it than without it.

## Details

- It is an example plugin: Wasm (`crates/plugin-release-notes`), the workflow that runs it, and its own test set, written to be copied.

## Sources

- `crates/plugin-release-notes/Cargo.toml`
- `docs/plugins/examples/release-notes.md`
