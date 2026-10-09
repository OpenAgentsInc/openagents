---
id: openagents.agent-payments
version: 2
kind: product
title: "Approving payments your agents ask for"
summary: >-
  An agent on your computer can ask the phone to pay; nothing pays until you
  approve it, with Face ID above ₿1,000.
tags: [wallet, agents, spending, approve, payments, in-app]
applies_when: >-
  The user asks how agents pay for things, how to approve or deny an agent's
  payment request, or how to stop a computer from asking for payments.
answer: >-
  An agent on one of your computers can ask your phone to pay an invoice. The
  request shows as a Payment request sheet with the computer, task, purpose,
  payee, amount, fee, and what that computer's grant has left. Nothing pays
  until you tap Approve, and above ₿1,000 Approve asks for Face ID or your
  passcode. Deny refuses it, and Stop payment requests revokes that computer.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
    - docs/breez/spend-protocol.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

An agent on one of your computers can ask your phone to pay an invoice. The request shows as a Payment request sheet with the computer, task, purpose, payee, amount, fee, and what that computer's grant has left. Nothing pays until you tap Approve, and above ₿1,000 Approve asks for Face ID or your passcode. Deny refuses it, and Stop payment requests revokes that computer.

## Details

- The Wallet's Agent payments section lists requests and which computers may ask.
- The phone checks every request against its own grant for that computer and its ledger.

## Sources

- `bins/openagents-ios/README.md`
- `docs/breez/spend-protocol.md`
