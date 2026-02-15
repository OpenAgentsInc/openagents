# Signatures (Typed Contracts)

Signatures are typed input/output contracts for model-facing decisions and generation steps.

Canonical terminology:
- `Signature`, `Module`, `Predictor`, `Optimizer`, `Metric` are defined in `docs/GLOSSARY.md`.

Normative expectations (agent-first invariants):
- Any decision point that gates behavior SHOULD be a signature (or signature-backed pipeline).
- Inputs and outputs MUST be typed and versioned.
- Boundary shapes MUST be validated (schema decode at ingress).

## Signature IDs

Signatures MUST have stable ids.

Recommended pattern:
- `oa.<domain>.<action>.v<major>`

Examples:
- `oa.autopilot.route_lane.v1`
- `oa.tools.select_next_step.v1`

## Versioning

- Backwards-compatible changes can be published as the same major version if outputs remain compatible.
- Breaking changes require incrementing the major version (new id).

## Where to Implement

- DSE signatures/modules: `packages/dse/`
- Autopilot-specific catalog wiring: `apps/autopilot-worker/src/dseCatalog.ts`

