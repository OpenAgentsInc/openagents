# OpenAgents

OpenAgents is currently scoped to the **Autopilot MVP**.

Autopilot is a desktop-first personal agent with a simple core loop:
- use the agent locally
- go online to accept NIP-90 work
- get paid in sats to a Spark wallet
- withdraw over Lightning

The product authority is [docs/MVP.md](docs/MVP.md).

## Current Repository Scope

- Active implementation is focused on `crates/wgpui`.
- This repository is intentionally pruned for MVP execution speed.
- Most historical services, docs, and supporting code were moved to backroom and are restored only when explicitly directed.

Backroom archive root:
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp`

## Working Principles

- Retained implementation is Rust/WGPUI-first.
- Sync and state continuity must remain deterministic and replay-safe.
- Wallet and payout state must be explicit and truthful in UI and behavior.

For contributor guardrails and scope rules, see [AGENTS.md](AGENTS.md).
