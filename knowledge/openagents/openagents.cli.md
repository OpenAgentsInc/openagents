---
id: openagents.cli
version: 3
kind: product
title: "The openagents command"
summary: >-
  openagents is one command-line program that reaches every OpenAgents surface
  over Nostr, with a help table per group, JSON output, and an MCP server.
tags: [cli, command-line, openagents, terminal, mcp]
applies_when: >-
  The user asks what the openagents command or CLI is, what it can do, or how
  to install it; not a request to run a specific command.
answer: >-
  `openagents` is our command-line program for everything over Nostr:
  computers and pairing (connect, host, pair, computer, session), Coder tasks, Verse,
  XP, and the Lagrange zone, the Gym, labor orders, keys, your wallet
  and x402, the knowledge base, relays, and playtest triage. `openagents
  COMMAND --help` shows each group's syntax, `--json` makes output
  machine-readable, and `openagents mcp serve` offers it as MCP tools.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/cli/README.md
    - crates/openagents-cli/src/main.rs
    - docs/coder/guides/link-devices.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-09-29: updated for QR pairing with OpenAgents for Mac, which replaced the Tailscale setup (#9978), and checked against the cited documents (#9995); the answer text awaits the owner's copy review."
  - "2026-10-02: the wallet in the command list is the person's own wallet, not a Lightning node (#10202)."
---

## Answer

`openagents` is our command-line program for everything over Nostr: computers and pairing (connect, host, pair, computer, session), Coder tasks, Verse, XP, and the Lagrange zone, the Gym, labor orders, keys, your wallet and x402, the knowledge base, relays, and playtest triage. `openagents COMMAND --help` shows each group's syntax, `--json` makes output machine-readable, and `openagents mcp serve` offers it as MCP tools.

## Details

- Build and install from a checkout: `cargo build --release -p openagents-cli`, then install `target/release/openagents` on your `PATH`.
- `openagents connect invite` shows a pairing code for a computer without a screen, and `openagents connect --ssh HOST` sets up one you reach over SSH.
- `openagents doctor` shows the identities, stores, and relays it uses.
- No command prints a secret key after the moment it's created.

## Sources

- `docs/cli/README.md`
- `crates/openagents-cli/src/main.rs`
- `docs/coder/guides/link-devices.md`
