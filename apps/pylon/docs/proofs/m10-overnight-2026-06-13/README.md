# M10 overnight proof run, 2026-06-13

Issue: OpenAgentsInc/openagents#4768

Run ref: `run.m10.overnight.fe89b989a4adfdde61631a79`

Window: `2026-06-13T00:49:44.268Z` to `2026-06-13T03:59:46.366Z`

## What this proves

This directory retains the completed local, no-spend M10 overnight proof
artifacts from the owner-directed unattended batch. The run covered 8 cycles
across both local lanes and both local surfaces:

- lanes: `codex`, `claude_agent`
- surfaces: `composer`, `work_order`
- supervision: `humanInLoopDuringRun: false`
- execution mode: `local_bounded`
- payment mode: `no-spend`

The summary reports 32 attempted tasks, 31 completed tasks, 1 failed task, and
167 written heartbeat records. The final heartbeat sequence reaches
`phase: "finished"`.

The artifact-level redaction scan is recorded as clean:

- `redactionScan.state: "clean"`
- `redactionScan.appliedToEveryArtifact: true`

## Primary evidence

- `m10-overnight-summary.json` is the run summary and closeout index.
- `heartbeats.jsonl` is the caller-clocked unattended heartbeat stream.
- `m10-c*-*-*-proof.json` files are retained per-task proof artifacts.
- `m10-task-failure-12.json` records the single retained failed task:
  cycle 3, `claude_agent` lane, `work_order` surface,
  `errorClass: "execution_error"`.

## Acceptance status

This is evidence toward #4768, not full closure evidence for #4768.

The run proves the unattended local no-spend subset across both local lanes and
both local surfaces. It does not claim the live SHC, live web UI,
`pylon work status`, notification, morning-review, or metering requirements
from the issue acceptance criteria.

The summary intentionally records these remaining deviations:

- `deviation.m10.shc_lane_deferred_requires_live_scheduled_launch`
- `deviation.m10.web_ui_surface_deferred_requires_live_api`
- `deviation.m10.work_status_cli_surface_deferred_requires_live_api`
- `deviation.m10.work_order_surface_used_local_assignment_harness`
- `deviation.m10.metering_ledger_check_deferred_to_live_lane`
- `deviation.m10.lanes_interpreted_as_local_codex_and_claude_adapters`

To close #4768, the remaining work is a live scheduled SHC pass, a live own
Pylon pass with the zero-credit ledger guarantee, matching live web UI and
`pylon work status` refs, notification/morning-review evidence, and a clean
receipt bundle for those live runs.
