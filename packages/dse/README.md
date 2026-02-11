# @openagentsinc/dse

`dse` is an Effect-first library for declarative, optimizable LM “programs”:

- **Signatures** are typed contracts (Effect `Schema` IO + Prompt IR).
- **Modules** are Effect programs (`I -> Effect<R, E, O>`).
- **Predict** is the minimal runtime bridge: Signature + Policy -> prompt -> model -> decoded output.
- **Eval** provides datasets, metrics (incl. judge-backed metrics), and reward signal aggregation.
- **Compile** runs a small, explicit optimizer loop to emit immutable compiled artifacts.

This package is intentionally small and contract-driven. The corresponding spec lives at `docs/autopilot/dse.md`.

Additional design notes:

- `packages/dse/docs/EFFECT_ONLY_DSE_RLM_GEPA_MIPRO_DESIGN.md`
- `packages/dse/docs/RLM_GEPA_MIPRO_DSE_REVIEW_AND_ROADMAP.md`
- `packages/dse/docs/RLM_GEPA_MIPRO_SUMMARY.md`

## Development Setup

Install deps, then patch TypeScript for Effect build-time diagnostics:

```bash
npm install
npm run effect:patch
```

The package tsconfig includes the `@effect/language-service` plugin for editor diagnostics.
