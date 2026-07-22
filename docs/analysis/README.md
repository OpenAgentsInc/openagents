# Conversation analysis

This directory contains methods that assess conversation quality.
These methods do not give product or release authority.

- [Conversation thread coherence rubric](./conversation-thread-coherence-rubric.md):
  the semantic assessment method. Eight weighted dimensions, hard-fail
  gates, tripwires, and a report format.
- [Deterministic coherence screening](./deterministic-coherence-screening.md):
  the fast machine-checkable layer. Run it with
  `pnpm run grade:coherence`. Tests: `pnpm run test:coherence`.
- [Coherence flywheel](./coherence-flywheel.md): the dogfood process.
  Create conversations, grade them, fix the worst defect, add a
  fixture, and record the sweep.
- [Coherence ledger](./coherence-ledger.md): append-only sweep records
  and the hill-climb trend.
- [Public coherence benchmark spec](./public-coherence-benchmark-spec.md):
  proposal for the public graph, toolchain comparison, version trend
  lines, and user-contributed traces.
- [Issue #9159 programmatic validation](./2026-07-22-conversation-coherence-programmatic-validation.md):
  the baseline score and the corrected programmatic scores.
