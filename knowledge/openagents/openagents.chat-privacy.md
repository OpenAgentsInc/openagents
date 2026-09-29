---
id: openagents.chat-privacy
version: 1
kind: product
title: "How chat messages travel and who sees them"
summary: >-
  Chat messages are signed by the device key, encrypted to the chat worker,
  and carried by an ephemeral relay; the worker sends the conversation to its
  model, to Jev, and, for product lookups, to an embeddings provider.
tags: [privacy, encryption, chat, relay, data]
applies_when: >-
  The user asks whether chat messages are private, encrypted, stored, or
  logged, or which services see what they write in the chat.
answer: >-
  Each message is signed by your phone's device key and encrypted to our chat
  worker, then carried by our relay, relay.openagents.com, which sees only
  ciphertext and keeps nothing, because chat messages are ephemeral. To
  answer, the worker sends the conversation to a Gemini model through Vercel's
  AI Gateway and to TypeSafe's Jev, which chooses how we reply; when we search
  our product notes, your message also goes to an embeddings provider. The app
  holds no model key.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/deployment/chat-worker.md
    - INVARIANTS.md
    - nips/openagents/NIP-CJ.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Each message is signed by your phone's device key and encrypted to our chat worker, then carried by our relay, relay.openagents.com, which sees only ciphertext and keeps nothing, because chat messages are ephemeral. To answer, the worker sends the conversation to a Gemini model through Vercel's AI Gateway and to TypeSafe's Jev, which chooses how we reply; when we search our product notes, your message also goes to an embeddings provider. The app holds no model key.

## Details

- The request carries the conversation, instructions, a client name, and the first-response request, and no credential, model choice, or grant.
- The phone shows only answers signed by the worker's key, tagged to its own request and device.
- There is no account, sign-in, or key to paste: the device key made on first launch signs the request.

## Sources

- `docs/deployment/chat-worker.md`
- `INVARIANTS.md`
- `nips/openagents/NIP-CJ.md`
