---
id: openagents.get-the-app
version: 9
kind: product
title: "Getting the apps"
summary: >-
  openagents.com/download offers Coder and the openagents command-line
  program for macOS, Linux, and Windows, and the iPhone app's TestFlight
  beta; the Android and desktop apps are built from source for now.
tags: [install, download, terminal, mac, desktop, android, iphone]
applies_when: >-
  The user asks how to download or install the app on iPhone, Mac,
  Windows, Linux, or Android, where to get the desktop app, or whether
  it's in the App Store or Play Store.
answer: >-
  https://openagents.com/download has Coder, our coding agent for your
  terminal, for macOS, Linux, and Windows; the iPhone app is in beta on
  TestFlight, and you can chat with us right here.
ui: |
  root = Stack([apps, source])
  apps = Columns([coder, phone])
  coder = Card("Coder for your terminal", [Text("Installs with the openagents command-line program."), Command("curl -fsSL https://openagents.com/cli/install.sh | bash", windows="irm https://openagents.com/cli/install.ps1 | iex"), Button("Download page", href="/download", style="secondary")])
  phone = Card("iPhone", [Text("The app is in beta on TestFlight. Open the link on your iPhone."), Button("Join the beta", href="https://testflight.apple.com/join/dvQdns5B")])
  source = Text("The Android and desktop apps aren't on the download page yet; build them from source at https://github.com/OpenAgentsInc/openagents.")
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/download.rs
    - scripts/install/coder.sh
    - crates/openagents-web/src/pages/connect.rs
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
  - "2026-10-01: the download page moved from openagents.com/install to openagents.com/download (owner-directed), so the answer names it and the cited page is `download.rs`; checked against it."
  - "2026-10-01: We checked the download page and corrected the installation guidance."
  - "2026-10-01: The page offers Mac and Terminal release candidates. All other apps require source builds."
  - "2026-10-09: v5 (chat goldens): the download page now offers only Coder and the OpenAgents command-line program (download.rs: Download Coder), not a Mac .dmg, so the answer says so."
  - "2026-10-09: v6: the install command's result is Coder and the `openagents` command; the engine Coder runs with is part of Coder, not a separate download."
  - "2026-10-09: the answer gives the exact page or the one command to run for each thing it tells the reader to do (the owner's rule of 2026-10-09), checked against the cited sources."
  - "2026-10-09: v8: the iPhone app ships on TestFlight (https://testflight.apple.com/join/dvQdns5B, the link the download page and /connect give), not as a source build; checked against the cited sources."
  - "2026-10-09: v9 (#11187): a short answer with components (ui): the install command for each system with Copy, the download page, the TestFlight link, and the source link; checked against the cited sources."
---

## Answer

https://openagents.com/download has Coder, our coding agent for your terminal, with the `openagents` command-line program, for macOS, Linux, and Windows, each installed with one command. On macOS or Linux, run `curl -fsSL https://openagents.com/cli/install.sh | bash`. The iPhone app is in beta on TestFlight: open https://testflight.apple.com/join/dvQdns5B on your iPhone. The Android and desktop apps aren't on the download page right now; you can build them from source at https://github.com/OpenAgentsInc/openagents. And you can chat with us right here on openagents.com.

## Details

- macOS and Linux: `curl -fsSL https://openagents.com/cli/install.sh | bash`. Windows, in PowerShell: `irm https://openagents.com/cli/install.ps1 | iex`.
- The page's command installs `coder` and the `openagents` command together; run it again to update.

## Sources

- `crates/openagents-web/src/pages/download.rs`
- `scripts/install/coder.sh`
- `crates/openagents-web/src/pages/connect.rs`
