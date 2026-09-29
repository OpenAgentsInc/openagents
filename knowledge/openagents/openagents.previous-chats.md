---
id: openagents.previous-chats
version: 1
kind: product
title: "Finding earlier chats"
summary: >-
  Earlier chats are behind the menu button at the top left of the Chat tab;
  only OpenAgents and Coder chats are listed.
tags: [chat, history, previous, menu]
applies_when: >-
  The user asks where their earlier or previous chats are, how to reopen a
  conversation, or why Claude Code, Codex, OpenCode, or Devin sessions aren't
  listed.
answer: >-
  Earlier chats are behind the menu button at the top left of the Chat tab,
  newest first: your conversations with us and Coder's tasks on your
  computers. Only OpenAgents and Coder chats are listed, not Claude Code,
  Codex, OpenCode, or Devin sessions. When a Coder task handed work to
  OpenCode or Devin, that session shows inside its chat.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-mobile/src/account.rs
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Earlier chats are behind the menu button at the top left of the Chat tab, newest first: your conversations with us and Coder's tasks on your computers. Only OpenAgents and Coder chats are listed, not Claude Code, Codex, OpenCode, or Devin sessions. When a Coder task handed work to OpenCode or Devin, that session shows inside its chat.

## Details

- The list paints from what the phone kept while the computers are read again, so earlier chats open quickly.
- Chat reads go directly over the tailnet, with the relay as a fallback.

## Sources

- `crates/openagents-mobile/src/account.rs`
- `bins/openagents-ios/README.md`
