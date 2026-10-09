---
id: openagents.identity-keys
version: 2
kind: product
title: "Your device's identity key"
summary: >-
  Account > Identity keys shows this phone's Nostr key; the nsec is revealed
  only behind a warning, isn't derived from recovery words, and must never be
  shared.
tags: [keys, identity, nostr, npub, nsec, security, in-app]
applies_when: >-
  The user asks where their keys are, what their npub or nsec is, how to back
  up the device key, or whether it's safe to share a key.
answer: >-
  Account > Identity keys shows this phone's Nostr key: the public key (npub),
  with Copy. The secret key (nsec) stays hidden until you tap Reveal nsec and
  confirm a warning, and a copied nsec stays on the pasteboard for only 60
  seconds. The key was made at random on this phone and isn't derived from
  recovery words, so the nsec is its only backup. Anyone with it can act as
  this device on your computers, so never share it.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

Account > Identity keys shows this phone's Nostr key: the public key (npub), with Copy. The secret key (nsec) stays hidden until you tap Reveal nsec and confirm a warning, and a copied nsec stays on the pasteboard for only 60 seconds. The key was made at random on this phone and isn't derived from recovery words, so the nsec is its only backup. Anyone with it can act as this device on your computers, so never share it.

## Details

- The key is kept in the phone's Keychain, this device only, not synchronized.
- The screen hides the nsec again when you leave it or the app goes to the background.
- About this device shows the same public key and the app's version.

## Sources

- `bins/openagents-ios/README.md`
- `INVARIANTS.md`
