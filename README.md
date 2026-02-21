# OpenAgents

OpenAgents is a Rust-first, Rust-target architecture for multi-surface agent execution and administration.

Canonical architecture and migration sequencing live in:
- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`

## Active Surfaces and Services (Canonical)

- `apps/openagents.com/service/`: Rust control service (auth/session/control APIs + static host)
- `apps/openagents.com/web-shell/`: Rust/WGPUI WASM web application
- `apps/runtime/`: Rust runtime authority service (runs/workers/events/projectors/Khala delivery)
- `apps/autopilot-desktop/`: Rust desktop app (WGPUI)
- `apps/autopilot-ios/`: iOS host app for shared Rust client/runtime integration
- `apps/onyx/`: Rust local-first notes app
- `apps/lightning-ops/`: Rust Lightning operations service
- `apps/lightning-wallet-executor/`: Rust Lightning payment execution service

## Authority Model

- `control.*` data plane: identity/session/authorization authority
- `runtime.*` data plane: execution/sync/read-model authority
- Khala is delivery/replay infrastructure only; it is never an authority mutation path

## Historical/Removed Surfaces

These are removed from active architecture and treated as historical only:
- `apps/mobile/` (removed)
- `apps/desktop/` (removed)
- `apps/inbox-autopilot/` (removed)
- `apps/openagents-runtime/` (removed; superseded by `apps/runtime/`)

## Local Verification

- Changed-files gate: `./scripts/local-ci.sh changed`
- Rust pre-push gate: `./scripts/local-ci.sh all-rust`
- Workspace baseline: `cargo check --workspace --all-targets`
- Proto gate: `./scripts/local-ci.sh proto`

## Quick Runtime Commands

- Runtime service: `cargo run --manifest-path apps/runtime/Cargo.toml --bin openagents-runtime-service`
- Control service: `cargo run --manifest-path apps/openagents.com/service/Cargo.toml`
- Desktop app: `cargo run -p autopilot-desktop`

## Documentation Entry Points

- `AGENTS.md`
- `docs/README.md`
- `docs/PROJECT_OVERVIEW.md`
- `docs/ROADMAP.md`
