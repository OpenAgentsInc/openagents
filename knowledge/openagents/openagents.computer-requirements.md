---
id: openagents.computer-requirements
version: 4
kind: product
title: "What you need to use Coder"
summary: >-
  Coder needs your own computer: a Mac with OpenAgents for Mac and Codex,
  Claude Code, or Grok Build signed in; there is no hosted computer yet.
tags: [coder, requirements, computer, hosted]
applies_when: >-
  The user asks what they need to use Coder, whether Coder works without their
  own computer, or whether OpenAgents provides a hosted or cloud computer.
answer: >-
  Coder runs on your own computer, on macOS, Linux, or Windows, with Codex,
  Claude Code, or Grok Build signed in there. Install it with `curl -fsSL
  https://openagents.com/cli/install.sh | bash` (on Windows, in PowerShell,
  `irm https://openagents.com/cli/install.ps1 | iex`), then run `coder` in
  your project. We don't provide a hosted computer for Coder yet. Without one,
  you can still chat with us here.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/download.rs
    - docs/coder/guides/link-devices.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
  - "2026-10-01: version 3 (#10091): Grok Build is allowed by default beside Codex and Claude Code, on the Mac's own runs and its phone switch, so any of the three signed in there runs Coder; checked against INVARIANTS.md and docs/cli/settings.md."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
---

## Answer

Coder runs on your own computer, on macOS, Linux, or Windows, with Codex, Claude Code, or Grok Build signed in there. Install it with `curl -fsSL https://openagents.com/cli/install.sh | bash` (on Windows, in PowerShell, `irm https://openagents.com/cli/install.ps1 | iex`), then run `coder` in your project. We don't provide a hosted computer for Coder yet. Without one, you can still chat with us here.

## Details

- Desktop builds for Linux and Windows aren't published yet.
- Chatting needs no computer.

## Sources

- `crates/openagents-web/src/pages/download.rs`
- `docs/coder/guides/link-devices.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `INVARIANTS.md`
