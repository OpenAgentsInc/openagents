# Probe Operator Controls

This document records the current app-owned operator loop for Probe-backed Autopilot sessions.

## What Ships Now

- If a Probe-backed thread is idle, submitting from the composer starts or continues the session immediately.
- If a Probe-backed thread already has an active turn, the composer now queues the follow-up instead of rejecting it.
- The desktop approval UI now returns Probe tool-approval decisions through the shared Probe client boundary.
- The desktop interrupt control now sends a Probe turn interrupt instead of remaining Codex-only.
- The desktop command surface now exposes queue inspection and queue cancel through:
  - `/queue`
  - `/queue cancel [turn-id]`

## Current UI Semantics

- Probe turn state distinguishes `queued`, `running`, `paused`, `running+queued`, `cancelled`, and `timed_out`.
- `/requests` now includes Probe queue state when the active thread is Probe-backed.
- `/approvals session` is accepted for UX continuity, but Probe currently only supports per-call approval resolution, so the desktop maps that action to a single approval.

## Honest Limits

- Queue inspection is based on the current Probe session-control snapshot already projected into the desktop app. If that cache looks stale, reload the thread before cancelling a queued turn.
- Queue cancel is currently command-driven in the desktop shell. There is not yet a dedicated pane button for cancelling a specific queued Probe turn.
