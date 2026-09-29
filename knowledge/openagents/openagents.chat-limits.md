---
id: openagents.chat-limits
version: 1
kind: product
title: "How many messages the chat allows"
summary: >-
  The chat worker allows each device key 6 messages in any minute and 40 a
  day, with a shared daily total, and the app says how long to wait.
tags: [chat, limits, quota, rate-limit]
applies_when: >-
  The user asks how many messages they can send, why the chat says to wait or
  that it's done for today, or what the chat's limits are; not what OpenAgents
  costs.
answer: >-
  Our chat allows each device key up to 6 messages in any minute and 40 a day,
  and the day resets at midnight UTC. There's also a total for everyone
  together each day. When you reach a limit, the app says how long to wait.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/deployment/chat-worker.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Our chat allows each device key up to 6 messages in any minute and 40 a day, and the day resets at midnight UTC. There's also a total for everyone together each day. When you reach a limit, the app says how long to wait.

## Details

- The deployed limits, as documented on 2026-09-28: 6 jobs per key in any 60 seconds, 40 per key per UTC day, 3,000 for every caller together per UTC day, 96 KiB per request.
- A refusal counts nothing and carries the wait when waiting helps.
- The phone sends at most the newest 48 KiB of the conversation.

## Sources

- `docs/deployment/chat-worker.md`
- `INVARIANTS.md`
