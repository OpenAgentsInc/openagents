# OpenAgents

OpenAgents builds **Autopilot**: a cross-platform coding agent that turns repo issues into verified PRs, within a budget, with receipts + replay artifacts.

Under the hood, the system is built around a shared runtime/control model so agent sessions can be observed, replayed, and administered across surfaces instead of being tied to one client.

Surfaces (explicit non-parity):
- Desktop (execution): `apps/autopilot-desktop/`
- Web (distribution landing only): `apps/openagents.com/service/`
- Onyx (limited scope): `apps/onyx/`

Optional: add your resources to the network by enrolling devices as **OpenAgents Compute** providers (provider mode, Pylon), earning credits and strengthening supply liquidity.

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
