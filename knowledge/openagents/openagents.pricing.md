---
id: openagents.pricing
version: 3
kind: product
title: "Pricing"
summary: >-
  OpenAgents hasn't published pricing; the documented facts are that the chat
  is free with no message cap and that Coder runs on the user's own
  computer.
tags: [pricing, cost, free, plans]
applies_when: >-
  The user asks what OpenAgents costs, whether it's free, or about plans,
  subscriptions, prices, paid tiers, fair-use caps, per-message charges, or
  whether they need their own API key; not the Wallet's network fees, and
  not the chat's message limits alone.
answer: >-
  Chatting with us is free: there's no plan, subscription, paid tier, or
  account, no message cap, usage cap, quota, rate limit, or throttle, and no
  API key to bring. We haven't published pricing for anything else, and
  we'll answer this when we do. Coder runs on your own computer through the
  coding agents you've signed in to there, such as Codex or Claude Code,
  under your own accounts with their providers; we bill nothing for it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/deployment/chat-worker.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-01: the chat's message limits are gone (#10120); the answer says so."
  - "2026-10-02: the answer says outright that there is no plan, subscription, account, cap, quota, rate limit, throttle, or API key to bring for the chat, and that Coder's coding agents run under the user's own accounts and we bill nothing (#10135)."
---

## Answer

Chatting with us is free: there's no plan, subscription, paid tier, or account, no message cap, usage cap, quota, rate limit, or throttle, and no API key to bring. We haven't published pricing for anything else, and we'll answer this when we do. Coder runs on your own computer through the coding agents you've signed in to there, such as Codex or Claude Code, under your own accounts with their providers; we bill nothing for it.

## Details

- No price is documented in the repository as of 2026-09-28.
- Wallet payments carry network and routing fees, shown on the confirm screen before you send.
- The chat needs no API key: the chat worker holds its own model keys, and the app holds none.
- There is no OpenAgents account, so there is no plan or settings page with a price, cap, or rate limit to look up.

## Sources

- `docs/deployment/chat-worker.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
