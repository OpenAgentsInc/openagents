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
- The desktop command surface now also exposes reviewer evidence recording through:
  - `/evidence verify <label> <passed|failed|running> [reference]`
  - `/evidence log <label> <reference>`
  - `/evidence preview <label> <reference>`
  - `/evidence screenshot <label> <reference>`
- The desktop command surface now also exposes delivery tracking through:
  - `/deliver pr [base-branch] [pr-url]`
  - `/deliver review <commented|approved|changes_requested> <reviewer-label> [summary]`
  - `/deliver merge <reviewer-label> [summary]`

## Current UI Semantics

- Probe turn state distinguishes `queued`, `running`, `paused`, `running+queued`, `cancelled`, and `timed_out`.
- `/requests` now includes Probe queue state when the active thread is Probe-backed.
- `/approvals session` is accepted for UX continuity, but Probe currently only supports per-call approval resolution, so the desktop maps that action to a single approval.
- evidence commands stay app-owned: Probe provides raw runtime truth, and the
  desktop groups that truth into one reviewer-facing evidence bundle per shared
  session
- delivery commands also stay app-owned: Probe does not become the hidden home
  for PR state, reviewer outcome, or authorship attribution

## Honest Limits

- Queue inspection is based on the current Probe session-control snapshot already projected into the desktop app. If that cache looks stale, reload the thread before cancelling a queued turn.
- Queue cancel is currently command-driven in the desktop shell. There is not yet a dedicated pane button for cancelling a specific queued Probe turn.
- Evidence references are local-first. They can point at local files or capture
  current terminal excerpts, but they are not a hosted artifact registry.
- Delivery receipts are also local-first. The first cut tracks GitHub PR state
  and reviewer outcome above the local shell rather than inventing a hosted
  publication substrate.
