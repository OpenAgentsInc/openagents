# lightning-ops

Effect-native operational service for hosted L402 gateway workflows.

## Scope in Phase 2A

- Pull hosted paywall control-plane state from Convex.
- Compile deterministic `aperture.yaml` artifacts + stable `configHash`.
- Validate route/policy state and emit typed diagnostics.
- Persist compile/deployment intent records back to Convex.

## Commands

```bash
npm run typecheck
npm test
npm run smoke:compile -- --json
```

`smoke:compile -- --json` prints machine-readable JSON with:

- `configHash`
- `ruleCount`
- `valid`

Environment variables for Convex-backed operation:

- `OA_LIGHTNING_OPS_CONVEX_URL`
- `OA_LIGHTNING_OPS_SECRET`

The smoke command uses deterministic in-memory fixtures and does not require network access.
