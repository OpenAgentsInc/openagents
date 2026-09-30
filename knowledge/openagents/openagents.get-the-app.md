---
id: openagents.get-the-app
version: 2
kind: product
title: "Getting the apps"
summary: >-
  The iPhone app installs from TestFlight, and OpenAgents for Mac from its
  .dmg; Android is still in testing.
tags: [install, download, testflight, mac, desktop, dmg, android]
applies_when: >-
  The user asks how to download or install the app on iPhone, Mac, or Android,
  where to get the desktop app, or whether it's in the App Store or Play
  Store.
answer: >-
  On iPhone, OpenAgents installs from our TestFlight link. To connect your
  Mac, get OpenAgents for Mac: open the downloaded .dmg and drag OpenAgents
  onto Applications, then open it and scan its QR code with your phone. It
  needs macOS 13 or later. The Android app is still in testing and not public,
  and desktop builds for Linux and Windows aren't published yet.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/install.rs
    - crates/openagents-desktop/README.md
    - docs/desktop/release.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
---

## Answer

On iPhone, OpenAgents installs from our TestFlight link. To connect your Mac, get OpenAgents for Mac: open the downloaded .dmg and drag OpenAgents onto Applications, then open it and scan its QR code with your phone. It needs macOS 13 or later. The Android app is still in testing and not public, and desktop builds for Linux and Windows aren't published yet.

## Details

- The Mac app is one universal build for Apple silicon and Intel, signed by OpenAgents, Inc. and notarized by Apple, and it updates itself.
- Our install page gathers both downloads in order.

## Sources

- `crates/openagents-web/src/pages/install.rs`
- `crates/openagents-desktop/README.md`
- `docs/desktop/release.md`
