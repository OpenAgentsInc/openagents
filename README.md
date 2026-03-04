# OpenAgents

OpenAgents builds Autopilot: a personal agent for your computer. It does useful work for you and, in provider mode, sells spare compute for Bitcoin.

The MVP centers on one irreducible outcome: a user can go online, receive paid NIP-90 work, see sats land in their Spark wallet, and successfully withdraw over Lightning. If that loop is not real and repeatable, the product is not complete.

This repository exists to deliver that loop with clear authority, deterministic behavior, and a fast WGPUI-native desktop experience.

The product authority is [docs/MVP.md](docs/MVP.md).
Ownership boundaries are defined in [docs/OWNERSHIP.md](docs/OWNERSHIP.md).

## Earning Bitcoin (WIP)

Autopilot Earn starts with the simplest possible market: spare compute. You run the desktop app, press `Go Online`, and offer idle CPU/GPU capacity to paid NIP-90 jobs. A buyer posts work, your machine executes locally, and settlement happens over Lightning. The first loop is intentionally concrete: compute goes in, result comes out, sats hit the wallet.

That narrow loop is the point of the MVP. The product has to prove, quickly and honestly, that paid jobs exist and payouts are real. If work appears completed but wallet-confirmed sats do not arrive, it does not count as successful earnings. This is why the current Earn surface is deliberately focused and still marked WIP.

From there, the model expands from a single job type into a broader provider economy. Compute is the first lane, not the last one. Future lanes can include roles like liquidity solver participation under Hydra, where providers contribute capital plus execution and earn routing fees/spreads in an OpenAgents-native solver market. The architecture stays the same: intent-driven work, deterministic receipts, and explicit payouts.

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
