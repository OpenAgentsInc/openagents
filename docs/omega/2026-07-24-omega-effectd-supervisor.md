# Omega-effectd Rust supervisor (FA-02)

- Date: 2026-07-24
- Packet: `OMEGA-FA-02`
- Omega issue: [OpenAgentsInc/omega#21](https://github.com/OpenAgentsInc/omega/issues/21)
- Package: `@openagentsinc/omega-effectd` `0.1.0`
- Pack SHA-256: `20e4e002202b6db640501301f134e0a9f0f8718fe1c1dc38bef4aa5ab2d69ffe`
- Protocol: `openagents.omega.effectd.v1` (newline-framed JSON on stdio)
- Freeze: [2026-07-24-full-auto-contract-freeze.md](./2026-07-24-full-auto-contract-freeze.md)
- Extract: [2026-07-24-omega-effectd-extract.md](./2026-07-24-omega-effectd-extract.md)

## Result

`omega-effectd` speaks a framed stdio protocol for Omega Rust supervision.

- Methods: `initialize`, `health`, `list_runs`, `get_run`, `start`, `pause`,
  `resume`, `handoff`, `stop`, `retry`
- A manual handoff is legal only while paused. It rechecks the target lane
  against the live Omega host, durably rebinds both run and dispatch profiles,
  and records a bounded provider-handoff transition.
- Generation fencing refuses stale supervisor generations
- Durable runs stay under `{dataRoot}/full-auto/runs.json`
- Run projections omit objective and transcript (redacted for supervisor
  events)
- Mutation API remains `full-auto-run-actions`
- Generation-fenced reverse host calls replace fabricated workspace, thread,
  lane, turn, interruption, and evidence state
- The service limits frames to 64 KiB. Stale and late host replies fail closed.

Omega crate `omega_effectd` can start, health-check, restart, and stop the
service. Crash recovery re-reads disk truth. GPUI is not run authority.

## Verification

- `pnpm --dir packages/omega-effectd test` — 197 passed
- `cargo test -p omega_effectd` in Omega — supervisor start/health/restart/stop
  and disk recovery

## Component release

OpenAgents released the service and the fixed Node.js runtime as
[`omega-effectd-v0.1.0-rc.1`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.1).
The archive SHA-256 is
`52fb8333ee65b944ba47b2ec00abc77b3826aa7f9a4cacc3ca6f7d37e139ffa5`.
The Omega RC packager must verify and install this asset at the component path.
Until an installed Omega candidate has the component, the panels report that
the component is unavailable.

The first component candidate did not expose the existing manual-handoff
authority over the framed protocol. Issue
[#9215](https://github.com/OpenAgentsInc/openagents/issues/9215) adds that
missing packaged boundary. The replacement component receipt supersedes the
RC.1 input for the final Omega candidate.

The GPUI launcher landed in
[2026-07-24-omega-full-auto-gpui-launcher.md](./2026-07-24-omega-full-auto-gpui-launcher.md).
