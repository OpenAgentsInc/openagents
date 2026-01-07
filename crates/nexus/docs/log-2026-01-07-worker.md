# Nexus Log - 2026-01-07 (Worker Bringup)

- Copied relay protocol helpers into `crates/nexus/src/protocol/` and added `Filter` matching in `crates/nexus/src/filter.rs`.
- Wired `crates/nexus/src/lib.rs` exports and added core crate dependencies for protocol + auth validation.
- Ported relay-worker Cloudflare code into `crates/nexus/worker/src/` with Nexus naming, env parsing, and updated NIP-11 output.
- Renamed the Durable Object to `NexusRelay` and aligned it with `NEXUS_RELAY` bindings.
- Removed placeholder gitkeep files once real sources landed.
- Generated `crates/nexus/worker/Cargo.lock` and verified `bun run build` for the worker.
