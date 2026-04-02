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
- Probe turn-control status now maps into explicit desktop session states:
  `attached`, `attached:running`, `attached:paused`, `queued`, `idle`,
  `completed`, `failed`, `cancelled`, `timed_out`, and `archived`
- runtime progress events update the existing active assistant message flow

This keeps project, transcript, and operator state app-owned even when the
runtime changes.

## Attach And Resume Posture

The desktop lane restores an existing Probe-backed thread by reloading the same
`session_id` instead of fabricating a new local session.

Current local-first behavior:

- startup refresh lists the available Probe sessions
- startup refresh now inspects per-session turn-control state so the thread
  rail can distinguish idle, completed, attached, and archived sessions instead
  of flattening them into one generic active row
- the active workspace reattaches to the matching live session if one is
  already present
- reloading a thread uses the same Probe session id
- session selection restores the thread's saved shell preferences before the
  Probe load command runs
- `new thread` in a Probe-backed workspace now runs an attach-vs-create policy:
  - if exactly one live session already matches the workspace, Autopilot
    attaches to it instead of starting a replacement
  - if the currently attached Probe session already matches the workspace,
    Autopilot reuses that session and reloads it into the shell
  - if multiple live sessions match the workspace and none is already attached,
    Autopilot refuses honestly and tells the operator to choose one from the
    thread rail instead of fabricating a new session

## Transcript And Status Projection

The reducer layer now translates these Probe shapes into app state:

- session snapshot transcript entries
- runtime progress events
- pending approval updates
- queued, interrupted, and cancelled turn control updates

That gives one Probe-backed thread a coherent transcript and runtime status
inside the existing shell.

Attach failures also stay visible at the shell layer. If the desktop cannot
reload the selected Probe session, the error is surfaced directly instead of
falling back to silent session creation.

## Shared Session Layer

Autopilot now keeps an app-owned Forge shared session object above raw Probe
session ids.

Current local-first rules:

- the Forge shared session id is distinct from the Probe `session_id`
- one shared session can point at one or more Probe sessions over time
- the shared session records the local human and the local Probe agent as
  explicit participants
- the current control owner is stored separately from raw Probe runtime status
- explicit handoffs persist summary, provenance, and timestamp in the desktop
  artifact projection
- reducer-level interrupt and resume events also update control-owner posture so
  the shell does not lose lineage when control flips between the operator and
  the background agent

The first operator-facing control is the chat command:

- `/handoff human <summary>`
- `/handoff agent <summary>`

## Workspace Restore Provenance

The shared session now also carries app-owned workspace restore provenance.

Current local-first behavior:

- `StartSession` marks the shared workspace as a `cold_start`
- `LoadSession` marks the shared workspace as a `warm_start` and records a
  local restore pointer derived from the Probe session id
- operators can explicitly mark a session as `restored` with:
  - `/restore <restore-pointer>`
  - `/restore <restore-pointer> <snapshot-ref>`
- base repo identity is captured from the local git workspace when available:
  remote origin, current branch, and current head commit
- when Probe cannot supply a snapshot ref, the shell says so directly instead of
  pretending a real snapshot registry already exists

## Evidence Bundle Layer

Autopilot now also keeps one app-owned evidence bundle above the shared session.

Current local-first behavior:

- the evidence bundle is linked from the Forge shared session and persists in
  the same desktop artifact projection
- latest diff and latest review truth are pulled into the bundle automatically
  from app-owned shell artifacts instead of asking the reviewer to spelunk the
  raw transcript
- operators can extend the bundle with:
  - `/evidence verify <label> <passed|failed|running> [reference]`
  - `/evidence log <label> <reference>`
  - `/evidence preview <label> <reference>`
  - `/evidence screenshot <label> <reference>`
- verification and log entries capture the current terminal tail when available
  so a reviewer gets durable evidence even though the live terminal buffer is
  not the persistence contract
- reviewer-facing evidence state is rendered honestly as missing, partial,
  complete, or failed

## Delivery Receipt Layer

Autopilot now also keeps one app-owned delivery receipt above the shared
session and evidence bundle.

Current local-first behavior:

- one delivery receipt is linked from the Forge shared session
- the receipt points back at the evidence bundle that justified the delivery
- `/deliver pr [base-branch] [pr-url]` records branch, commit, compare URL,
  optional GitHub PR URL, and suggested title/body state
- `/deliver review <commented|approved|changes_requested> <reviewer-label> [summary]`
  records reviewer outcome explicitly
- `/deliver merge <reviewer-label> [summary]` records merge closure explicitly
- authorship mapping is stored as an explicit product object with separate local
  human and local Probe agent roles instead of leaving that inference to raw
  transcript history

## Artifact Ownership

Plan, diff, review, and compaction presentation stays app-owned.

The first Probe projection slice does not pretend Probe already emits every
desktop-native artifact shape that the Codex lane has today. Where a Probe path
is not wired yet, the shell now refuses honestly instead of routing the action
through Codex against the wrong session type.

Examples in the current slice:

- thread reload is Probe-aware
- rename plus archive or unarchive now stay app-owned above the Forge shared
  session and persist as shell overlays instead of pretending Probe already owns
  those product semantics
- review now produces an app-owned review snapshot from the current shared
  session, evidence bundle, and delivery receipt state instead of refusing
- compaction now records an app-owned shell checkpoint artifact instead of
  pretending Probe already exposes a runtime-native compaction primitive
- rollback still stays one explicit product-level refusal because the current
  seam does not yet mutate a Probe workspace back to an earlier snapshot or
  restore pointer
- desktop mention and image attachments now go through an explicit app-owned
  forwarding manifest for Probe-backed turns
- the forwarded manifest is visible both in shell activity state and in the
  Probe transcript because the same rendered manifest is what the runtime
  receives
- shell-selected skill attachments still stay app-owned for Probe turns until a
  real tool-attachment contract exists

## What Still Follows

This projection layer is enough to make one local Probe-backed coding thread
real inside Autopilot.

It is not yet the full local operator loop. The next slice wires:

- queued follow-ups while a turn is still running
- approval roundtrips back through Probe
- queued-turn cancel and active-turn interrupt
