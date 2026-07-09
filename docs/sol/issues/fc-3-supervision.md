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
7. Model conversation and media separately. Conversation states are at least
   `idle | connecting | text_live | busy | reconnecting | ended | failed`;
   media states are at least `not_requested | queued | connecting | live |
   stale | unavailable | evicted | ended`. `media=live` requires a fresh frame/
   transport lease. `media=stale + conversation=text_live` renders an explicit
   text-continuation/reconnect state and can never render a frozen LIVE badge.
8. Keep the FC latency contract visible: first executor progress or typed
   blocker p95 <= 30 seconds; active progress/heartbeat at least every 15
   seconds; after 30 seconds without freshness, show `stalled` or
   `reconnecting`, never indefinite live/running.
9. Render the first one-minute-readable receipt card for coding closeout. In
   order it answers: what happened; whether verification passed and what
   verified it; what changed and where the safe artifact is; account/capacity
   class and cost or `not_measured`; applicable approval/authority; and the next
   available action. Exact refs remain expandable audit detail.
10. Apply the FC-1 typed relationship posture to response density and controls;
    the UI never derives operator/admin authority from tone.

## Exit

From `/sarah`, an owner starts or opens a three-stream fixture run, pauses and
resumes it, steers one named work unit, resolves one approval, reconnects the
browser, and sees the same state. Every transition is idempotent and receipted;
no raw prompt, command, output, credential, or path appears in the Sarah
projection. Simulator tests expire the media lease without killing text/fleet
control, make the frozen-frame LIVE combination unrepresentable, and exercise
the 30-second stall transition. The coding closeout passes the one-minute
non-developer comprehension test.
