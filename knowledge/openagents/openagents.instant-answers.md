---
id: openagents.instant-answers
version: 1
kind: product
title: "Why some answers appear instantly"
summary: >-
  Jev reads each message first; when a prepared answer or a documented entry
  fits exactly, it is shown at once instead of waiting for the model.
tags: [jev, speed, prepared-answers, chat]
applies_when: >-
  The user asks why some replies appear instantly while others take a few
  seconds, or how the chat decides how to answer.
answer: >-
  A small, fast model, Jev from TypeSafe, reads each message first and chooses
  how we reply. When one of our prepared answers, or an entry in our product
  notes, fully answers the question, we show it at once, in about half a
  second, without waiting for the larger model. Otherwise the larger model
  writes the reply, which takes a few seconds.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/deployment/chat-worker.md
    - crates/coder/src/first.rs
    - docs/coder/design/2026-09-28-chat-router.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

A small, fast model, Jev from TypeSafe, reads each message first and chooses how we reply. When one of our prepared answers, or an entry in our product notes, fully answers the question, we show it at once, in about half a second, without waiting for the larger model. Otherwise the larger model writes the reply, which takes a few seconds.

## Details

- Jev runs beside the model call and never in front of it, so a slow or failed judgment never delays the model's reply.
- Code, not Jev, decides what is shown, from Jev's probabilities and fixed thresholds.

## Sources

- `docs/deployment/chat-worker.md`
- `crates/coder/src/first.rs`
- `docs/coder/design/2026-09-28-chat-router.md`
