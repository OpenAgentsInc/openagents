# Omega Full Auto GPUI launcher (FA-03)

- Date: 2026-07-24
- Packet: `OMEGA-FA-03`
- Omega issue: [OpenAgentsInc/omega#22](https://github.com/OpenAgentsInc/omega/issues/22)
- Package: `@openagentsinc/omega-effectd` `0.1.0`
- Pack SHA-256: `20e4e002202b6db640501301f134e0a9f0f8718fe1c1dc38bef4aa5ab2d69ffe`
- Protocol: `openagents.omega.effectd.v1` methods `start` and `retry` plus owner-local `get_run` detail
- Freeze: [2026-07-24-full-auto-contract-freeze.md](./2026-07-24-full-auto-contract-freeze.md)

## Result

Framed protocol now admits the GPUI launcher mutation path:

- `start` — mint a run through `startFullAutoRunAction`
- `retry` — retry a stalled run through `retryFullAutoRunNowAction`
- `get_run` — owner-local detail (objective, turns, stall/recovery) for the monitor
- `list_runs` — remains redacted (no objective text)

Omega GPUI owns presentation only. Durable mutation stays in supervised
`omega-effectd`. Ordinary chat and composer paths do not start Full Auto.

## Verification

- `pnpm --filter @openagentsinc/omega-effectd test` — 178 passed

## Next

- Omega crate `full_auto_ui` panel wiring and proof (same issue closeout)
- FA-04 routing and liveness
