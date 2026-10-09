---
id: openagents.auto-start
version: 4
kind: product
title: "When a task from the phone starts on a computer"
summary: >-
  A task from the phone runs at once only where the owner allowed it: in
  OpenAgents for Mac, a picked project and **Let my phone start Coder here**;
  otherwise it is recorded without running.
tags: [coder, auto-start, host, permissions, in-app]
applies_when: >-
  The user asks why a task they sent didn't start, how to let their phone
  start Coder on their computer, or how tasks from the phone get permission to
  run there.
answer: >-
  A task from your phone starts on your computer right away only when you've
  allowed it there: in OpenAgents for Mac, pick a project folder and turn on
  **Let my phone start Coder here**. Then Coder runs at once in that project.
  Without it, the task is recorded but doesn't run by itself. Codex, Claude
  Code, or Grok Build must be signed in on the Mac. The phone sends only a project, a title,
  and a prompt.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-desktop/README.md
    - docs/coder/runtime/host-autostart.md
    - INVARIANTS.md
    - docs/coder/guides/link-devices.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
  - "2026-10-01: version 3 (#10091): Grok Build is allowed by default beside Codex and Claude Code, on the Mac's own runs and its phone switch, so any of the three signed in there runs Coder; checked against INVARIANTS.md and docs/cli/settings.md."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

A task from your phone starts on your computer right away only when you've allowed it there: in OpenAgents for Mac, pick a project folder and turn on **Let my phone start Coder here**. Then Coder runs at once in that project. Without it, the task is recorded but doesn't run by itself. Codex, Claude Code, or Grok Build must be signed in on the Mac. The phone sends only a project, a title, and a prompt.

## Details

- Only the computer itself turns auto-start on or widens it: the desktop app, or on a computer without it, `coder host autostart on`. No phone or relay message can.
- An auto-started task runs under a normal operator execution grant with every usual check.

## Sources

- `crates/openagents-desktop/README.md`
- `docs/coder/runtime/host-autostart.md`
- `INVARIANTS.md`
- `docs/coder/guides/link-devices.md`
