# $TS Work Log (oa-36e88a)
- Investigated typecheck hang/regressions after disabling Effect plugin; running `bun run typecheck` exposed errors.
- Fixes: corrected Bun platform import and Task fixtures in recovery tests; strengthened recovery result handling; extended flow TaskStatus to include commit_pending; improved Healer progress upsert (marker handling + legacy replacement) and HUD event mapping return.
- Added previous attempts info to Healer summary to keep scenarios distinct and ensure idempotent replacement; adjusted markers to avoid double counting.
- Validation: `bun run typecheck`, `bun test` (all passing).
