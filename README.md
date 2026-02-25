# OpenAgents

OpenAgents builds **Autopilot**: a personal agent that can code and do many other tasks. It is extensible through upgrades in an open marketplace, so capabilities can expand over time as new modules and services are adopted.

Under the hood, the system is built around a shared runtime/control model so agent sessions can be observed, replayed, and administered across surfaces instead of being tied to one client.

## Apps

[apps/autopilot-desktop/](apps/autopilot-desktop/) is the native Rust/WGPUI desktop app where users run and operate their personal agent, including local workflows and runtime-authenticated execution sessions.

[apps/openagents.com/](apps/openagents.com/) contains the Rust control service and web distribution surface, including auth/session APIs, sync token issuance, and the landing/download experience.

[apps/runtime/](apps/runtime/) is the Rust runtime authority service that handles execution lifecycle, worker orchestration, event/replay semantics, and sync delivery paths.

[apps/lightning-ops/](apps/lightning-ops/) is the Rust operational service/CLI for Lightning/L402 policy and reconciliation workflows, including staging/smoke runbooks.

[apps/lightning-wallet-executor/](apps/lightning-wallet-executor/) is the Rust HTTP payment execution service for wallet actions such as BOLT11 pay, invoice creation, and on-chain send flows.

Optional: add your resources to the network by enrolling devices as **OpenAgents Compute** providers (provider mode, Pylon), earning bitcoin and strengthening supply liquidity.

Core services:
- Runtime authority: `apps/runtime/`
- Control service + site host: `apps/openagents.com/service/`

## Run Locally

```bash
# Runtime service
cargo run --manifest-path apps/runtime/Cargo.toml --bin openagents-runtime-service

# Control service
cargo run --manifest-path apps/openagents.com/service/Cargo.toml

# Desktop app
cargo run -p autopilot-desktop
```

For architecture, contracts, and contributor docs, start with `AGENTS.md`.
