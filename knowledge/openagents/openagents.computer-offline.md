---
id: openagents.computer-offline
version: 1
kind: product
title: "When your computer is offline"
summary: >-
  Messages to Coder wait in an encrypted outbox with a stable command ID; the
  phone nudges the computer and resends when it returns.
tags: [computer, offline, outbox, reliability]
applies_when: >-
  The user asks what happens when their computer is offline, asleep, or
  unreachable, or whether a message could be sent twice.
answer: >-
  Every message to Coder on a computer waits in the app's encrypted outbox
  until the computer answers, with the same command ID each try, so nothing is
  sent twice. If the computer can't be reached, the phone leaves it a nudge on
  the relay, and when the computer comes back the waiting command goes out at
  once.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/docs/chat-later.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Every message to Coder on a computer waits in the app's encrypted outbox until the computer answers, with the same command ID each try, so nothing is sent twice. If the computer can't be reached, the phone leaves it a nudge on the relay, and when the computer comes back the waiting command goes out at once.

## Details

- A relaunch or a bad connection never sends a command twice.

## Sources

- `bins/openagents-ios/docs/chat-later.md`
