---
id: openagents.wallet-send
version: 2
kind: product
title: "Sending bitcoin"
summary: >-
  Send takes a pasted or scanned invoice, Lightning address, LNURL code, npub,
  or Spark or Bitcoin address, and pays only after you confirm the amount and
  fee.
tags: [wallet, send, pay, invoice, scan, in-app]
applies_when: >-
  The user asks how to send, pay, or withdraw bitcoin, pay an invoice or a
  Lightning address, or pay another person by npub.
answer: >-
  In the Wallet, choose Send, then paste or scan what you're paying: a
  Lightning invoice, a Lightning address, an LNURL code, an npub, or a Spark
  or Bitcoin address. A confirm screen shows the amount and the fee, and
  nothing is sent until you confirm it. An npub is paid at the address its
  owner published. On-chain withdrawals offer three speeds.
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

In the Wallet, choose Send, then paste or scan what you're paying: a Lightning invoice, a Lightning address, an LNURL code, an npub, or a Spark or Bitcoin address. A confirm screen shows the amount and the fee, and nothing is sent until you confirm it. An npub is paid at the address its owner published. On-chain withdrawals offer three speeds.

## Details

- A payment is sent only for the quote on screen, once.
- A Lightning address shows its range and can take a comment.
- Paying an npub is never a zap; the confirm screen names the person, the address, and where it came from.

## Sources

- `bins/openagents-ios/README.md`
- `INVARIANTS.md`
