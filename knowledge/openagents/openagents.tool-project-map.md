---
id: openagents.tool-project-map
version: 3
kind: product
title: "Project map"
summary: >-
  Shows Coder how the project is laid out before it starts: its files, languages, largest files, build files, and tests.
tags: [gym, tool, project-map, repo-map, extension]
applies_when: >-
  The user asks what Project map is or does, or whether it helps Coder.
answer: >-
  Project map shows Coder how a project is laid out before it starts: how many
  files and bytes it has, its languages, its top folders, its largest files,
  its build files, and its test files. It reads file sizes, not their
  contents. In the Gym, a test set for it measures whether Coder does better
  with it than without it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/plugin-repo-map/Cargo.toml
    - docs/extensions/plugins.md
    - docs/product/2026-09-28-app-wireframe.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
  - "2026-10-01: version 2 (#10087) says plugin, the one word for anything a person adds (skills, workflows, knowledge, Wasm, and tests); the note's id and tags stay."
  - "2026-10-01: version 3 (#10090) takes its summary, its one line in the chat and on its Gym card, from the plugin's `package.json`; a test keeps the two equal."
---

## Answer

Project map shows Coder how a project is laid out before it starts: how many files and bytes it has, its languages, its top folders, its largest files, its build files, and its test files. It reads file sizes, not their contents. In the Gym, a test set for it measures whether Coder does better with it than without it.

## Details

- Its engineering name is the `repo_map` evidence guest, operation `map`.
- It is a plugin, and the Gym's default plugin to test first.

## Sources

- `crates/plugin-repo-map/Cargo.toml`
- `docs/extensions/plugins.md`
- `docs/product/2026-09-28-app-wireframe.md`
