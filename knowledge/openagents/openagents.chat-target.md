---
id: openagents.chat-target
version: 2
kind: product
title: "Starting a new chat, and where it goes"
summary: >-
  New chat (Cmd+N on the desktop, /new in OpenAgents Terminal) starts a
  chat with OpenAgents; coding work in it runs Coder on your computer.
tags: [chat, new chat, terminal, computer]
applies_when: >-
  The user asks how to start a new chat or thread, where a new message goes,
  or how to send a message to a particular computer.
answer: >-
  Start one with New chat: Cmd+N in the desktop app, or /new in OpenAgents
  Terminal. Every chat goes to us, OpenAgents; when a message is coding work,
  Coder runs on your computer for it. There's nothing to pick.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-chat-app/src/commands.rs
    - crates/openagents-terminal/src/slash.rs
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents (#9923)."
  - "2026-10-01: rewritten: the selector beside the title and the workspace chips above the composer were removed on 2026-09-29 (#9962), and the terminal's /new was added (#10111)."
---

## Answer

Start one with New chat: Cmd+N in the desktop app, or /new in OpenAgents Terminal. Every chat goes to us, OpenAgents; when a message is coding work, Coder runs on your computer for it. There's nothing to pick.

## Details

- No selector or target chips: since #9962 a chat starts on a computer only through Coder, when the router reads a message as coding work.
- OpenAgents Terminal opens on a new thread; `--continue` reopens the last one in that folder, and Ctrl+T lists them.

## Sources

- `crates/openagents-chat-app/src/commands.rs`
- `crates/openagents-terminal/src/slash.rs`
- `INVARIANTS.md`
