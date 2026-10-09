---
id: openagents.overview
version: 6
kind: product
title: "What OpenAgents is"
summary: >-
  OpenAgents is a chat on openagents.com, Coder for the terminal, and
  phone and desktop apps, all open source.
tags: [overview, app, product, website, off-computer]
applies_when: >-
  The user asks what OpenAgents is, what the product or the app is for, or
  what it does overall; not who is answering in this chat, and not which
  AI model powers the chat.
answer: >-
  OpenAgents is where you chat with us and get work done on your code. On
  openagents.com you can chat without an account, or sign in with GitHub to
  keep your chats and add your repositories as projects. Coder, our coding
  agent, runs in your terminal on your own computer: get it at
  https://openagents.com/download. Our phone and desktop apps add Verse, the
  shared Grid, and a bitcoin Wallet. Everything is open source, at
  https://github.com/OpenAgentsInc/openagents.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - README.md
    - docs/web/cloud-reset.md
    - crates/openagents-web/src/pages/download.rs
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: updated for QR pairing with OpenAgents for Mac, which replaced the Tailscale setup (#9978), and checked against the cited documents (#9995); the answer text awaits the owner's copy review."
  - "2026-10-01: We checked the download page and corrected the installation guidance."
  - "2026-10-01: The page offers Mac and Terminal release candidates. All other apps require source builds."
  - "2026-10-09: v4 (chat goldens): the download page offers Coder, not a Mac .dmg, and the website has accounts and projects, so the answer leads with the website and Coder."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
  - "2026-10-09: v6: the iPhone app ships on TestFlight (https://testflight.apple.com/join/dvQdns5B, the link the download page and /connect give), not as a source build; checked against the cited sources."
---

## Answer

OpenAgents is where you chat with us and get work done on your code. On openagents.com you can chat without an account, or sign in with GitHub to keep your chats and add your repositories as projects. Coder, our coding agent, runs in your terminal on your own computer: get it at https://openagents.com/download. Our phone and desktop apps add Verse, the shared Grid, and a bitcoin Wallet. Everything is open source, at https://github.com/OpenAgentsInc/openagents.

## Details

- Environments, where Claude Code runs on your repositories in the cloud, are in early testing; they'll come with our Pro plan.
- The iPhone app is in beta on TestFlight at https://testflight.apple.com/join/dvQdns5B; the Android and desktop apps are built from source for now.

## Sources

- `README.md`
- `docs/web/cloud-reset.md`
- `crates/openagents-web/src/pages/download.rs`
- `bins/openagents-ios/README.md`
