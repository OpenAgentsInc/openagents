---
name: session-handoff
description: >-
  Use when a long conversation hits ~4-6 context compactions, completes a major
  milestone, or needs to hand off cleanly to a fresh chat or another agent
  without losing state or repeating dead ends.
---

# Compact Session Handoff Protocol

Long threads beyond 6–8 compactions burn time and tokens on post-compaction re-reads, stale `edit_file` targets, and forgotten invariants. Maintain a scratch ledger during `/goal` runs and emit this compact handoff at milestone boundaries.

## During Long `/goal` Runs (Anti-Amnesia Ledger)
Write and update a compact ledger in your scratch/artifact directory after every sprint:
- `baseline`: exact command, binary/commit SHA, and numeric metrics.
- `iterations`: 1 row per sprint (change, commit/diff, verified metric delta, keep/revert).
- `ruled_out`: approaches tried and rejected with exact reason (prevents post-compaction retries).
- `next`: single next concrete experiment.

## Fresh-Chat Handoff Template (<= 25 Lines)

```markdown
### Handoff Summary
- **Goal:** <1 sentence + exact acceptance criteria>
- **Branch / Worktree:** `<branch>` @ `<short-sha>` (worktree `<path>` or cleaned up)
- **Shipped Commits (on origin/main):** `<sha1> <subject>`, `<sha2> <subject>`
- **Verified Checks (real outputs):** `<exact command>` -> `<pass count / metric>`
- **Remaining Work:**
  1. <concrete next item with exact file path & symbol>
  2. <remaining verification gate>
- **Pitfalls Not To Repeat:**
  - <dead end / trap already discovered>
- **Exact Next Command:** `<command to run first in fresh chat>`
```
