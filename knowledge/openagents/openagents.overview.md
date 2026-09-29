---
id: openagents.overview
version: 1
kind: product
title: "What OpenAgents is"
summary: >-
  OpenAgents is a phone app, on iPhone and Android, for chatting with us and
  commanding your own computers, with a shared world, a bitcoin wallet, and
  open-source code.
tags: [overview, app, product]
applies_when: >-
  The user asks what OpenAgents is, what the app is for, or what it does
  overall; not who is answering in this chat, and not which AI model powers
  the chat.
answer: >-
  OpenAgents is an app for your phone, on iPhone and Android. In the Chat tab
  you talk with us, and when something needs a computer we dispatch Coder, our
  coding agent, to a computer you've connected. The app also has Verse, a
  shared world called the Grid; a bitcoin Wallet; and Account, where you
  manage computers, keys, and reports. Everything behind it is open source.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
    - bins/openagents-android/README.md
    - crates/openagents-mobile/src/account.rs
    - docs/roadmap/2026-09-29-launch-roadmap.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

OpenAgents is an app for your phone, on iPhone and Android. In the Chat tab you talk with us, and when something needs a computer we dispatch Coder, our coding agent, to a computer you've connected. The app also has Verse, a shared world called the Grid; a bitcoin Wallet; and Account, where you manage computers, keys, and reports. Everything behind it is open source.

## Details

- The iPhone and Android apps run the same Rust library, `openagents-mobile`, with thin native hosts.
- Build 18's changelog names the first tab **Chat**: you chat with OpenAgents, which speaks as "we", and work for a computer is dispatched to Coder there.
- Playtesters get the iPhone app from a public TestFlight link and the Android app as a signed APK.

## Sources

- `bins/openagents-ios/README.md`
- `bins/openagents-android/README.md`
- `crates/openagents-mobile/src/account.rs`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
