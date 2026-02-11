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

## RLM Trace Compatibility

- Canonical trace format: `openagents.dse.rlm_trace` v1 with typed `events`.
- Compatibility path: legacy v1 traces are normalized by
  `decodeRlmTraceDocV1CompatibleSync` in `src/traceMining/rlmTrace.ts`.
- Explicit version guard: unsupported `formatVersion` values fail fast.

## Tracing Span Names

Public runtime/eval operations are instrumented with `Effect.fn` and use stable
`dse.<area>.<operation>` names:

- `dse.compile.compile`
- `dse.eval.evaluate`
- `dse.eval.evaluateMetric`
- `dse.runtime.executeRlmAction`

`predict` keeps a signature-qualified span name (`dse.Predict(<signatureId>)`)
so traces remain attributable to a concrete signature call site.

## Development Setup

Install deps, then patch TypeScript for Effect build-time diagnostics:

```bash
npm install
npm run effect:patch
```

The package tsconfig includes the `@effect/language-service` plugin for editor diagnostics.
