---
id: openagents.computers-screen
version: 1
kind: product
title: "Managing computers in Account"
summary: >-
  Account > Computers lists computers with their status and a menu to switch
  them off or on, retry, open access, or forget them.
tags: [computers, account, manage, remove, terminal]
applies_when: >-
  The user asks how to see, manage, remove, forget, or switch off a computer,
  open a terminal on it, or see its recent work.
answer: >-
  Account > Computers lists your computers with a one-word status each. A
  computer's menu switches it off or on, tries it now, opens its access, or
  forgets it. Tapping a computer opens its status, order work, a terminal,
  access, and recent work. Add a computer and Activity are on the same screen.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Account > Computers lists your computers with a one-word status each. A computer's menu switches it off or on, tries it now, opens its access, or forgets it. Tapping a computer opens its status, order work, a terminal, access, and recent work. Add a computer and Activity are on the same screen.

## Details

- Rust builds the list from the Computers snapshot and runs every choice through the same authority check as the shared Computers screens.

## Sources

- `bins/openagents-ios/README.md`
