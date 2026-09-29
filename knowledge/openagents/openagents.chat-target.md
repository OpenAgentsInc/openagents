---
id: openagents.chat-target
version: 1
kind: product
title: "Where a new chat goes"
summary: >-
  A new chat goes to OpenAgents even with a computer connected; Coder runs on
  a computer only when you pick it in the selector or tap one of its
  workspaces.
tags: [chat, selector, cloud, workspace, computer]
applies_when: >-
  The user asks where a new message goes, how to send a message to a
  particular computer or workspace, what Cloud means in the selector, or what
  the chips above the composer do.
answer: >-
  A new chat goes to us, OpenAgents, even when a computer is connected. Coder
  runs on a computer only when you pick that computer in the selector beside
  the title or tap one of its workspaces in the chips above the composer. The
  selector also offers Cloud, which is this chat with us, and a way to connect
  a computer. Other chips continue your recent chats.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-mobile/src/account.rs
    - bins/openagents-ios/README.md
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

A new chat goes to us, OpenAgents, even when a computer is connected. Coder runs on a computer only when you pick that computer in the selector beside the title or tap one of its workspaces in the chips above the composer. The selector also offers Cloud, which is this chat with us, and a way to connect a computer. Other chips continue your recent chats.

## Details

- Build 19 changed this: before it, a new chat went to a ready computer as a Coder task.
- A chat on a computer is a NIP-HOST `task.create` in the workspace you picked.
- The workspace this phone used last comes first among the chips.
- With no computer added, a chip offers to connect one, which opens Account > Computers.

## Sources

- `crates/openagents-mobile/src/account.rs`
- `bins/openagents-ios/README.md`
- `INVARIANTS.md`
