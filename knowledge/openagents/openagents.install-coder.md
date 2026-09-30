---
id: openagents.install-coder
version: 2
kind: product
title: "Getting Coder on a computer"
summary: >-
  Coder comes with OpenAgents for Mac; after pairing, pick a project and let
  the phone start Coder there, with Codex or Claude Code signed in on the Mac.
tags: [install, coder, computer, setup, mac]
applies_when: >-
  The user asks how to install or set up Coder, the coder command, or the
  Coder host on their computer. Not how to pair the phone
  (openagents.connect-computer).
answer: >-
  Coder comes with OpenAgents for Mac, so there's nothing else to install.
  Open the app, connect your phone by scanning its QR code, then pick a
  project folder and turn on **Let my phone start Coder here**. Sign in to Codex
  or Claude Code on the Mac, and Run Coder in a chat on your phone starts
  Coder there.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/install.rs
    - crates/openagents-desktop/README.md
    - docs/coder/guides/link-devices.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
---

## Answer

Coder comes with OpenAgents for Mac, so there's nothing else to install. Open the app, connect your phone by scanning its QR code, then pick a project folder and turn on **Let my phone start Coder here**. Sign in to Codex or Claude Code on the Mac, and Run Coder in a chat on your phone starts Coder there.

## Details

- The desktop app bundles `coder` and `microcoder` and runs the Coder host as a login agent; it updates itself.
- A computer without a screen gets the `openagents` binary and its host from `openagents connect --ssh HOST`, run from a computer that reaches it over SSH.

## Sources

- `crates/openagents-web/src/pages/install.rs`
- `crates/openagents-desktop/README.md`
- `docs/coder/guides/link-devices.md`
