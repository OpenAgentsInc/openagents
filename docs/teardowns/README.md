# Product Teardowns

This directory holds point-in-time, read-only teardowns of installed products
that are close architectural references for OpenAgents. The goal is not feature
admiration or pixel copying. Each teardown separates observed evidence from
inference, then asks which boundaries, workflows, and failure modes should
change OpenAgents product decisions.

## Current set

| Teardown | Subject | Central finding |
| --- | --- | --- |
| [ChatGPT desktop app](./2026-07-10-chatgpt-desktop-app-teardown.md) | OpenAI's ChatGPT/Codex macOS app | A closed agent host on the Owl Chromium/Electron-compat runtime, with the open Rust Codex engine, plugins, skills, computer use, remote control, and ambient screen-memory components |
| [Claude desktop app](./2026-07-10-claude-desktop-app-teardown.md) | Anthropic's Claude macOS app | Stock Electron orchestrating a live/bundled web UI, Claude Code over stdio JSON, MCP/skills, native bridges, computer use, and a hardware-isolated Cowork VM |
| [OpenAgents adaptation analysis](./2026-07-10-openagents-product-adaptation-analysis.md) | Cross-teardown synthesis | Keep stock Electron and the hardened Effect Native boundary; adopt a versioned engine protocol, explicit execution isolation, open extension compatibility, signed component updates, and cross-device typed authority while rejecting ambient surveillance and remote-renderer authority |

## Evidence convention

The dated teardowns use these labels where applicable:

- **`[bundle]`** — observed in the installed signed application bundle
- **`[runtime]`** — observed in live process, UI, network-listener, or
  names-only filesystem state
- **`[public]`** — corroborated by a linked public source

Bundle metadata and compiled public strings can reveal architecture, but they
do not prove that every dormant feature is enabled or that a remote service
behaves as its client suggests. Runtime observations are snapshots, not ongoing
monitoring. Private credentials, conversation contents, and user-data payloads
do not belong in these documents.

## How to use these documents

Treat the teardowns as design evidence, not current OpenAgents status. The
authorities for implementation state and sequencing remain:

- [Sol master roadmap](../sol/MASTER_ROADMAP.md)
- [OpenAgents Desktop guarantees](../../apps/openagents-desktop/GUARANTEES.md)
- [OpenAgents Desktop README](../../apps/openagents-desktop/README.md)
- current code, tests, receipts, issues, and runtime evidence

When a teardown lesson becomes a product requirement, move it into the owning
typed contract, roadmap gate, issue, and verification surface. Do not leave a
load-bearing decision only in competitive analysis.
