---
id: openagents.coder-sync
version: 3
kind: product
title: "Signing in to Coder and syncing its chats"
summary: >-
  `coder login` signs Coder in to an openagents.com account with a short
  code; `/sync on` then saves Coder's chats to the account, where they show
  in the website's left panel and can be answered from the website while
  Coder is open.
tags: [coder, login, sign-in, sync, device, terminal, chats, website]
applies_when: >-
  The user asks how to sign in or log in to Coder or the coder terminal with
  their OpenAgents account, what the code at openagents.com/device is, how
  to see or sync their Coder chats on openagents.com, how to reply to a
  Coder chat from the website, how to stop syncing or delete synced chats,
  or how to sign Coder out. Not how to install Coder
  (openagents.install-coder).
answer: >-
  Sign Coder in with `coder login` and approve its code at
  https://openagents.com/device; then `/sync on` shows its chats in the
  website's left panel, where you can reply while Coder is open.
ui: |
  root = Stack([steps, more])
  steps = Steps([login, approve, sync])
  login = Step("Sign in from your terminal", [CodeBlock("coder login", "bash"), Text("Or type /login inside Coder.")])
  approve = Step("Approve its code while signed in", [Button("Approve sign-in", href="/device")])
  sync = Step("Save its chats to your account", [CodeBlock("/sync on")])
  more = Text("/sync all adds earlier chats, /sync off stops, and /sync delete removes this computer's chats. Messages that look like they hold a password or key are left out.")
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/coder-new/README.md
    - docs/auth/README.md
    - crates/openagents-web/src/coder_sync.rs
evidence:
  - "2026-10-09: written from the cited documents and code and checked against them (#11045, #11046, #11047, #11048, chat goldens); the answer text awaits the owner's copy review."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
  - "2026-10-09: v3 (#11187): a short answer with components (ui): `coder login`, an Approve sign-in button for openagents.com/device, and `/sync on`, each copyable; checked against the cited sources."
---

## Answer

Run `coder login` (or type `/login` in Coder) and approve its short code at https://openagents.com/device while signed in. Then type `/sync on` in Coder to save its chats to your account: they show in the website's left panel with your computer's name. `/sync all` adds earlier chats, `/sync off` stops, and `/sync delete` removes this computer's chats. Messages that look like they hold a password or key are left out. While Coder is open with sync on, you can reply to those chats from the website.

## Details

- The code is eight letters, good for 10 minutes. The website asks "Sign in to Coder on" your computer's name, with Approve and Deny.
- Coder's sign-in lasts 30 days and is listed in Settings under computers signed in to Coder, where Remove ends it. `coder logout` or `/logout` ends it from the computer.
- Sync is off by default. Deleting a synced chat in Coder or on the website deletes it in both places.
- A reply sent from the website waits until Coder is free: Coder opens that chat, shows the reply as your message, and answers it with that computer's tools. When Coder isn't open, the chat's page says so and offers no reply box.

## Sources

- `crates/coder-new/README.md`
- `docs/auth/README.md`
- `crates/openagents-web/src/coder_sync.rs`
