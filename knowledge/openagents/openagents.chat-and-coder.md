---
id: openagents.chat-and-coder
version: 2
kind: product
title: "Chatting with OpenAgents and dispatching Coder"
summary: >-
  The Chat tab talks with OpenAgents, which can't reach your computer; Coder
  is the coding agent we dispatch to a computer you've connected when a
  message needs one.
tags: [chat, coder, dispatch, computer, off-computer]
applies_when: >-
  The user asks the difference between chatting with OpenAgents and Coder,
  what Coder is, or when Coder gets involved; not how to connect a computer,
  and not what model the chat uses.
answer: >-
  In the Chat tab you talk with us, OpenAgents. We answer questions and help
  you plan, but from the chat we can't run code, read files, or reach your
  computer. Coder is our coding agent: when a message needs a computer, such
  as changing a repository or running commands, we send Coder to a
  computer you've connected, with the conversation as its task. Coder uses
  that computer's own git and GitHub login.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-mobile/src/basic_coder.rs
    - crates/openagents-mobile/src/account.rs
    - INVARIANTS.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-09: version 2 (#11031) says it in plain words, without internal terms."
---

## Answer

In the Chat tab you talk with us, OpenAgents. We answer questions and help you plan, but from the chat we can't run code, read files, or reach your computer. Coder is our coding agent: when a message needs a computer, such as changing a repository or running commands, we send Coder to a computer you've connected, with the conversation as its task. Coder uses that computer's own git and GitHub login.

## Details

- Chatting needs no computer. Coder runs on a computer only when you ask, as a NIP-HOST task whose prompt is the conversation so far.
- The app picks the target from the screen's controls, never by reading your message.
- Without a computer, the chat is the whole screen; the app offers to connect one when a message needs a computer.

## Sources

- `crates/openagents-mobile/src/basic_coder.rs`
- `crates/openagents-mobile/src/account.rs`
- `INVARIANTS.md`
