---
id: openagents.chat-limits
version: 4
kind: product
title: "How many messages the chat allows"
summary: >-
  The chat has no message limit: no per-minute, per-day, or shared cap. Only
  one request's size is bounded.
tags: [chat, limits, quota, rate-limit]
applies_when: >-
  The user asks how many messages they can send, why the chat says to wait or
  that it's done for today, or what the chat's limits are; not what OpenAgents
  costs.
answer: >-
  Send us as many messages as you like: we don't cap how many you send in
  a minute or in a day, and we don't throttle you or charge per message.
  There's no quota or fair-use cap behind that, so there's nothing to look
  up. If you ever see "Couldn't reach OpenAgents; try again.", send the
  message again.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/deployment/chat-worker.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-01: the owner removed every usage limit (#10120); rewritten from the cited documents."
  - "2026-10-02: the answer says there is no throttle, per-message charge, plan, account, quota, or fair-use cap behind the missing limit (#10135)."
  - "2026-10-09: v4 (chat goldens): accounts exist now and a paid plan is being built, so the answer no longer says there is no plan or account behind the missing limit."
---

## Answer

Send us as many messages as you like: we don't cap how many you send in a minute or in a day, and we don't throttle you or charge per message. There's no quota or fair-use cap behind that, so there's nothing to look up. If you ever see "Couldn't reach OpenAgents; try again.", send the message again.

## Details

- Since 2026-10-01 the chat worker counts no messages per device key, per minute, per day, or for everyone together (#10120).
- One request is at most 96 KiB; the phone sends at most the newest 48 KiB of the conversation, so a long chat stays inside it.
- The worker records each job's time, key, surface, route, model, and timings for usage stats, never the message text.

## Sources

- `docs/deployment/chat-worker.md`
- `INVARIANTS.md`
