## Problem

Nothing in the current export distinguishes useful executions from repeat executions of the same work. In trajectory `2026-08-27T12:29:02`: `npx vp test --run` executed twice for ~150s each (second was pure failure-name recovery), and `pnpm run test:rust` executed three times in one invocation — roughly 5–7 minutes of the 22-minute session spent redundantly, invisible to anyone reading the final metrics.

## Recommendation

Add automatic waste detection to the ATIF exporter (`extra.waste` or similar):

- Normalize every executed command to its head (executable + first meaningful args, i.e. everything before the first pipeline operator).
- Cluster steps by equal normalized head; flag any family with execution count > 1.
- Emit structured records:
  ```json
  { "head": "npx vp test --run", "executions": 2, "approx_wasted_seconds": 150 }
  ```
  (`wasted_seconds` = sum of durations beyond the first in the family, using `duration_ms` from the per-step-timing change.)
- Same for the pre-normalized *line* level so step 55's three-in-one-invocation triple-run gets caught even though it's one shell call.

This surfaces repeat-execution cost at export time without anyone hand-parsing the trajectory, which is exactly what had to happen manually today.

## Acceptance criteria

- [ ] Exporter emits per-head execution counts + wasted-seconds on every session.
- [ ] Against a replayed/fresh run reproducing this trajectory's shape, it reports `npx vp test --run: 2×` and `pnpm run test:rust: 3×` automatically.
- [ ] Depends on / pairs with the `duration_ms` issue; without timing, wasted_seconds falls back to wall-clock deltas between duplicated heads.
