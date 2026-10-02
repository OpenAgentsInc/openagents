---
id: openagents.overview
version: 3
kind: product
title: "What OpenAgents is"
summary: >-
  OpenAgents is a phone app, on iPhone and Android, for chatting with us and
  commanding your own computers, with a shared world, a bitcoin wallet, and
  open-source code.
tags: [overview, app, product, off-computer]
applies_when: >-
  The user asks what OpenAgents is, what the app is for, or what it does
  overall; not who is answering in this chat, and not which AI model powers
  the chat.
answer: >-
  OpenAgents is an app for iPhone and Android. In Chat you talk with us;
  Coder, our coding agent, handles work on a computer you've connected with
  OpenAgents for Mac. It also has Verse, the shared Grid; a bitcoin Wallet;
  and Account for computers, keys, and reports. Everything is open source.
  At openagents.com/download, Mac 1.0.0-rc.2 has a .dmg and Terminal
  1.0.0-rc.2 has macOS, Linux, and Windows installers. Build iPhone,
  Android, and Linux and Windows desktop apps from source.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - bins/openagents-ios/README.md
    - bins/openagents-android/README.md
    - crates/openagents-mobile/src/account.rs
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - crates/openagents-web/src/pages/download.rs
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: updated for QR pairing with OpenAgents for Mac, which replaced the Tailscale setup (#9978), and checked against the cited documents (#9995); the answer text awaits the owner's copy review."
  - "2026-10-01: We checked the download page and corrected the installation guidance."
  - "2026-10-01: The page offers Mac and Terminal release candidates. All other apps require source builds."
---

## Answer

OpenAgents is an app for iPhone and Android. In Chat you talk with us; Coder,
our coding agent, handles work on a computer you've connected with OpenAgents
for Mac. It also has Verse, the shared Grid; a bitcoin Wallet; and Account for
computers, keys, and reports. Everything is open source. At
openagents.com/download, Mac 1.0.0-rc.2 has a .dmg and Terminal 1.0.0-rc.2 has
macOS, Linux, and Windows installers. Build iPhone, Android, and Linux and
Windows desktop apps from source.

## Details

- The iPhone and Android apps run the same Rust library, `openagents-mobile`, with thin native hosts.
- Build 18's changelog names the first tab **Chat**: you chat with OpenAgents, which speaks as "we", and work for a computer is dispatched to Coder there.
- OpenAgents for Mac pairs a Mac with the phone by a QR code.
- At openagents.com/download, Mac 1.0.0-rc.2 has a .dmg, and Terminal
  1.0.0-rc.2 has installers for macOS, Linux, and Windows.
- Build iPhone, Android, and the Linux and Windows desktop apps from
  source at https://github.com/OpenAgentsInc/openagents.

## Sources

- `bins/openagents-ios/README.md`
- `bins/openagents-android/README.md`
- `crates/openagents-mobile/src/account.rs`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `crates/openagents-web/src/pages/download.rs`
