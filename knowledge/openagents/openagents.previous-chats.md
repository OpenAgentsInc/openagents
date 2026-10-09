---
id: openagents.previous-chats
version: 3
kind: product
title: "Finding earlier chats"
summary: >-
  On openagents.com earlier chats are in the left panel, with pin, rename,
  archive, move, and delete in each chat's menu; in the phone app they are
  behind the Chat tab's menu button.
tags: [chat, history, previous, menu, sidebar, archive, website]
applies_when: >-
  The user asks where their earlier or previous chats are, how to reopen,
  find, search, pin, rename, archive, or unarchive a conversation, or why
  Claude Code, Codex, OpenCode, or Devin sessions aren't listed. Not
  deleting chats or whether we keep them (openagents.chat-privacy).
answer: >-
  On openagents.com your chats are in the left panel, newest first, under
  their project when they have one. Each chat's menu can pin, rename,
  archive, move it to a project, or delete it; Archived chats lists the
  archived ones, and Search chats finds one. In the phone app, earlier
  chats are behind the menu button at the top left of the Chat tab. Only
  OpenAgents and Coder chats are listed, not Claude Code, Codex, OpenCode,
  or Devin sessions.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/chat_sidebar.rs
    - docs/web/sidebar.md
    - crates/openagents-mobile/src/account.rs
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: updated for QR pairing with OpenAgents for Mac, which replaced the Tailscale setup (#9978), and checked against the cited documents (#9995); the answer text awaits the owner's copy review."
  - "2026-10-09: v3 (chat goldens): adds the website's left panel, its row menu (Pin, Rename, Archive, Move to project, Delete), and the Archived chats and Search chats pages (chat_sidebar.rs)."
---

## Answer

On openagents.com your chats are in the left panel, newest first, under their project when they have one. Each chat's menu can pin, rename, archive, move it to a project, or delete it; Archived chats lists the archived ones, and Search chats finds one. In the phone app, earlier chats are behind the menu button at the top left of the Chat tab. Only OpenAgents and Coder chats are listed, not Claude Code, Codex, OpenCode, or Devin sessions.

## Details

- Signed in, your chats follow your account to any browser where you sign in; signed out, they belong to this browser.
- With Coder signed in and `/sync on`, Coder's chats show in the left panel too, marked with the computer's name.
- In the phone app, the list paints from what the phone kept while the computers are read again, so earlier chats open quickly.

## Sources

- `crates/openagents-web/src/pages/chat_sidebar.rs`
- `docs/web/sidebar.md`
- `crates/openagents-mobile/src/account.rs`
- `bins/openagents-ios/README.md`
