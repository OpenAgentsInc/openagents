---
id: openagents.delete-chats
version: 2
kind: product
title: "Deleting and archiving web chats"
summary: >-
  On openagents.com a chat is deleted from its menu or its page, Delete
  all chats removes every one, and Archive hides a chat without deleting
  it.
tags: [chat, delete, archive, history, website, privacy]
applies_when: >-
  The user asks how to delete a chat, delete all their chats or their chat
  history, remove a conversation from our servers, archive or hide a chat,
  or what happens to a deleted chat on openagents.com. Not whether we keep
  or train on chats in general (openagents.chat-privacy).
answer: >-
  On openagents.com, open a chat's menu in the left panel and choose
  **Delete**, or use Delete on the chat's page; it's removed from our servers
  right away. **Delete all chats** is in Settings when you're signed in
  (https://openagents.com/settings), or on a chat's delete step when you're
  not. Our storage provider may keep a recoverable copy for up to 7 days. To
  hide a chat without deleting it, choose **Archive**; Archived chats lists
  those.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/chat.rs
    - crates/openagents-web/src/pages/chat_delete_all.rs
    - crates/openagents-web/src/pages/chat_sidebar.rs
    - knowledge/openagents/openagents.chat-privacy.md
evidence:
  - "2026-10-09: written from the cited code and the privacy note and checked against them (#11036, #11038, chat goldens); the answer text awaits the owner's copy review."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
---

## Answer

On openagents.com, open a chat's menu in the left panel and choose **Delete**, or use Delete on the chat's page; it's removed from our servers right away. **Delete all chats** is in Settings when you're signed in (https://openagents.com/settings), or on a chat's delete step when you're not. Our storage provider may keep a recoverable copy for up to 7 days. To hide a chat without deleting it, choose **Archive**; Archived chats lists those.

## Details

- Chats you don't delete stay; there's no time limit yet.
- Signed in, deleting a chat removes it from your account in every browser.
- Deleting a Coder chat synced from the terminal deletes it on that computer too.

## Sources

- `crates/openagents-web/src/pages/chat.rs`
- `crates/openagents-web/src/pages/chat_delete_all.rs`
- `crates/openagents-web/src/pages/chat_sidebar.rs`
- `knowledge/openagents/openagents.chat-privacy.md`
