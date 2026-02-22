# OpenAgents

OpenAgents is an open platform for running and supervising AI agents across web, desktop, and iOS.

The system is built around a shared runtime/control model so agent sessions can be observed, replayed, and administered across surfaces instead of being tied to one client.

Use OpenAgents:
- Web: [openagents.com](https://openagents.com)
- Desktop client: `apps/autopilot-desktop/`
- iOS client: `apps/autopilot-ios/`

Core services:
- Runtime authority: `apps/runtime/`
- Control service + site host: `apps/openagents.com/service/`
- Web shell: `apps/openagents.com/web-shell/`

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
