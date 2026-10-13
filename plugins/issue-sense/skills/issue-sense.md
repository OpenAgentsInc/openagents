# Issue Sense

Use this when someone asks which open issues are already done, stale, or
blocked, or which commit did an issue's work.

- Which new commits did an open issue's work:
  `cargo run -q -p coder-new -- solver watch --since origin/main~20`.
  One JSON line per commit and issue asked; `likely_landed` at p ≥ 0.8.
- Every open issue judged landed, current, and actionable:
  `cargo run -q -p coder-new -- solver queue --decisions gateway`.

Both ask our hosted Clef (`--decisions gateway`), which is keyless and free.
Report what they found with links. Close or comment on an issue only when
the person asks.
