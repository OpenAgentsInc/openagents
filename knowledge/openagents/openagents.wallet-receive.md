---
id: openagents.wallet-receive
version: 2
kind: product
title: "Receiving bitcoin"
summary: >-
  Receive offers a Lightning invoice, a Spark address, a Bitcoin deposit
  address, and the device's npub, each with a QR code; there is no receiving
  Lightning address yet.
tags: [wallet, receive, invoice, address, qr, in-app]
applies_when: >-
  The user asks how to receive or get paid bitcoin, how to make an invoice,
  what address to share, or whether they have a Lightning address.
answer: >-
  In the Wallet, choose Receive and pick a method: a Lightning invoice, where
  you can set an amount, your Spark address, or a Bitcoin deposit address,
  each with a QR code. You can also share this device's npub, and a switch
  publishes your Spark address in your Nostr profile so people can pay your
  npub. We don't offer a receiving Lightning address of your own yet.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

In the Wallet, choose Receive and pick a method: a Lightning invoice, where you can set an amount, your Spark address, or a Bitcoin deposit address, each with a QR code. You can also share this device's npub, and a switch publishes your Spark address in your Nostr profile so people can pay your npub. We don't offer a receiving Lightning address of your own yet.

## Details

- A receiving Lightning address needs its own LNURL server, which isn't built.
- On-chain deposits are claimed into the balance automatically when they mature.

## Sources

- `bins/openagents-ios/README.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `INVARIANTS.md`
