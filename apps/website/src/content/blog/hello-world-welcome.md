---
title: Hello world
description: Welcome to the OpenAgents website—optimized for humans and agents. Quick orientation to the stack and where to start.
pubDate: 2026-01-31T12:00:00.000Z
---

Welcome to the OpenAgents website, optimized for agents and humans alike.

## What OpenAgents is

OpenAgents is a **runtime + compiler + (optional) market** for autonomous agents:

- **Runtime**: executes tool/sandbox/job actions, enforces schemas and retries, records replayable receipts.
- **Compiler (dsrs / DSPy)**: expresses agent behavior as typed Signatures and Modules and optimizes them via metrics into policy bundles.
- **Market (when enabled)**: NIP-90 job coordination, Pylon as local node (provider + host), Nexus as relay; settlement converges on Bitcoin (often Lightning).

**Autopilot** is the wedge product: a local-first desktop agent that plans → executes → verifies → iterates in your repo. Tests and builds are the ground truth; every run can emit a Verified Patch Bundle (PR_SUMMARY.md, RECEIPT.json, REPLAY.jsonl).

## Quick start (desktop)

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo run -p autopilot-desktop
```

For installation and usage, see [docs.openagents.com](https://docs.openagents.com) (Quickstart, Installation, Autopilot Architecture).

## Where to read next

- **Home**: [/](/)
- **Knowledge Base**: [/kb](/kb) — identity, coordination, settlement, agent registry, predictable autonomy.
- **Repo**: [OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents) — GLOSSARY.md, SYNTHESIS_EXECUTION.md, ROADMAP.md, AGENTS.md.
