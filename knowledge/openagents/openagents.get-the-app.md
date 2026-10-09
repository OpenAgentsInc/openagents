---
id: openagents.get-the-app
version: 6
kind: product
title: "Getting the apps"
summary: >-
  openagents.com/download offers Coder and the openagents command-line
  program for macOS, Linux, and Windows; the phone and desktop apps are
  built from source for now.
tags: [install, download, terminal, mac, desktop, android, iphone]
applies_when: >-
  The user asks how to download or install the app on iPhone, Mac,
  Windows, Linux, or Android, where to get the desktop app, or whether
  it's in the App Store or Play Store.
answer: >-
  openagents.com/download has Coder, our coding agent for your terminal,
  with the `openagents` command-line program, for macOS, Linux, and
  Windows, each installed with one command. The iPhone, Android, and
  desktop apps aren't on the download page right now; you can build them
  from source at github.com/OpenAgentsInc/openagents. And you can chat
  with us right here on openagents.com.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/download.rs
    - scripts/install/coder.sh
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
  - "2026-10-01: the download page moved from openagents.com/install to openagents.com/download (owner-directed), so the answer names it and the cited page is `download.rs`; checked against it."
  - "2026-10-01: We checked the download page and corrected the installation guidance."
  - "2026-10-01: The page offers Mac and Terminal release candidates. All other apps require source builds."
  - "2026-10-09: v5 (chat goldens): the download page now offers only Coder and the OpenAgents command-line program (download.rs: Download Coder), not a Mac .dmg, so the answer says so."
  - "2026-10-09: v6: the install command's result is Coder and the `openagents` command; the engine Coder runs with is part of Coder, not a separate download."
---

## Answer

openagents.com/download has Coder, our coding agent for your terminal, with the `openagents` command-line program, for macOS, Linux, and Windows, each installed with one command. The iPhone, Android, and desktop apps aren't on the download page right now; you can build them from source at github.com/OpenAgentsInc/openagents. And you can chat with us right here on openagents.com.

## Details

- macOS and Linux: `curl -fsSL https://openagents.com/cli/install.sh | bash`. Windows, in PowerShell: `irm https://openagents.com/cli/install.ps1 | iex`.
- The page's command installs `coder` and the `openagents` command together; run it again to update.

## Sources

- `crates/openagents-web/src/pages/download.rs`
- `scripts/install/coder.sh`
