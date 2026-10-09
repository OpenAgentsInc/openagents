---
id: openagents.chat-and-coder
version: 3
kind: product
title: "Chatting with OpenAgents and sending Coder"
summary: >-
  We answer in the chat and can't reach a computer from it; Coder, our
  coding agent, works on a computer: in the terminal from the download
  page, or sent from the phone app to a paired computer.
tags: [chat, coder, dispatch, computer, off-computer]
applies_when: >-
  The user asks the difference between chatting with OpenAgents and Coder,
  what Coder is, or when Coder gets involved; not how to connect a computer,
  and not what model the chat uses.
answer: >-
  In this chat you talk with us, OpenAgents: we answer questions and help
  you plan, but from the chat we can't run code, read files, or reach your
  computer. Coder is our coding agent, and it does that work on a
  computer. From openagents.com, get Coder at openagents.com/download and
  run it in your terminal. From the phone app, we send Coder to a computer
  you've paired, with the conversation as its task. Coder uses that
  computer's own git and GitHub login.
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
  - "2026-10-09: v3 (chat goldens): the web chat sends Coder nowhere (docs/web/cloud-reset.md); on openagents.com Coder is the terminal agent from the download page, so the answer says both ways."
---

## Answer

In this chat you talk with us, OpenAgents: we answer questions and help you plan, but from the chat we can't run code, read files, or reach your computer. Coder is our coding agent, and it does that work on a computer. From openagents.com, get Coder at openagents.com/download and run it in your terminal. From the phone app, we send Coder to a computer you've paired, with the conversation as its task. Coder uses that computer's own git and GitHub login.

## Details

- Chatting needs no computer. Coder runs on a computer only when you ask, as a NIP-HOST task whose prompt is the conversation so far.
- The app picks the target from the screen's controls, never by reading your message.
- Without a computer, the chat is the whole screen; the app offers to connect one when a message needs a computer.

## Sources

- `crates/openagents-mobile/src/basic_coder.rs`
- `crates/openagents-mobile/src/account.rs`
- `INVARIANTS.md`
