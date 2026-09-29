---
id: openagents.nostr-and-relay
version: 1
kind: product
title: "Nostr and relay.openagents.com"
summary: >-
  The app talks over Nostr; relay.openagents.com carries chat jobs, Grid
  presence, XP, and playtest records, and keys made on the phone replace
  accounts.
tags: [nostr, relay, protocol, account]
applies_when: >-
  The user asks what Nostr is, what relay.openagents.com does, or whether they
  need an account or password.
answer: >-
  The app talks over Nostr, an open protocol of signed events. Our relay,
  relay.openagents.com, carries your chat with our chat worker, presence in
  the Grid, XP and trainer records, and playtest records. Chat messages cross
  it encrypted. The app signs with keys it makes on your phone, so there's no
  account or password.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/deployment/chat-worker.md
    - docs/verse/mobile.md
    - docs/verse/tutorial-quests.md
    - docs/game/playtesting.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

The app talks over Nostr, an open protocol of signed events. Our relay, relay.openagents.com, carries your chat with our chat worker, presence in the Grid, XP and trainer records, and playtest records. Chat messages cross it encrypted. The app signs with keys it makes on your phone, so there's no account or password.

## Details

- The device key authenticates to the relay with NIP-42.
- The protocols OpenAgents authored are under `nips/openagents/`.

## Sources

- `docs/deployment/chat-worker.md`
- `docs/verse/mobile.md`
- `docs/verse/tutorial-quests.md`
- `docs/game/playtesting.md`
