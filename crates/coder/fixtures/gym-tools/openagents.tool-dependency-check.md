---
id: openagents.tool-dependency-check
version: 2
kind: product
title: "Dependency check"
summary: >-
  Reads your manifests and lockfiles offline and flags duplicate versions, loose or unpinned version ranges, and licenses your declared policy doesn't allow.
tags: [gym, tool, dependency-check, plugin, extension]
applies_when: >-
  The user asks what Dependency check is or does, or whether it helps Coder.
answer: >-
  Dependency check reads a project's manifests and lockfiles offline (Cargo, npm, pnpm, Yarn, Python, and Go) and flags packages held at more than one version, dependencies left at any version or with no upper bound, and licenses the project's declared policy doesn't allow. In the Gym, a test set for it measures whether Coder does better with it than without it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/plugin-dependency-check/Cargo.toml
    - docs/plugins/examples/dependency-check.md
evidence:
  - "2026-10-01: written from the cited plugin and its example page (#10086)."
  - "2026-10-01: version 2 (#10090) takes its summary, its one line in the chat and on its Gym card, from the plugin's `package.json`; a test keeps the two equal."
---

## Answer

Dependency check reads a project's manifests and lockfiles offline (Cargo, npm, pnpm, Yarn, Python, and Go) and flags packages held at more than one version, dependencies left at any version or with no upper bound, and licenses the project's declared policy doesn't allow. In the Gym, a test set for it measures whether Coder does better with it than without it.

## Details

- It is an example plugin: Wasm (`crates/plugin-dependency-check`), the workflow that runs it, and its own test set, written to be copied.

## Sources

- `crates/plugin-dependency-check/Cargo.toml`
- `docs/plugins/examples/dependency-check.md`
