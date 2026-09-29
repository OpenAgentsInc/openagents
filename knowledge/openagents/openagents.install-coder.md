---
id: openagents.install-coder
version: 1
kind: product
title: "Installing Coder on a computer"
summary: >-
  Coder installs from a checkout of the open-source repository with
  scripts/install-coder.sh; the host then serves the phone.
tags: [install, coder, computer, setup, cli]
applies_when: >-
  The user asks how to install or set up Coder, the coder command, or the
  Coder host on their computer.
answer: >-
  Coder installs from a checkout of our open-source repository: run
  `./scripts/install-coder.sh`, which builds Coder and puts `coder` on your
  path; `--rollback` switches back to the build it replaced. Then run `coder
  host serve --tailnet-admission standard` so your phone can reach the
  computer over your tailnet.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/coder/guides/install.md
    - bins/openagents-ios/README.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
---

## Answer

Coder installs from a checkout of our open-source repository: run `./scripts/install-coder.sh`, which builds Coder and puts `coder` on your path; `--rollback` switches back to the build it replaced. Then run `coder host serve --tailnet-admission standard` so your phone can reach the computer over your tailnet.

## Details

- The script builds `crates/coder` in release mode with the pinned toolchain and links `~/.openagents/bin/coder` to the new build.
- `coder --version` and `coder doctor` show which build runs.
- It expects `~/.openagents/bin` on your `PATH`.

## Sources

- `docs/coder/guides/install.md`
- `bins/openagents-ios/README.md`
