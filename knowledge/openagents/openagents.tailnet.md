---
id: openagents.tailnet
version: 2
kind: product
title: "Tailscale and the Tailnet screen"
summary: >-
  Tailscale isn't needed: phones pair by scanning the desktop app's QR code.
  Tailscale stays an optional route, and the Tailnet screen only lists a
  tailnet's devices.
tags: [tailnet, tailscale, network, security]
applies_when: >-
  The user asks whether they need Tailscale, what the Tailnet screen does, or
  whether signing in to Tailscale gives the app access to their computers. Not
  the steps to connect a computer (openagents.connect-computer).
answer: >-
  No, you don't need Tailscale. Your phone connects to your computer by
  scanning the QR code in OpenAgents for Mac, directly when it can and through
  our relay when it can't, on Wi-Fi or mobile data. Tailscale stays an
  optional route if you already use it: the Tailnet screen in Account signs
  the app in to Tailscale only to list your tailnet's devices, and that
  sign-in grants no access to any computer.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/coder/guides/link-devices.md
    - docs/coder/design/2026-09-29-auto-pairing.md
    - nips/openagents/NIP-HOST.md
    - INVARIANTS.md
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
---

## Answer

No, you don't need Tailscale. Your phone connects to your computer by scanning the QR code in OpenAgents for Mac, directly when it can and through our relay when it can't, on Wi-Fi or mobile data. Tailscale stays an optional route if you already use it: the Tailnet screen in Account signs the app in to Tailscale only to list your tailnet's devices, and that sign-in grants no access to any computer.

## Details

- A tailnet address only introduces a device; access comes only from a grant the computer signs.
- A computer's owner can turn on tailnet admission when serving the host; it is off by default and optional.
- The app registers as its own tailnet node, `openagents-ios`, and reads one netmap; it never carries traffic.

## Sources

- `docs/coder/guides/link-devices.md`
- `docs/coder/design/2026-09-29-auto-pairing.md`
- `nips/openagents/NIP-HOST.md`
- `INVARIANTS.md`
- `bins/openagents-ios/README.md`
