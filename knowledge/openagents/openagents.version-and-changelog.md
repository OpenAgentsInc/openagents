---
id: openagents.version-and-changelog
version: 1
kind: product
title: "Finding the app's version and what changed"
summary: >-
  Account > About this device shows the version and build; Account > Changelog
  lists each build's changes and a What to test line.
tags: [version, build, changelog, account]
applies_when: >-
  The user asks which version or build of the app they have, what changed in a
  build, or what to test.
answer: >-
  Account > About this device shows the app's version and build, and Account >
  Changelog lists what each build brought, with a What to test line for
  playtesters.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
    - crates/openagents-mobile/src/account.rs
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Account > About this device shows the app's version and build, and Account > Changelog lists what each build brought, with a What to test line for playtesters.

## Details

- Every TestFlight build ships with its own changelog entry.

## Sources

- `bins/openagents-ios/README.md`
- `crates/openagents-mobile/src/account.rs`
