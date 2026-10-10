---
id: openagents.products
version: 1
kind: product
title: "What OpenAgents offers"
summary: >-
  OpenAgents is an open network of agents you work with through one
  conversation, reached on the website, in your terminal with Coder, and
  on your iPhone in beta, plus a public API, plugins, and the Verse.
tags: [overview, products, app, website, api, iphone, terminal]
applies_when: >-
  The user asks what products, apps, or services we have or make, what
  OpenAgents offers, or what they can use or get from us; not what
  OpenAgents is in general, what this chat can do, or which tools or
  plugins we have.
answer: >-
  We're OpenAgents, an open network of agents you work with through one
  conversation. Use it on the web at https://openagents.com; in your
  terminal with Coder, our coding agent: `curl -fsSL
  https://openagents.com/cli/install.sh | sh`; or on your iPhone with our
  beta app: https://testflight.apple.com/join/dvQdns5B. There's also a
  public API (https://openagents.com/docs/api), plugins that add
  abilities, and the Verse, a shared world. What works today:
  https://openagents.com/promises. What's next:
  https://openagents.com/roadmap.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/download.rs
    - crates/openagents-web/content/docs/iphone.md
    - docs/api/README.md
    - crates/openagents-web/content/docs/plugins.md
    - crates/openagents-web/content/docs/verse.md
    - crates/openagents-web/src/promises.rs
evidence:
  - "2026-10-09: v1: written from the cited documents for 'what products do you have', which the chat answered with a model reply saying we had no documented list; it matches the prepared answer meta.products and its variants, and names no third-party model or company as ours."
---

## Answer

We're OpenAgents, an open network of agents you work with through one conversation. Use it on the web at https://openagents.com; in your terminal with Coder, our coding agent: `curl -fsSL https://openagents.com/cli/install.sh | sh`; or on your iPhone with our beta app: https://testflight.apple.com/join/dvQdns5B. There's also a public API (https://openagents.com/docs/api), plugins that add abilities, and the Verse, a shared world. What works today: https://openagents.com/promises. What's next: https://openagents.com/roadmap.

## Details

- The website: chat without an account, or sign in with GitHub to keep your chats and add repositories as projects.
- Coder: our coding agent, in your terminal on your own computer. Windows, in PowerShell: `irm https://openagents.com/cli/install.ps1 | iex`. All downloads: https://openagents.com/download.
- The iPhone app is a TestFlight beta. The Android and desktop apps are built from source for now, at https://github.com/OpenAgentsInc/openagents.
- The public API: https://openagents.com/docs/api.
- Plugins add abilities to Coder, like handing a task to Claude Code or Codex: https://openagents.com/docs/plugins.
- The Verse is a shared world, in the phone and desktop apps: https://openagents.com/docs/verse.

## Sources

- `crates/openagents-web/src/pages/download.rs`
- `crates/openagents-web/content/docs/iphone.md`
- `docs/api/README.md`
- `crates/openagents-web/content/docs/plugins.md`
- `crates/openagents-web/content/docs/verse.md`
- `crates/openagents-web/src/promises.rs`
