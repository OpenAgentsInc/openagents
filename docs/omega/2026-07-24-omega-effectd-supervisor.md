# Omega-effectd Rust supervisor (FA-02)

- Date: 2026-07-24
- Packet: `OMEGA-FA-02`
- Omega issue: [OpenAgentsInc/omega#21](https://github.com/OpenAgentsInc/omega/issues/21)
- Package: `@openagentsinc/omega-effectd` `0.1.0`
- Pack SHA-256: `4cc1cb2e5d71ff8af6f730248871ee779488a991f21a848880e885331ef31831`
- Protocol: `openagents.omega.effectd.v1` (newline-framed JSON on stdio)
- Freeze: [2026-07-24-full-auto-contract-freeze.md](./2026-07-24-full-auto-contract-freeze.md)
- Extract: [2026-07-24-omega-effectd-extract.md](./2026-07-24-omega-effectd-extract.md)

## Result

`omega-effectd` speaks a framed stdio protocol for Omega Rust supervision.

- Methods: `initialize`, `health`, `list_runs`, `get_run`, `pause`, `resume`,
  `stop`
- Generation fencing refuses stale supervisor generations
- Durable runs stay under `{dataRoot}/full-auto/runs.json`
- Run projections omit objective and transcript (redacted for supervisor
  events)
- Mutation API remains `full-auto-run-actions`

Omega crate `omega_effectd` can start, health-check, restart, and stop the
service. Crash recovery re-reads disk truth. GPUI is not run authority.

## Verification

- `pnpm --dir packages/omega-effectd test` — 177 passed
- `cargo test -p omega_effectd` in Omega — supervisor start/health/restart/stop
  and disk recovery

## Deferred

- GPUI launcher (FA-03)
- Full host routing/liveness wiring (FA-04)
- Packaged Node 24 + digest install into Omega release artifacts
