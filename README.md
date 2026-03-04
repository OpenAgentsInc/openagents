# OpenAgents

OpenAgents builds Autopilot: a personal agent for your computer. It does useful work for you and performs paid jobs on the network for Bitcoin.

The MVP centers on one irreducible outcome: a user goes online, receives paid NIP-90 work, sees sats land in their Spark wallet, and withdraws over Lightning. The product delivers this loop as a real, repeatable path.

This repository exists to deliver that loop with clear authority, deterministic behavior, and a fast WGPUI-native desktop experience.

The product authority is [docs/MVP.md](docs/MVP.md).
Ownership boundaries are defined in [docs/OWNERSHIP.md](docs/OWNERSHIP.md).

## Earning Bitcoin (WIP)

Autopilot Earn starts with spare compute. You run the desktop app, press `Go Online`, and offer idle CPU/GPU capacity to paid NIP-90 jobs. A buyer posts work, your machine executes locally, and settlement happens over Lightning.

MVP completion means this loop works end to end with clear proof in-app: job lifecycle, payment settlement, and wallet-confirmed earnings. The first release is deliberately focused so users can reach first sats fast and repeat that path reliably.

From there, the model expands from a single job type into a broader provider economy. Compute is lane one. Next lanes can include liquidity solver participation under Hydra, where providers contribute capital plus execution and earn routing fees/spreads in an OpenAgents-native solver market. The architecture stays the same: intent-driven work, deterministic receipts, and explicit payouts.

For setup expectations, current limitations, and source-of-truth behavior, see the user guide: [docs/EARN.md](docs/EARN.md).

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
- Codex chat/skills regression coverage: `cargo test -p autopilot-desktop codex_lane`, `cargo test -p autopilot-desktop assemble_chat_turn_input`, and `cargo test -p codex-client --test skills_and_user_input`.

For contributor guardrails and scope rules, see [AGENTS.md](AGENTS.md).
