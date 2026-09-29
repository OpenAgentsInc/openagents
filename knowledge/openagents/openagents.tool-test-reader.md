---
id: openagents.tool-test-reader
version: 1
kind: product
title: "Test reader"
summary: >-
  Reads test reports and shows Coder each failing test with its file, line,
  and message.
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
---

## Answer

Test reader reads a project's test reports (JUnit XML, cargo test output, or pytest output) and shows Coder the counts and each failing test with its file, line, and message. In the Gym, a test set for it measures whether Coder does better with it than without it.

## Details

- Its engineering name is the `test_report` evidence guest, operation `parse`.

## Sources

- `crates/plugin-test-report/Cargo.toml`
- `docs/extensions/plugins.md`
- `docs/product/2026-09-28-app-wireframe.md`
