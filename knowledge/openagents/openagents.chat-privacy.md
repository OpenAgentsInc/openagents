---
id: openagents.chat-privacy
version: 3
kind: product
title: "How chat messages travel and who sees them"
summary: >-
  Chat messages are signed by the device key, encrypted to the chat worker,
  and carried by a relay that keeps nothing; the worker stores no message
  text and sends the conversation to Jev (TypeSafe, through the Vercel AI
  Gateway), Space Bunny Alpha (OpenRouter), Gemini (Vercel AI Gateway) when
  Space Bunny can't answer, and, for product lookups, an embeddings
  provider.
tags: [privacy, encryption, chat, relay, data]
applies_when: >-
  The user asks whether chat messages are private, encrypted, stored, or
  logged, which services or model providers see what they write in the
  chat, who else can read their chats, whether the chat runs locally, or
  whether they can turn provider access off.
answer: >-
  Each message is encrypted to our chat worker; our relay sees only
  ciphertext and keeps nothing, and the worker stores no message text.
  Replies aren't made on your computer: the worker sends the conversation to
  TypeSafe's Jev through the Vercel AI Gateway to choose how we reply, to
  Space Bunny Alpha on OpenRouter, whose provider may keep prompts but not
  train on them, and to Gemini 3.8 Flash on the Vercel AI Gateway when Space
  Bunny can't answer. Product lookups also reach an embeddings provider. No
  one else reads your chats, and no setting turns the providers off.
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
  - "2026-10-01: the chat model is Space Bunny Alpha on OpenRouter first, Gemini 3.8 Flash on the gateway after (#10109); OpenRouter's notice for the model says prompts and completions may be retained by the provider but are not used for training."
  - "2026-10-02: the answer now names Jev's route through the Vercel AI Gateway (its first door since #10110), says the worker stores no message text and that the chat is not answered on the computer, and says no setting turns the providers off (#10136)."
---

## Answer

Each message is encrypted to our chat worker; our relay sees only ciphertext and keeps nothing, and the worker stores no message text. Replies aren't made on your computer: the worker sends the conversation to TypeSafe's Jev through the Vercel AI Gateway to choose how we reply, to Space Bunny Alpha on OpenRouter, whose provider may keep prompts but not train on them, and to Gemini 3.8 Flash on the Vercel AI Gateway when Space Bunny can't answer. Product lookups also reach an embeddings provider. No one else reads your chats, and no setting turns the providers off.

## Details

- The request carries the conversation, instructions, a client name, and the first-response request, and no credential, model choice, or grant.
- On a computer (the desktop app or `openagents chat`), or from a phone paired with one, the request also names that computer by the label you gave it and the chat's project folder, with the folder's path only from the computer itself. Only the chat worker and the chat model read them, so the chat knows it is on your computer and can say which folder it works in (#10077).
- The phone shows only answers signed by the worker's key, tagged to its own request and device.
- Each message is signed by your device key; the relay is relay.openagents.com, and every event it carries is ephemeral.
- Your conversations are saved on your device, not on our servers.
- There is no account, sign-in, or key to paste: the device key made on first launch signs the request, and the app holds no model key.
- Jev's doors, in order: the Vercel AI Gateway (`typesafe-ai/jev`), then OpenRouter, then TypeSafe direct.
- The worker's usage log records each job's time, key, surface, route, model, and timings, never the message text.
- When Coder runs on your computer, it sends what it reads to the coding agent's model provider; your code stays on that computer.

## Sources

- `docs/deployment/chat-worker.md`
- `INVARIANTS.md`
- `nips/openagents/NIP-CJ.md`
