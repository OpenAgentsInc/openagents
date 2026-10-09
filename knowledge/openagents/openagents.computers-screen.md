---
id: openagents.computers-screen
version: 3
kind: product
title: "Managing computers in Account"
summary: >-
  Account > Computers lists computers with their status, opens the scanner
  with Connect a computer, and has a menu per computer to switch it off or on,
  retry, open access, or forget it.
tags: [computers, account, manage, remove, terminal, in-app]
applies_when: >-
  The user asks how to see, manage, remove, forget, or switch off a computer,
  open a terminal on it, or see its recent work.
answer: >-
  Account > Computers lists your computers with a one-word status each.
  Connect a computer opens the scanner, and Add another way takes a pasted
  invitation. A computer's menu switches it off or on, tries it now, opens its
  access, or forgets it. Tapping a computer opens its status, order work, a
  terminal, access, and recent work. To take a phone's access away for good,
  click Remove next to it in OpenAgents for Mac.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-mobile/src/computers_home.rs
    - docs/coder/guides/link-devices.md
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

Account > Computers lists your computers with a one-word status each. Connect a computer opens the scanner, and Add another way takes a pasted invitation. A computer's menu switches it off or on, tries it now, opens its access, or forgets it. Tapping a computer opens its status, order work, a terminal, access, and recent work. To take a phone's access away for good, click Remove next to it in OpenAgents for Mac.

## Details

- Forget stops this phone connecting and drops the computer from the list; the computer keeps the phone's access until it's removed there.
- Rust builds the list from the Computers snapshot and runs every choice through the same authority check as the shared Computers screens.

## Sources

- `crates/openagents-mobile/src/computers_home.rs`
- `docs/coder/guides/link-devices.md`
- `bins/openagents-ios/README.md`
