---
id: openagents.coder-chat-controls
version: 2
kind: product
title: "Following up, queuing, steering, and stopping Coder"
summary: >-
  In a Coder chat, messages continue or queue, a long press on send steers or
  stops, and questions and approvals are answered in the chat.
tags: [coder, chat, queue, steer, stop, approve, in-app]
applies_when: >-
  The user asks how to send a follow-up, queue or steer a message while Coder
  works, stop a run, edit the queue, or answer Coder's questions and approval
  requests.
answer: >-
  In a Coder chat, a message after Coder finishes continues the same task, and
  a message while it works queues for its next turn. A long press on send
  offers the other ways to send: queue for the next turn, steer now, or stop
  and send. Edit queue changes what's waiting. When Coder asks a question or
  asks to approve a step, you answer in the chat, and Stop ends the run.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/docs/chat-later.md
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-09: tagged in-app: its steps are screens of the OpenAgents app, so the website's chat never shows the answer whole."
---

## Answer

In a Coder chat, a message after Coder finishes continues the same task, and a message while it works queues for its next turn. A long press on send offers the other ways to send: queue for the next turn, steer now, or stop and send. Edit queue changes what's waiting. When Coder asks a question or asks to approve a step, you answer in the chat, and Stop ends the run.

## Details

- Every message is a durable NIP-HOST `task.command`.
- The queue panel holds an edit lease, so nothing runs a message while you edit it.
- An answer is data for Coder and widens nothing it may do.

## Sources

- `bins/openagents-ios/docs/chat-later.md`
- `bins/openagents-ios/README.md`
