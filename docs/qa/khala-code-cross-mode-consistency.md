# Khala Code Cross-Mode Consistency

Status: ROADMAP_QA Q4.5 / issue #8031 implemented.

The cross-mode corpus lives in
`packages/khala-qa-harness/src/seed-corpus.ts` as
`scenario.khala_code.seed.cross_mode_consistency.v1`. It is one fixture-tier
scenario document with `modes: ["rpc", "dom"]`; the runner executes that same
document through Mode P and Mode D drivers, then evaluates the scenario's
`consistency` oracles across both mode reports.

Covered projections:

- `projection:thread_list`
- `projection:fleet_counts`
- `projection:gym_state`
- `projection:runtime_badges`

Mode D is deterministic in this tier: `makeKhalaCodeDomFixtureQaDriver` replays
the typed fixture RPCs and exposes DOM-shaped `projection:*` reads. The
projection helper canonicalizes both modes to stable public-safe state before
comparison, so the oracle checks the user-visible contract rather than raw
transport payload trivia.

When a cross-mode oracle refutes, `runKhalaCodeQaCrossModeScenario` returns a
`khala_code_qa_cross_mode_disagreement_bug.v1` payload and can call a
`fileDisagreement` hook. The payload includes the scenario id, phase, mismatch
paths, and both mode states.

Coverage flows through `crossModeSurfacesExercised` in the coverage ledger and
`crossModeSurfaces` in the seed-corpus manifest/frontier report.
