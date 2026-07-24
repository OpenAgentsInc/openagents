# Omega-effectd Full Auto extract (FA-01)

- Date: 2026-07-24
- Packet: `OMEGA-FA-01`
- Omega issue: [OpenAgentsInc/omega#20](https://github.com/OpenAgentsInc/omega/issues/20)
- Package: `@openagentsinc/omega-effectd` `0.1.0`
- Pack SHA-256: `20e4e002202b6db640501301f134e0a9f0f8718fe1c1dc38bef4aa5ab2d69ffe`
- Freeze: [2026-07-24-full-auto-contract-freeze.md](./2026-07-24-full-auto-contract-freeze.md)

## Result

The portable Full Auto engine now lives in `packages/omega-effectd`.
OpenAgents Desktop keeps re-export shims at the old paths.
Omega pins the packed tarball digest.
Omega must not use a relative monorepo path or an unpublished `workspace:*`
edge to run Full Auto.

## Injected data root

Set `OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT` for the service and control client.
Durable files stay under `{dataRoot}/full-auto/`.

## Verification

- `pnpm --dir packages/omega-effectd test` — 177 passed
- `pnpm --dir packages/omega-effectd run pack:digest` — digest above

## Deferred to later FA packets

- Rust supervisor and framed protocol — see
  [2026-07-24-omega-effectd-supervisor.md](./2026-07-24-omega-effectd-supervisor.md)
  (FA-02)
- GPUI launcher — see
  [2026-07-24-omega-full-auto-gpui-launcher.md](./2026-07-24-omega-full-auto-gpui-launcher.md)
  (FA-03)
- Full routing/liveness host wiring beyond the extracted engine (FA-04)
- MemoHarness and initiative (FA-00 cut)
