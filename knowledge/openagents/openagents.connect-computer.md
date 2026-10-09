---
id: openagents.connect-computer
version: 4
kind: product
title: "Connecting a computer"
summary: >-
  A computer joins your account through Coder: install it with one command,
  run `coder login`, approve the code at https://openagents.com/device, and
  type `/sync on`. The phone app pairs with a computer by the QR code
  `openagents connect invite` draws.
tags: [computer, connect, pair, phone, mac, qr, nearby, setup, coder, login, sync]
applies_when: >-
  The user asks how to connect, add, or link a computer to OpenAgents or
  their account so Coder can run there, how to pair their phone with their
  Mac or another computer, or how the QR code, nearby pairing, or a copied
  code works. Not whether Tailscale is needed
  (openagents.tailnet) and not where to download the apps
  (openagents.get-the-app).
answer: >-
  Connect a computer by installing Coder on it. On macOS or Linux, run `curl
  -fsSL https://openagents.com/cli/install.sh | bash`; on Windows, in
  PowerShell, run `irm https://openagents.com/cli/install.ps1 | iex`. Then run
  `coder login`, approve its code at https://openagents.com/device, and type
  `/sync on` in Coder. Its chats then show in your left panel on
  openagents.com with the computer's name, and while Coder is open you can
  reply to them from the website.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/download.rs
    - crates/coder-new/src/account.rs
    - crates/coder-new/src/account_sync.rs
    - crates/openagents-web/src/device.rs
    - docs/coder/guides/link-devices.md
    - docs/coder/design/2026-09-29-auto-pairing.md
    - crates/openagents-cli/src/connect.rs
    - nips/openagents/NIP-HOST.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
  - "2026-10-09: v4: OpenAgents for Mac is not out at 1.0, so phone pairing names the commands that draw the code (`openagents host serve --iroh --control`, then `openagents connect invite`), checked against crates/openagents-cli/src/connect.rs."
---

## Answer

Connect a computer by installing Coder on it. On macOS or Linux, run `curl -fsSL https://openagents.com/cli/install.sh | bash`; on Windows, in PowerShell, run `irm https://openagents.com/cli/install.ps1 | iex`. Then run `coder login`, approve its code at https://openagents.com/device, and type `/sync on` in Coder. Its chats then show in your left panel on openagents.com with the computer's name, and while Coder is open you can reply to them from the website.

## Details

- `/login` inside Coder does the same as `coder login`. `/sync all` also adds earlier chats, `/sync off` stops, and `/sync delete` removes this computer's chats from your account.
- The phone app (iPhone, on TestFlight at https://testflight.apple.com/join/dvQdns5B) pairs with a computer by QR code: on the computer, start the host with `openagents host serve --iroh --control` and leave it running, then run `openagents connect invite` in a second terminal and scan its code from Account > Computers > Connect a computer. `openagents connect invite --text` also prints a code to paste under **Paste a code**. No Tailscale.
- Every phone pairing gets full permission, a terminal included; nothing asks first. To take a phone's access away, run `openagents connect remove DEVICE` on the computer (`openagents connect devices` lists them).
- The phone connects directly when it can and through our relay (iroh.openagents.com) when it can't, so it works on the same Wi-Fi and on mobile data.
- The pairing code works once, for one phone, and stops working when `openagents connect invite` ends.
- `openagents connect --ssh HOST` sets up a computer you reach over SSH and pairs with it.
- Tailscale is an optional route for people who already use it (openagents.tailnet), never required.

## Sources

- `crates/openagents-web/src/pages/download.rs`
- `crates/coder-new/src/account.rs`
- `crates/coder-new/src/account_sync.rs`
- `crates/openagents-web/src/device.rs`
- `docs/coder/guides/link-devices.md`
- `docs/coder/design/2026-09-29-auto-pairing.md`
- `crates/openagents-cli/src/connect.rs`
- `nips/openagents/NIP-HOST.md`
- `INVARIANTS.md`
