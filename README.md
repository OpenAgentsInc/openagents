# OpenAgents

OpenAgents is currently scoped to the **Autopilot MVP**.

Autopilot is a desktop-first personal agent that runs on your machine, uses local compute and tools, and can both do useful work for you and earn Bitcoin when you opt into provider mode.

The MVP centers on one irreducible outcome: a user can go online, receive paid NIP-90 work, see sats land in their Spark wallet, and successfully withdraw over Lightning. If that loop is not real and repeatable, the product is not complete.

This repository exists to deliver that loop with clear authority, deterministic behavior, and a fast WGPUI-native desktop experience.

The product authority is [docs/MVP.md](docs/MVP.md).
Ownership boundaries are defined in [docs/OWNERSHIP.md](docs/OWNERSHIP.md).

## Current Repository Scope

- Active implementation is focused on `crates/wgpui`.
- This repository is intentionally pruned for MVP execution speed.
- Most historical services, docs, and supporting code were moved to a backroom archive and are restored only when explicitly directed.

## Working Principles

- Retained implementation is Rust/WGPUI-first.
- Sync and state continuity must remain deterministic and replay-safe.
- Wallet and payout state must be explicit and truthful in UI and behavior.
- Shared dependency hygiene is guarded by `scripts/lint/workspace-dependency-drift-check.sh`.
- Architecture boundary hygiene is guarded by `scripts/lint/ownership-boundary-check.sh`.
- Clean-on-touch clippy hygiene is guarded by `scripts/lint/touched-clippy-gate.sh` with tracked debt in `scripts/lint/clippy-debt-allowlist.toml`.
- Repo-managed Agent Skills validation is guarded by `scripts/skills/validate_registry.sh`.

For contributor guardrails and scope rules, see [AGENTS.md](AGENTS.md).
