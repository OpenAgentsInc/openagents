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
- The desktop command surface now also exposes campaign curation through:
  - `/campaign open <title>`
  - `/campaign goal <summary>`
  - `/campaign scope <summary>`
  - `/campaign candidate <probe_summary|accepted_patch|evidence_bundle|psionic_eval|psionic_compare> <reference> [summary]`
  - `/campaign case <case-id> <probe_summary|accepted_patch|evidence_bundle|psionic_eval|psionic_compare> <reference> [summary]`
  - `/campaign verify <evidence_bundle|delivery_receipt|psionic_eval|psionic_compare> <reference> [summary]`
  - `/campaign status`
- The desktop command surface now also exposes delivery tracking through:
  - `/deliver pr [base-branch] [pr-url]`
  - `/deliver status`
  - `/deliver refresh`
  - `/deliver review <commented|approved|changes_requested> <reviewer-label> [summary]`
  - `/deliver merge <reviewer-label> [summary]`
- The desktop command surface now also exposes settlement tracking through:
  - `/settle merge <reviewer-label> [summary]`
  - `/settle metric <evaluator-label> <reference> [summary]`
  - `/settle dispute <actor-label> [summary]`
  - `/settle cancel <reason>`
  - `/settle status`
- The header actions for Probe-backed threads now use app-owned parity where the
  runtime seam is still narrower:
  - rename and archive or unarchive persist as shared-session shell overlays
  - review emits a shell-owned review snapshot from current evidence or delivery
    truth
  - compact records a shell checkpoint artifact
  - rollback stays an explicit refusal until workspace restore mutation exists

## Current UI Semantics

- Probe turn state distinguishes `queued`, `running`, `paused`, `running+queued`, `cancelled`, and `timed_out`.
- `/requests` now includes Probe queue state when the active thread is Probe-backed.
- `/approvals session` is accepted for UX continuity, but Probe currently only supports per-call approval resolution, so the desktop maps that action to a single approval.
- evidence commands stay app-owned: Probe provides raw runtime truth, and the
  desktop groups that truth into one reviewer-facing evidence bundle per shared
  session
- campaign commands also stay app-owned: Probe is not the hidden home for
  retained-case selection, eval candidate curation, or promotion intent above
  the current shared session
- delivery commands also stay app-owned: Probe does not become the hidden home
  for PR state, reviewer outcome, or authorship attribution
- settlement commands also stay app-owned: Probe is not the hidden home for
  dispute windows, settlement cancel reasons, evaluator references, or payout
  closure posture
- local branch and compare watch now refresh automatically from Probe session
  load and detached workspace-state events when a delivery receipt already
  exists
- GitHub PR and CI watch refresh stays explicit and bounded through
  `/deliver refresh`; the desktop does not start hidden background browser or
  API polling loops for the first pass

## Honest Limits

- Queue inspection is based on the current Probe session-control snapshot already projected into the desktop app. If that cache looks stale, reload the thread before cancelling a queued turn.
- Queue cancel is currently command-driven in the desktop shell. There is not yet a dedicated pane button for cancelling a specific queued Probe turn.
- Evidence references are local-first. They can point at local files or capture
  current terminal excerpts, but they are not a hosted artifact registry.
- Campaign refs are also local-first. Psionic bundle or comparison refs are
  typed strings in the shell projection, not yet a hosted retained-case catalog
  or promotion ledger.
- Delivery receipts are also local-first. The first cut tracks GitHub PR state
  and reviewer outcome above the local shell rather than inventing a hosted
  publication substrate.
- Settlement receipts are also local-first. They close the current shared
  session truth for review or retained-metric purposes, but they are not yet a
  money-movement system or a hosted claims database.
- GitHub watch refresh depends on a working local `gh` installation plus repo
  access. If that is missing, the shell keeps the last recorded watch state and
  reports the refresh failure honestly.
