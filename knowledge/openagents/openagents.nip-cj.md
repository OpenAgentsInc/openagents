---
id: openagents.nip-cj
version: 1
kind: product
title: "What NIP-CJ is"
summary: >-
  NIP-CJ is the OpenAgents Nostr protocol for encrypted agent jobs; the chat
  uses its conversation kinds 25900, 27000, and 26900, all ephemeral.
tags: [nip-cj, protocol, nostr, chat]
applies_when: >-
  The user asks what NIP-CJ is, how a chat message travels as a Nostr event,
  or what kinds 25900, 26900, or 27000 are.
answer: >-
  NIP-CJ is our Nostr protocol for agent jobs. The chat uses its conversation
  family: your phone sends a kind 25900 request encrypted to the chat worker,
  and the worker answers with kind 27000 feedback, including partial text, and
  one kind 26900 result. All of these kinds are ephemeral, so relays pass them
  along without keeping them.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - nips/openagents/NIP-CJ.md
    - docs/deployment/chat-worker.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

NIP-CJ is our Nostr protocol for agent jobs. The chat uses its conversation family: your phone sends a kind 25900 request encrypted to the chat worker, and the worker answers with kind 27000 feedback, including partial text, and one kind 26900 result. All of these kinds are ephemeral, so relays pass them along without keeping them.

## Details

- NIP-CJ also defines decision jobs (25910) and execution jobs (25920).
- A relay transports requests; it grants no execution authority.

## Sources

- `nips/openagents/NIP-CJ.md`
- `docs/deployment/chat-worker.md`
