## Problem

The ATIF export (`extra`, `final_metrics`) carries no per-step timing. Trajectory `2026-08-27T12:29:02` required manual `jq` over timestamps to estimate that ~5–7 of 22 minutes went to redundant suite reruns — and even then step boundaries were mushy because consecutive agent steps share a timestamp when several tool calls land in one model turn.

## Recommendation

Emit per-tool-result timing in the ATIF exporter:

- Add `duration_ms` to each entry in `results` (the observation block), measured around the tool call.
- Optionally also `started_at`/`finished_at` on the step, so multi-call steps stop collapsing into one instant.

This makes waste visible instead of reconstructable by hand, and lets downstream tooling (fleet dashboards, cost audits, trajectory graders) compute "time spent re-executing known output" without parsing prose.

## Acceptance criteria

- [ ] Every ATIF v1.x `results[]` entry carries an integer `duration_ms`.
- [ ] Re-exporting trajectory `2026-08-27T12:29:02` (or any future equivalent) shows step 48 ≈ 152000ms, step 55 ≈ 289000ms without timestamp subtraction.
