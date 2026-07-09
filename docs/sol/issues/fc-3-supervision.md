# FC-3: Sarah fleet supervision — canvas, progress, approvals, and steering

Parent: #8638

## Outcome

Sarah can explain and control an active FleetRun from conversation and the
Blueprint canvas without exposing raw worker logs.

## Existing substrate

MH-6 already shipped Khala Sync fleet projections plus the shared typed
`fleet_run_control`, `approval_decision`, and `steer_message` intents. The
Sarah surface already has a typed SSE bus, Blueprint deltas, Actions, and
Code/Receipts panels. This issue composes them.

## Scope

1. Project run, work-unit, worker, account-health, blocker, approval, verify,
   and closeout summaries into owner-scoped Sync/Sarah reads.
2. Add Sarah tools/intents for status, pause, resume, drain, stop, steer,
   approval decision, and bounded follow-up.
3. Render active work in the Blueprint canvas with evidence-backed edges:
   plan → claim → assignment → verification → closeout.
4. Give each work stream a stable identity so conversation cannot steer “the
   latest” ambiguous task.
5. Preserve private steer bodies and raw events in owner-private stores; safe
   projections carry refs and summaries.
6. Make reconnect/resume normal: a new browser or mobile session reconstructs
   current state from durable projections.
7. Add useful degradation behavior when avatar video is unavailable.

## Exit

From `/sarah`, an owner starts or opens a three-stream fixture run, pauses and
resumes it, steers one named work unit, resolves one approval, reconnects the
browser, and sees the same state. Every transition is idempotent and receipted;
no raw prompt, command, output, credential, or path appears in the Sarah
projection.
