---
id: openagents.auto-start
version: 1
kind: product
title: "When a task from the phone starts on a computer"
summary: >-
  A task from the phone runs at once only on a computer whose owner turned on
  the host's auto-start policy; otherwise it is recorded without running.
tags: [coder, auto-start, host, permissions, full-access]
applies_when: >-
  The user asks why a task they sent didn't start, what full-access hosts are,
  or how tasks from the phone get permission to run on a computer.
answer: >-
  A task from your phone starts on a computer right away only when that
  computer's owner has turned on the host's auto-start policy with a command
  on the computer. Then Coder runs at once, with the access the owner granted,
  in the workspaces the policy lists. Without the policy, the task is recorded
  but doesn't run by itself. The phone sends only a workspace, a title, and a
  prompt.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - INVARIANTS.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - bins/openagents-ios/docs/chat-later.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

A task from your phone starts on a computer right away only when that computer's owner has turned on the host's auto-start policy with a command on the computer. Then Coder runs at once, with the access the owner granted, in the workspaces the policy lists. Without the policy, the task is recorded but doesn't run by itself. The phone sends only a workspace, a title, and a prompt.

## Details

- Only the host's owner, with a command on the host, turns auto-start on or widens it.
- An auto-started task runs under a normal operator execution grant with every usual check.

## Sources

- `INVARIANTS.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `bins/openagents-ios/docs/chat-later.md`
