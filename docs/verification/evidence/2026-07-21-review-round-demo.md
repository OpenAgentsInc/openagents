# Review-round demonstration — 2026-07-21

One live demonstration of `.agents/workflows/review-round.js` over a real
commit, retained as evidence for FREERANGE-01 (#9124). The run proves the
workflow executes end to end and that the positive-control path returns a
`clean` result only when the lenses proved a sweep.

## Run

- Frozen commit: `d5f040c1d4` ("fix(release): read GCS object sha256 from
  custom_fields, not metadata"), a one-line release fix.
- Lenses: `correctness` and `contract`. Model: `claude-opus-4-8`.
- Verify phase: not requested. No findings, so no verifier ran.

Per-lens result:

| Lens        | probesRun | findings | tool calls | duration |
| ----------- | --------- | -------- | ---------- | -------- |
| correctness | 8         | 0        | 23         | ~195 s   |
| contract    | 14        | 0        | 19         | ~153 s   |

## Round result

```json
{
  "schemaVersion": "openagents.review-round.v1",
  "status": "clean",
  "confirmedFindings": [],
  "failures": [],
  "lensesSwept": 2,
  "lensesReported": 2,
  "lensesAttempted": 2,
  "probesRun": 22
}
```

## What this demonstrates

- The round reached `clean` only because both lenses reported a positive
  control (`probesRun` of 8 and 14). Each lens actually inspected the commit:
  42 tool calls across the two agents. An empty findings list from a lens that
  ran zero probes would have been `lens-unproven` and the round would be
  `failed`, not clean (rule 1).
- Both lenses reported (`lensesReported` of 2) and both proved a sweep
  (`lensesSwept` of 2). No lens died, so no `agent-died` row.

The died-lens, unproven-lens, unsubstantiated-finding, malformed-report, and
no-sweep paths are covered exhaustively and deterministically by the unit
tests in `packages/review-round/src/index.test.ts`. This live run adds the
end-to-end evidence that the workflow drives real lens agents through the same
fail-closed fold.
