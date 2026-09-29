---
id: openagents.tailnet
version: 1
kind: product
title: "The Tailnet screen and Tailscale"
summary: >-
  The Tailnet screen signs the app in to Tailscale only to list the tailnet's
  devices; access to a computer comes from the computer's own invitation.
tags: [tailnet, tailscale, network, security]
applies_when: >-
  The user asks what the Tailnet screen does, why Tailscale is needed, or
  whether signing in to Tailscale gives the app access to their computers.
answer: >-
  Your phone reaches your computers over your Tailscale tailnet, so the phone
  needs the Tailscale app and the computer must be on the same tailnet. The
  Tailnet screen signs the app in to Tailscale only to list the tailnet's
  devices and ask your computers for an invitation. A computer answers only a
  device that Tailscale says belongs to its own user, and the sign-in itself
  grants no access to any computer.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
    - nips/openagents/NIP-HOST.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Your phone reaches your computers over your Tailscale tailnet, so the phone needs the Tailscale app and the computer must be on the same tailnet. The Tailnet screen signs the app in to Tailscale only to list the tailnet's devices and ask your computers for an invitation. A computer answers only a device that Tailscale says belongs to its own user, and the sign-in itself grants no access to any computer.

## Details

- The app registers as its own tailnet node, `openagents-ios`, with Tailscale's Rust control client and reads one netmap; it never joins the data plane or carries traffic.
- The node keys stay in the app's Application Support directory, so later launches skip the sign-in until the node key expires.
- Computers never uses this sign-in.

## Sources

- `bins/openagents-ios/README.md`
- `nips/openagents/NIP-HOST.md`
