---
id: openagents.get-the-app
version: 4
kind: product
title: "Getting the apps"
summary: >-
  The download page offers the Mac .dmg and Terminal installers. All other
  apps require source builds.
tags: [install, download, terminal, mac, desktop, dmg, android]
applies_when: >-
  The user asks how to download or install the app on iPhone, Mac, or Android,
  where to get the desktop app, or whether it's in the App Store or Play
  Store.
answer: >-
  Get our apps at openagents.com/download: OpenAgents for Mac 1.0.0-rc.2 is
  a .dmg, and OpenAgents Terminal 1.0.0-rc.2 has installers for macOS,
  Linux, and Windows. Build iPhone, Android, and the Linux and Windows
  desktop apps from source. On macOS 13 or later, open the .dmg, drag
  OpenAgents onto Applications, then open it and scan its QR code with your
  phone.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/download.rs
    - crates/openagents-desktop/README.md
    - docs/desktop/release.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
  - "2026-10-01: the download page moved from openagents.com/install to openagents.com/download (owner-directed), so the answer names it and the cited page is `download.rs`; checked against it."
  - "2026-10-01: We checked the download page and corrected the installation guidance."
  - "2026-10-01: The page offers Mac and Terminal release candidates. All other apps require source builds."
---

## Answer

Get our apps at openagents.com/download: OpenAgents for Mac 1.0.0-rc.2 is a
.dmg, and OpenAgents Terminal 1.0.0-rc.2 has installers for macOS, Linux, and
Windows. Build iPhone, Android, and the Linux and Windows desktop apps from
source. On macOS 13 or later, open the .dmg, drag OpenAgents onto Applications,
then open it and scan its QR code with your phone.

## Details

- The Mac app is one universal build for Apple silicon and Intel, signed by OpenAgents, Inc. and notarized by Apple, and it updates itself.
- Our download page, openagents.com/download, offers Mac 1.0.0-rc.2 as a .dmg
  and Terminal 1.0.0-rc.2 installers for macOS, Linux, and Windows.
- Build iPhone, Android, and the Linux and Windows desktop apps from
  source at https://github.com/OpenAgentsInc/openagents.

## Sources

- `crates/openagents-web/src/pages/download.rs`
- `crates/openagents-desktop/README.md`
- `docs/desktop/release.md`
