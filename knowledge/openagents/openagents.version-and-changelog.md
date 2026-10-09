---
id: openagents.version-and-changelog
version: 3
kind: product
title: "Finding the app's version and what changed"
summary: >-
  Account > About this device shows the version and build; Account > Changelog
  lists each build's changes and a What to test line.
tags: [version, build, changelog, account, in-app]
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
  - "2026-09-29: version 2 (#9941) adds build 21, Test tools in chat, from its changelog entry in crates/openagents-mobile/src/account.rs; the answer is unchanged."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

Account > About this device shows the app's version and build, and Account > Changelog lists what each build brought, with a What to test line for playtesters.

## Details

- Every TestFlight build ships with its own changelog entry.
- Build 21, **Test tools in chat**, puts the Gym in chat: a menu with **Chat with OpenAgents** first, a first test in three taps, tests of a tool with and without it, making a tool and its tests with us, **Add to the Gym**, checks of other trainers' results, and the XP your work earns.
- Build 20 brought smarter chat and a simpler Wallet.

## Sources

- `bins/openagents-ios/README.md`
- `crates/openagents-mobile/src/account.rs`
