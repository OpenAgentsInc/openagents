---
id: openagents.overview
version: 9
kind: product
title: "What OpenAgents is"
summary: >-
  OpenAgents is an open network of agents you work with through one
  conversation on the web, in the terminal, on desktop, and on your phone;
  each message goes to whatever in the network serves it best, Coder does
  the code work, and plugins add abilities.
tags: [overview, app, product, website, off-computer]
applies_when: >-
  The user asks what OpenAgents is, what the product or the app is for, or
  what it does overall; not who is answering in this chat, not which
  AI model powers the chat, and not which products or apps we offer
  (openagents.products).
answer: >-
  OpenAgents is an open network of agents you work with through one
  conversation: on the web, in your terminal, on desktop, and on your
  phone. Ask a question, plan, or write, and each message goes to whatever
  in the network serves it best: one of many models, a specialist agent, or
  a computer. Work in your code goes
  to Coder, our coding agent, which runs in your terminal on your own
  computer: get it at https://openagents.com/download. Plugins add new
  abilities, like handing work to Claude Code or Codex. Everything is open source, at
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
    - crates/coder-new/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: updated for QR pairing with OpenAgents for Mac, which replaced the Tailscale setup (#9978), and checked against the cited documents (#9995); the answer text awaits the owner's copy review."
  - "2026-10-01: We checked the download page and corrected the installation guidance."
  - "2026-10-01: The page offers Mac and Terminal release candidates. All other apps require source builds."
  - "2026-10-09: v4 (chat goldens): the download page offers Coder, not a Mac .dmg, and the website has accounts and projects, so the answer leads with the website and Coder."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
  - "2026-10-09: v6: the iPhone app ships on TestFlight (https://testflight.apple.com/join/dvQdns5B, the link the download page and /connect give), not as a source build; checked against the cited sources."
  - "2026-10-09: v7 (#11095): the answer to the starter question 'What is OpenAgents?': a general agent on web, terminal, desktop, and phone; Coder does the code work; many models, routed per message; plugins such as handing work to Claude Code or Codex. It matches the prepared answer meta.who."
  - "2026-10-09: v8: the owner's framing: OpenAgents is an open network of agents you work with through one conversation, not one agent or a general agent; each message goes to whatever in the network serves it best. It matches the prepared answer meta.who v4 and the What is OpenAgents doc."
  - "2026-10-09: v9: applies_when leaves 'what products do you have' to openagents.products; the answer is unchanged."
---

## Answer

OpenAgents is an open network of agents you work with through one conversation: on the web, in your terminal, on desktop, and on your phone. Ask a question, plan, or write, and each message goes to whatever in the network serves it best: one of many models, a specialist agent, or a computer. Work in your code goes to Coder, our coding agent, which runs in your terminal on your own computer: get it at https://openagents.com/download. Plugins add new abilities, like handing work to Claude Code or Codex. Everything is open source, at https://github.com/OpenAgentsInc/openagents.

## Details

- On openagents.com you can chat without an account, or sign in with GitHub to keep your chats and add your repositories as projects.
- Our phone and desktop apps add Verse, the shared Grid, and a bitcoin Wallet.
- Environments, where Claude Code runs on your repositories in the cloud, are in early testing; they'll come with our Pro plan.
- The iPhone app is in beta on TestFlight at https://testflight.apple.com/join/dvQdns5B; the Android and desktop apps are built from source for now.

## Sources

- `README.md`
- `docs/web/cloud-reset.md`
- `crates/openagents-web/src/pages/download.rs`
- `bins/openagents-ios/README.md`
- `crates/coder-new/README.md`
