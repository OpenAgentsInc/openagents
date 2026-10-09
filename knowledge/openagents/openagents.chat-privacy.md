---
id: openagents.chat-privacy
version: 6
kind: product
title: "How chat messages travel and who sees them"
summary: >-
  In the apps, chat messages are signed by the device key, encrypted to the
  chat worker, carried by a relay that keeps nothing, and saved only on the
  device; the web chat saves conversations on our servers. The worker stores
  no message text and sends the conversation to Jev (TypeSafe, through the
  Vercel AI Gateway), Space Bunny Alpha (OpenRouter), Gemini (Vercel AI
  Gateway) when Space Bunny can't answer, and, for product lookups, an
  embeddings provider.
tags: [privacy, encryption, chat, relay, data]
applies_when: >-
  The user asks whether chat messages are private, encrypted, stored, or
  logged, whether web chats are kept, which services or model providers see
  what they write in the chat, who else can read their chats, whether the
  chat runs locally, or whether they can turn provider access off.
answer: >-
  In the web chat, our website reads your messages and saves your chats on
  our servers. In the apps, messages are encrypted to our chat worker, our
  relay keeps nothing, and chats are saved on your device. The worker stores
  no message text. To reply, it sends the conversation to TypeSafe's Jev
  through the Vercel AI Gateway, to Space Bunny Alpha on OpenRouter, whose
  provider may keep prompts but not train on them, and to Gemini 3.8 Flash
  on the Vercel AI Gateway when Space Bunny can't answer. Product lookups
  also reach an embeddings provider. No setting turns the providers off.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/deployment/chat-worker.md
    - INVARIANTS.md
    - nips/openagents/NIP-CJ.md
    - crates/openagents-web/src/chat_store.rs
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-01: the chat model is Space Bunny Alpha on OpenRouter first, Gemini 3.8 Flash on the gateway after (#10109); OpenRouter's notice for the model says prompts and completions may be retained by the provider but are not used for training."
  - "2026-10-02: the answer now names Jev's route through the Vercel AI Gateway (its first door since #10110), says the worker stores no message text and that the chat is not answered on the computer, and says no setting turns the providers off (#10136)."
  - "2026-10-02: BYOK (#10176): with **Use my keys for everything** on, messages reach the person's own provider accounts, and the keys travel sealed per message and are never kept."
  - "2026-10-02: on the person's keys, product, codebase, and Gym lookups stay on, embedded and judged on their keys (#10176)."
  - "2026-10-08: v6 separates the web chat, which saves conversations on our servers (crates/openagents-web/src/chat_store.rs, a private GCS bucket with no lifecycle rule and no delete route), from the apps, which save chats on the device; no code feeds chats into training."
---

## Answer

In the web chat, our website reads your messages and saves your chats on our servers. In the apps, messages are encrypted to our chat worker, our relay keeps nothing, and chats are saved on your device. The worker stores no message text. To reply, it sends the conversation to TypeSafe's Jev through the Vercel AI Gateway, to Space Bunny Alpha on OpenRouter, whose provider may keep prompts but not train on them, and to Gemini 3.8 Flash on the Vercel AI Gateway when Space Bunny can't answer. Product lookups also reach an embeddings provider. No setting turns the providers off.

## Details

- The request carries the conversation, instructions, a client name, and the first-response request, and no credential, model choice, or grant, except your own provider keys when you chose them (below).
- With **Use my keys for everything** on (your own OpenRouter, Vercel AI Gateway, or TypeSafe key, on a computer), your keys travel with each message, sealed to the worker apart from it, used for that message only, and never stored or logged; the worker records only the provider and a short fingerprint. Your messages then reach your own provider accounts (OpenRouter or the Vercel AI Gateway, and TypeSafe for Jev) under those accounts' data settings. Product, codebase, and Gym lookups still answer from our records, with the message embedded and judged on your keys, and nothing runs on our keys (#10176).
- On a computer (the desktop app or `openagents chat`), or from a phone paired with one, the request also names that computer by the label you gave it and the chat's project folder, with the folder's path only from the computer itself. Only the chat worker and the chat model read them, so the chat knows it is on your computer and can say which folder it works in (#10077).
- The phone shows only answers signed by the worker's key, tagged to its own request and device.
- Each message is signed by your device key; the relay is relay.openagents.com, and the chat events it carries to and from our chat worker are ephemeral, so it keeps none of them.
- In the Mac app, Terminal, and phone app, your conversations are saved on your device, encrypted with its key, not on our servers. When a phone reaches a computer through the relay, the relay holds those messages, encrypted, for that phone and computer only.
- In the web chat on openagents.com, the website holds the key that signs for you, so it reads your messages, and it saves each conversation in a private Google Cloud Storage bucket. Only the browser with that chat's cookie opens it, and our team can read the bucket. There's no time limit, and you can't delete a web chat yourself yet.
- We don't use chats to train models.
- There is no account, sign-in, or key to paste: the device key made on first launch signs the request, and the app holds no model key.
- Jev's doors, in order: the Vercel AI Gateway (`typesafe-ai/jev`), then OpenRouter, then TypeSafe direct.
- The worker's usage log records each job's time, key, surface, route, model, and timings, never the message text.
- When Coder runs on your computer, the coding agent sends what it reads to its model provider, under your sign-in there.

## Sources

- `docs/deployment/chat-worker.md`
- `INVARIANTS.md`
- `nips/openagents/NIP-CJ.md`
- `crates/openagents-web/src/chat_store.rs`
