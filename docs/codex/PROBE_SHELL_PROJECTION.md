# Probe Shell Projection

## Scope

This document records how a local Probe session is projected into the existing
Autopilot coding shell.

The goal is not a second UI. The goal is to map Probe runtime truth into the
same app-owned shell state that previously only projected Codex.

## Mapping Rules

The current local mapping is intentionally simple:

- Probe `session_id` is the Autopilot thread id
- the session `cwd` and transcript path map into existing thread workspace
  metadata
- Probe turn-control status maps into the existing shell status vocabulary:
  `queued`, `running`, `paused`, `completed`, `failed`, `cancelled`,
  `timed_out`
- runtime progress events update the existing active assistant message flow

This keeps project, transcript, and operator state app-owned even when the
runtime changes.

## Attach And Resume Posture

The desktop lane restores an existing Probe-backed thread by reloading the same
`session_id` instead of fabricating a new local session.

Current local-first behavior:

- startup refresh lists the available Probe sessions
- the active workspace can attach to the matching session if one is already
  present
- reloading a thread uses the same Probe session id
- session selection restores the thread's saved shell preferences before the
  Probe load command runs

## Transcript And Status Projection

The reducer layer now translates these Probe shapes into app state:

- session snapshot transcript entries
- runtime progress events
- pending approval updates
- queued, interrupted, and cancelled turn control updates

That gives one Probe-backed thread a coherent transcript and runtime status
inside the existing shell.

## Artifact Ownership

Plan, diff, review, and compaction presentation stays app-owned.

The first Probe projection slice does not pretend Probe already emits every
desktop-native artifact shape that the Codex lane has today. Where a Probe path
is not wired yet, the shell now refuses honestly instead of routing the action
through Codex against the wrong session type.

Examples in the current slice:

- thread reload is Probe-aware
- review, compaction, rollback, rename, archive, and similar Codex-only actions
  refuse honestly for Probe-backed sessions
- desktop mention and image attachments are also refused honestly for Probe
  turns until that forwarding contract exists

## What Still Follows

This projection layer is enough to make one local Probe-backed coding thread
real inside Autopilot.

It is not yet the full local operator loop. The next slice wires:

- queued follow-ups while a turn is still running
- approval roundtrips back through Probe
- queued-turn cancel and active-turn interrupt
