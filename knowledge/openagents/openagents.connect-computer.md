---
id: openagents.connect-computer
version: 2
kind: product
title: "Connecting your phone to a computer"
summary: >-
  OpenAgents for Mac shows a QR code; scanning it with the iPhone Camera or in
  the app under Account > Computers > Connect a computer pairs the phone, with
  no Tailscale and no commands.
tags: [computer, connect, pair, phone, mac, qr, nearby, setup]
applies_when: >-
  The user asks how to connect, add, link, or pair their phone with their Mac
  or another computer so Coder can run there, or how the QR code, nearby
  pairing, or a copied code works. Not whether Tailscale is needed
  (openagents.tailnet) and not where to download the apps
  (openagents.get-the-app).
answer: >-
  Open OpenAgents for Mac and it shows a QR code. Scan it with your iPhone
  Camera, or in our app tap Account, Computers, Connect a computer and point
  it at the code. Both screens then say the computer is connected. No
  Tailscale and no commands. On the same Wi-Fi, the Mac can also show up under
  Nearby: tap it, check both screens show the same six-digit code, and click
  Connect on the Mac. Can't scan? Click Copy a code instead and paste it in
  the app.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/coder/guides/link-devices.md
    - docs/coder/design/2026-09-29-auto-pairing.md
    - crates/openagents-desktop/README.md
    - crates/openagents-web/src/pages/download.rs
    - nips/openagents/NIP-HOST.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
---

## Answer

Open OpenAgents for Mac and it shows a QR code. Scan it with your iPhone Camera, or in our app tap Account, Computers, Connect a computer and point it at the code. Both screens then say the computer is connected. No Tailscale and no commands. On the same Wi-Fi, the Mac can also show up under Nearby: tap it, check both screens show the same six-digit code, and click Connect on the Mac. Can't scan? Click Copy a code instead and paste it in the app.

## Details

- Every pairing gets full permission, a terminal included; nothing asks first. To take a phone's access away, click **Remove** next to it in the desktop app.
- The phone connects directly when it can and through our relay (iroh.openagents.com) when it can't, so it works on the same Wi-Fi and on mobile data.
- The code changes every minute, works once, and shows only while the app's window is open on an unlocked screen.
- On a computer without a screen, `openagents connect invite` prints the same code in the terminal, and `openagents connect --ssh HOST` sets up a computer you reach over SSH.
- A computer set up the old way is upgraded silently when the desktop app opens, keeping its paired phones; the old setup commands were removed (#9978).
- Tailscale is an optional route for people who already use it (openagents.tailnet), never required.

## Sources

- `docs/coder/guides/link-devices.md`
- `docs/coder/design/2026-09-29-auto-pairing.md`
- `crates/openagents-desktop/README.md`
- `crates/openagents-web/src/pages/download.rs`
- `nips/openagents/NIP-HOST.md`
- `INVARIANTS.md`
