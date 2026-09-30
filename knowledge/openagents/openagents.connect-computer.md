---
id: openagents.connect-computer
version: 1
kind: product
title: "Connecting a Mac or Linux computer"
summary: >-
  A computer on your tailnet running the Coder host with tailnet admission
  adds itself after you sign in on the Tailnet screen; otherwise add it with
  an invitation or an 8-character code.
tags: [computer, connect, pair, mac, linux, tailnet, setup]
applies_when: >-
  The user asks how to connect, add, link, or pair their Mac, Linux machine,
  or other computer with the app so Coder can run there.
answer: >-
  Coder needs your own Mac or Linux computer. The quickest way: put the
  computer and your phone on the same Tailscale tailnet, run `coder host serve
  --tailnet-admission standard` on the computer, and sign in on the Tailnet
  screen in Account. The computer adds itself, with no QR code. You can also
  add one in Account > Computers > Add a computer with an invitation or an
  8-character code.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - nips/openagents/NIP-HOST.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Coder needs your own Mac or Linux computer. The quickest way: put the computer and your phone on the same Tailscale tailnet, run `coder host serve --tailnet-admission standard` on the computer, and sign in on the Tailnet screen in Account. The computer adds itself, with no QR code. You can also add one in Account > Computers > Add a computer with an invitation or an 8-character code.

## Details

- With tailnet admission, the app asks every device on the tailnet for an invitation; a host answers only a device that `tailscale whois` names as its own Tailscale user.
- The app redeems the invitation through normal NIP-HOST enrollment, so revocation and the device list work as usual, and the computer's Coder chats appear without a separate chat pairing.
- The phone must be on the tailnet, through the Tailscale app, to reach the computer.

## Sources

- `bins/openagents-ios/README.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `nips/openagents/NIP-HOST.md`
