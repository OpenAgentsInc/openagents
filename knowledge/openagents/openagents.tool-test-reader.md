---
id: openagents.tool-test-reader
version: 2
kind: product
title: "Test reader"
summary: >-
  Reads the test reports in a project for Coder: which tests failed, where, and why.
tags: [gym, tool, test-reader, test-report, extension]
applies_when: >-
  The user asks what Test reader is or does, or whether it helps Coder.
answer: >-
  Test reader reads a project's test reports (JUnit XML, cargo test output, or
  pytest output) and shows Coder the counts and each failing test with its
  file, line, and message. In the Gym, a test set for it measures whether
  Coder does better with it than without it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/plugin-test-report/Cargo.toml
    - docs/extensions/plugins.md
    - docs/product/2026-09-28-app-wireframe.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9936); the answer text awaits the owner's copy review."
  - "2026-10-01: version 2 (#10090) takes its summary, its one line in the chat and on its Gym card, from the plugin's `package.json`; a test keeps the two equal."
---

## Answer

Test reader reads a project's test reports (JUnit XML, cargo test output, or pytest output) and shows Coder the counts and each failing test with its file, line, and message. In the Gym, a test set for it measures whether Coder does better with it than without it.

## Details

- Its engineering name is the `test_report` evidence guest, operation `parse`.

## Sources

- `crates/plugin-test-report/Cargo.toml`
- `docs/extensions/plugins.md`
- `docs/product/2026-09-28-app-wireframe.md`
