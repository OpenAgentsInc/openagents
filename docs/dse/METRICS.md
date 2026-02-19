# Metrics (Evaluation + Optimization)

Metrics score outputs of signatures/modules. They enable optimization and safe rollouts.

See:
- `docs/GLOSSARY.md` (Metric, Proxy Metric, Truth Metric)
- `docs/adr/ADR-0015-policy-bundles.md` (rollout states)

## Metric IDs

Metrics SHOULD have stable ids.

Recommended pattern:
- `oa.metric.<domain>.<name>.v<major>`

## Proxy vs Truth

- Proxy metrics: cheap checks (schema validity, formatting, constraints).
- Truth metrics: expensive checks (sandbox verification, judge models).

## Where to Implement

- Compiler/runtime metric code: `packages/dse/src/`
- Domain-specific eval harnesses: `apps/openagents.com/`, `apps/openagents-runtime/`, `packages/effuse-test/`, and targeted runtime scripts in `scripts/`.
