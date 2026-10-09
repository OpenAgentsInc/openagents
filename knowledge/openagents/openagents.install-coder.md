---
id: openagents.install-coder
version: 5
kind: product
title: "Getting Coder on a computer"
summary: >-
  Coder installs with one command from openagents.com/download on macOS,
  Linux, and Windows, together with the openagents command-line program
  and Microcoder; then run coder in a project folder.
tags: [install, coder, computer, setup, download, terminal]
applies_when: >-
  The user asks how to install, download, update, or set up Coder, the
  coder command, or the Coder terminal on their computer, or how to start
  using Coder. Not how to sign Coder in to their account
  (openagents.coder-sync).
answer: >-
  Install Coder from openagents.com/download. On macOS or Linux, run `curl
  -fsSL https://openagents.com/cli/install.sh | bash`; on Windows, in
  PowerShell, run `irm https://openagents.com/cli/install.ps1 | iex`. It
  installs `coder`, the `openagents` command, and `microcoder` together.
  Then run `coder` from your project folder. Run the install command again
  to update.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - crates/openagents-web/src/pages/download.rs
    - scripts/install/coder.sh
    - crates/coder-new/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: rewritten from the cited documents for QR pairing with OpenAgents for Mac, which replaced the Tailscale and eight-character-code setup (#9978), and checked against them (#9995); the answer text awaits the owner's copy review."
  - "2026-10-01: version 3 (#10091): Grok Build is allowed by default beside Codex and Claude Code, on the Mac's own runs and its phone switch, so any of the three signed in there runs Coder; checked against INVARIANTS.md and docs/cli/settings.md."
  - "2026-10-01: version 4 (#10101): a coding reply on the phone starts Coder on the Mac at once, as on the Mac itself, unless the Mac's Coder setting is Ask first; checked against INVARIANTS.md and docs/cli/settings.md."
  - "2026-10-09: v5 (chat goldens): the download page offers the Coder terminal and the OpenAgents command-line program with one-line installers, not the Mac app, so the answer gives the install commands from download.rs; checked against it and scripts/install/coder.sh."
---

## Answer

Install Coder from openagents.com/download. On macOS or Linux, run `curl -fsSL https://openagents.com/cli/install.sh | bash`; on Windows, in PowerShell, run `irm https://openagents.com/cli/install.ps1 | iex`. It installs `coder`, the `openagents` command, and `microcoder` together. Then run `coder` from your project folder. Run the install command again to update.

## Details

- The installers verify SHA-256 checksums before installing and put the commands in `~/.openagents/bin`, adding it to your PATH.
- The download page also lists each platform's files for a manual install.
- On Windows, local task services and background automation need macOS or Linux for now.
- Signed in with `coder login`, `/sync on` saves Coder's chats to your openagents.com account.

## Sources

- `crates/openagents-web/src/pages/download.rs`
- `scripts/install/coder.sh`
- `crates/coder-new/README.md`
