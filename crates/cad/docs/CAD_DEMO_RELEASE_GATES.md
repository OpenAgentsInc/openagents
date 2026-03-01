# CAD Demo Release Gates (A-E)

This checklist is the blocking release gate for the CAD demo milestone.

Companion runbook for local/CI lane execution:

- [`crates/cad/docs/CAD_CODE_HEALTH.md`](/Users/christopherdavid/code/openagents/crates/cad/docs/CAD_CODE_HEALTH.md)

Command:

```bash
scripts/cad/release-gate-checklist.sh
```

Any failing check blocks milestone release.

## Scope

In scope:

- Gate A-E validation for deterministic CAD demo readiness.
- Deterministic command set mapped to each gate.
- Release-process integration so milestone release is blocked if any gate is red.

Out of scope:

- Wave 2 feature expansion.
- New visual polish beyond existing gate criteria.
- New CAD operations not already tracked in backlog issues.

Dependencies:

- Depends on `#2537` for 20-second reliability lane coverage.
- Uses existing gate fixtures and test lanes from backlog items 84-86.

## Gate Checklist

Gate A (Kernel + Validity + History):

- deterministic rebuild + receipts
- tolerance policy binding
- validity warning coverage
- undo/redo deterministic integrity

Gate B (Viewport + Pro UX):

- viewport control layouts remain in bounds
- clipping/overflow invariants for warning/timeline/dimension panels
- render mode/hotkey/3D mouse deterministic behavior
- selection inspect body/face/edge coverage

Gate C (Generator):

- deterministic rack template generation
- deterministic four-variant objective engine
- deterministic rack geometry golden snapshots

Gate D (AI):

- schema-constrained intent dispatch
- explicit free-text mutation rejection
- deterministic chat-to-intent recovery behavior
- deterministic follow-up interaction path in desktop reducer

Gate E (Engineering + Demo):

- STEP checker + round-trip fixture lane
- headless script harness lane
- performance budget lane
- 20-second scripted reliability lane

## Observability and Verification

- Each gate check is logged with gate id + check label.
- Failures print command output and stop immediately.
- STEP checker artifacts are written under `artifacts/cad-step-checker` when that lane runs.
- Reviewer verification command is a single entry point:
  - `scripts/cad/release-gate-checklist.sh`
