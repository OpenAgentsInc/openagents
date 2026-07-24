# `@openagentsinc/omega-effectd`

Supervised Node 24 + Effect Full Auto engine for Omega.

## Authority

- Freeze: `docs/omega/2026-07-24-full-auto-contract-freeze.md` (`OMEGA-FA-00`)
- Port audit: `docs/omega/2026-07-24-full-auto-port-audit.md` (`OMEGA-FA-01`)
- Omega issue: https://github.com/OpenAgentsInc/omega/issues/20

## Laws

- Active run limit: 8
- One active lease per thread
- Mutation API: `full-auto-run-actions` (engine module)
- Data root: injected (`OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT` / `OmegaEffectdPaths`)
- Never use a Zed data root or a hard-coded Electron userData path

## Pack digest

```sh
pnpm --dir packages/omega-effectd run pack:digest
```

Omega pins the printed `sha256`. It must not import this package through a
relative monorepo path or an unpublished `workspace:*` edge.

Framed protocol schema: `openagents.omega.effectd.v1` (stdio JSON lines).

## Deferred

MemoHarness and initiative stay out of the first Omega port (FA-00 freeze).
