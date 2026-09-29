---
id: openagents.chat-features-later
version: 1
kind: product
title: "Chat features that aren't built yet"
summary: >-
  Photo attachments, voice dictation, a model picker, and push notifications
  for finished tasks aren't built yet; Coder's replies update per step.
tags: [chat, roadmap, attachments, dictation, notifications, limits]
applies_when: >-
  The user asks whether they can attach photos or files, dictate by voice,
  pick a model, or get notifications, or why Coder's reply doesn't stream
  token by token.
answer: >-
  Not yet: photo attachments, voice dictation, a model picker, and push
  notifications when a task finishes. Coder's replies from your computer
  update step by step as the engine records them, not token by token. These
  are on our list of later work.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - bins/openagents-ios/docs/chat-later.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Not yet: photo attachments, voice dictation, a model picker, and push notifications when a task finishes. Coder's replies from your computer update step by step as the engine records them, not token by token. These are on our list of later work.

## Details

- Attachments need an attachment field or artifact upload that the host admits.
- A per-task model needs a `task.create` field the host's policy admits.
- A chat on the phone shows at most its newest 240 rows.

## Sources

- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `bins/openagents-ios/docs/chat-later.md`
