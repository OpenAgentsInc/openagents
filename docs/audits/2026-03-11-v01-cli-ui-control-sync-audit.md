# v0.1 CLI Control + UI Sync Audit

Date: 2026-03-11
Branch audited: `main`
Audit type: static product and architecture audit focused on making the retained `v0.1` desktop app fully controllable by CLI while keeping Mission Control and the rest of the running UI truthful and visibly in sync

## Audit Question

How should `v0.1`, as defined in `docs/v01.md`, become fully controllable by CLI so an agent can drive the app programmatically while the running desktop UI updates from the same underlying state transitions?

More concretely:

- what control seams already exist,
- what is missing today,
- what should remain app-owned in `apps/autopilot-desktop`,
- what should not be moved into shared crates,
- and what is the smallest honest control architecture that matches the `v0.1` Mission Control-first product cut?

## Scope

Primary docs reviewed:

- `docs/v01.md`
- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- prior audits in `docs/audits/`

Primary code reviewed:

- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/runtime_log.rs`
- `apps/autopilot-desktop/src/codex_remote.rs`
- `apps/autopilot-desktop/src/bin/autopilot_headless_compute.rs`

## v0.1 Product Constraint

`docs/v01.md` is very clear: `v0.1` is not a general floating-pane workstation. It is a one-screen Mission Control product cut whose core loop is:

1. open the desktop app
2. land directly in Mission Control
3. get Apple FM ready
4. go online
5. receive paid compute work
6. see wallet-confirmed sats increase
7. withdraw

That matters for CLI control. The goal is not "remote-control every historical pane feature." The goal is:

- an agent can drive the same `v0.1` Mission Control flow from CLI,
- the running app updates on screen as the commands land,
- the operator can still trust the UI,
- and there is only one state authority for what the app is doing.

## What "Fully Controllable Via CLI" Should Mean

For this product cut, "fully controllable via CLI" should mean all of the following:

- A local CLI can target the running desktop app, not just a separate headless process.
- CLI commands mutate the same app-owned state that UI clicks mutate.
- Mission Control visibly updates after those commands.
- The app can return structured status snapshots, not just human log lines.
- The app emits machine-readable events so an agent can wait for readiness, progress, settlement, or errors.
- The control path is replay-safe and leaves an audit trail in the same runtime/session logs the UI already uses.

It should not mean:

- a second hidden automation-only state machine,
- a headless sidecar that bypasses the desktop app,
- or a generic pane-click protocol that leaks renderer internals into the control surface.

## Current State

### 1. The desktop already has an app-owned action path

This is the most important positive finding.

The running UI is not driven by arbitrary widget callbacks spread everywhere. It already funnels important actions through app-owned action dispatch:

- `PaneHitAction::GoOnlineToggle` in `input.rs` calls `apply_provider_mode_target(...)`
- `PaneHitAction::MissionControl(action)` delegates to `run_mission_control_action(...)`
- `run_mission_control_action(...)` in `input/actions.rs` owns actions like:
  - local FM summary test
  - buy-mode start and stop
  - open buy-mode payments
  - withdraw send
  - wallet refresh and related Mission Control operations

That is exactly the kind of seam a CLI control plane should reuse.

### 2. There are already local machine-facing surfaces in the repo

Two existing patterns matter:

- `autopilot_headless_compute` proves there is already a CLI-first automation path for compute flows.
- `codex_remote.rs` proves the app already knows how to expose a loopback-only local runtime with:
  - an authenticated local HTTP server
  - a structured snapshot endpoint
  - an action endpoint
  - runtime snapshot sync
  - and app-owned action dispatch back into `RenderState`

This means the repo does not need a brand new control paradigm. It already contains a working precedent.

### 3. Mission Control truth exists, but mostly as a UI projection

The app already synthesizes a lot of operator truth:

- provider mode and blockers
- Apple FM readiness
- wallet status
- buy-mode status
- active job lifecycle
- log stream summaries

But much of that truth is currently assembled for rendering or textual projection, not exported as a stable structured control snapshot.

In other words:

- the truth is in the app,
- but the machine-friendly shape is still incomplete.

### 4. Runtime logs are already strong

`runtime_log.rs` already records:

- tracing events
- mission control lines
- session metadata
- compute-domain projections

This is a strong foundation for CLI synchronization because it means the app already has a durable machine-readable audit trail. The missing piece is not logging. The missing piece is a formal command and snapshot surface for the running desktop.

## Main Gap

The main gap is simple:

There is no app-owned "desktop control plane" for the running `v0.1` Mission Control product.

Today the repo has:

- UI click paths for the running desktop
- headless binaries for separate automation
- Codex-specific remote control for one subsystem

What it does not yet have is:

- a local CLI that can tell the running desktop app "go online", "refresh wallet", "start buy mode", "withdraw", or "show me current Mission Control truth"
- and receive that truth back in a stable structured form

That is the architectural gap to close.

## Findings

### 1. The right control boundary is app-owned, not crate-generic

Per `docs/OWNERSHIP.md`, this work belongs in `apps/autopilot-desktop`.

Reason:

- the `v0.1` control story is product-specific
- the command set is Mission Control-specific
- the read model is Mission Control-specific
- the required sync semantics are app UX semantics, not generic widget semantics

This should not be implemented by moving control concepts into `crates/wgpui`, `crates/spark`, or another reusable crate.

### 2. Headless compute is not enough

`autopilot_headless_compute` is useful, but it is not the same thing as controlling the desktop app.

Headless flow validates:

- relay behavior
- buyer/provider orchestration
- payment paths

It does not solve:

- synchronized desktop state
- on-screen operator trust
- or app-local command dispatch for the running UI

So the control-plane design should reuse headless tests as validation, but not mistake headless control for desktop control.

### 3. The repo already has the best implementation pattern to copy

`codex_remote.rs` is the best direct precedent in the tree.

It already does the hard structural parts:

- spawn loopback-only local runtime
- maintain auth token
- keep a structured snapshot
- sync snapshot as app state changes
- accept structured action requests
- dispatch them back into app-owned handlers

For `v0.1`, the most pragmatic route is not to invent a brand new IPC model. It is to build a Mission Control / desktop control sibling to `codex_remote`, with a smaller and stricter command surface.

### 4. Mission Control needs a stable structured snapshot, not only rendered text

If an agent is going to control the app, it needs more than:

- copied log text
- free-form status lines
- or parser-fragile label text

The app needs a structured `DesktopControlSnapshot` or equivalent that includes, at minimum:

- app mode and dev-mode status
- Mission Control blocker set
- Apple FM readiness
- provider mode and desired mode
- wallet status, balance, and withdrawal readiness
- buy-mode enabled/disabled, cadence, and current in-flight request
- active job status
- last command result
- last error
- recent event ids and payment pointers where relevant

Without that, CLI control will be brittle and agent-hostile.

### 5. CLI commands should not be raw pane-hit events

It would be tempting to expose raw `PaneHitAction` values or renderer-oriented button ids. That would be a mistake.

Why:

- pane hit actions are UI mechanics
- they include dev-mode and layout concerns the release cut does not need
- they are not a stable product contract

Instead, the app should expose a tighter product-level command model, something like:

- `GetStatus`
- `SetProviderMode(online|offline)`
- `RefreshAppleFm`
- `RunAppleFmSmokeTest`
- `RefreshWallet`
- `StartBuyMode`
- `StopBuyMode`
- `WithdrawLightningInvoice`
- `GetActiveJob`
- `GetBuyModeHistory`
- `CopyMissionControlLog` or better `GetMissionControlLogTail`

Those commands can then dispatch internally to the existing action helpers.

### 6. Control actions must be recorded as first-class runtime events

If an agent controls the app, the operator must be able to see that this happened.

That means every CLI action should generate an app-owned control-domain event that is visible in:

- runtime/session logs
- Mission Control log stream
- command responses

Examples:

- `control.command.received`
- `control.command.applied`
- `control.command.rejected`
- `control.snapshot.synced`

That will make agent-driven activity inspectable instead of invisible.

### 7. The UI and CLI need a shared state revision

A synced control plane should not return only `ok` or `error`.

Each command response should include something like:

- `snapshot_revision`
- `event_seq`
- or `state_signature`

That gives the caller a way to know:

- the command was applied
- the snapshot they are reading is newer than the command
- and the UI should already reflect the same underlying state

Without this, control will feel racey even if the state transitions are correct.

## Recommended Architecture

## 1. Add an app-owned Desktop Control Runtime

Add a new app-owned module under `apps/autopilot-desktop/src/`, for example:

- `desktop_control.rs`

Its role:

- host a local authenticated control server for the running app
- own the transport and command envelope
- keep a structured snapshot mirror of current Mission Control truth
- route control actions into the same app-owned action path used by the UI

For `v0.1`, the best transport choice is:

- loopback-only HTTP with token auth, following the `codex_remote.rs` pattern

Reason:

- already proven in this tree
- easy for a local CLI and for agent tooling
- easy to inspect with `curl`
- no new exotic dependency needed

Unix domain sockets can be a later hardening improvement if desired.

## 2. Define a narrow product command model

Add a dedicated command enum, for example:

- `DesktopControlAction`

Recommended `v0.1` actions:

- `GetSnapshot`
- `SetProviderMode { online: bool }`
- `RefreshAppleFm`
- `RunAppleFmSmokeTest`
- `RefreshWallet`
- `StartBuyMode`
- `StopBuyMode`
- `GetBuyModeStatus`
- `GetBuyModePayments`
- `GetActiveJob`
- `Withdraw { bolt11: String }`
- `GetMissionControlLogTail { limit: usize }`

Optional but useful:

- `WaitFor { condition, timeout_ms }`
- `AcknowledgeAlert { id }`

Do not expose:

- raw pane ids
- widget coordinates
- renderer-derived button identifiers

## 3. Dispatch commands through existing app action helpers

The control plane should not reimplement product logic.

Instead:

- `SetProviderMode { online: true }` should call the same `apply_provider_mode_target(...)` path used by `GoOnlineToggle`
- Mission Control actions should reuse `run_mission_control_action(...)`
- buy-mode commands should reuse the current Mission Control buy-mode logic
- wallet operations should reuse current Spark command queue paths

This is the critical rule that keeps UI and CLI in sync:

- one write path
- many callers

## 4. Add a structured Desktop Control Snapshot

The control plane needs a stable read model, something like:

- `DesktopControlSnapshot`

Recommended top-level sections:

- `session`
- `mission_control`
- `provider`
- `apple_fm`
- `wallet`
- `buy_mode`
- `active_job`
- `recent_logs`
- `last_command`

Each section should expose codes and booleans, not only prose. For example:

- `provider.mode = offline|preview|online|degraded`
- `provider.blockers = [APPLE_FM_UNAVAILABLE, WALLET_UNAVAILABLE]`
- `buy_mode.loop_enabled = true|false`
- `buy_mode.in_flight_request_id`
- `active_job.stage = accepted|running|delivered|settling|paid|failed`
- `wallet.balance_sats`

Mission Control can keep rendering prose from this truth, but the snapshot must not depend on parsing UI strings back into state.

## 5. Add an event stream, not just polling

Polling snapshots is not enough for agent control.

Add a streaming read surface, preferably one of:

- server-sent events
- newline-delimited JSON stream
- or a simple long-poll event cursor

The event stream should expose:

- state changes
- command acks
- lifecycle milestones
- payment events
- provider mode changes
- Apple FM readiness changes

This can be built directly from existing runtime-log and app-state change points.

## 6. Add a first-class CLI client

Add a small app-owned CLI binary, for example:

- `autopilotctl`

Example commands:

- `autopilotctl status`
- `autopilotctl provider online`
- `autopilotctl provider offline`
- `autopilotctl apple-fm refresh`
- `autopilotctl apple-fm smoke-test`
- `autopilotctl wallet refresh`
- `autopilotctl buy-mode start`
- `autopilotctl buy-mode stop`
- `autopilotctl buy-mode status`
- `autopilotctl active-job`
- `autopilotctl logs --tail 50`
- `autopilotctl withdraw <bolt11>`

The CLI should not implement product logic itself. It should be a thin client over the control runtime.

## Synchronization Rules

To keep the UI truthful, the implementation should obey these rules:

### Rule 1: One write path

Every state-mutating CLI action must flow through the same app-owned action path as the UI.

### Rule 2: One read model

Mission Control rendering and CLI snapshots should be fed by the same underlying app-state projections, not two parallel data models.

### Rule 3: No invisible side effects

If the CLI can do it, the Mission Control UI must be able to represent it afterward.

### Rule 4: Every command leaves evidence

Every command must be visible in runtime logs and ideally in Mission Control log history.

### Rule 5: Revisioned responses

Every command response should tell the caller what snapshot revision they are now looking at.

## Suggested Implementation Order

### Phase 1: Minimal `v0.1` control plane

- Introduce `desktop_control.rs`
- Define `DesktopControlAction`
- Define `DesktopControlSnapshot`
- Expose loopback `/v1/snapshot` and `/v1/action`
- Support:
  - provider online/offline
  - Apple FM refresh
  - wallet refresh
  - buy-mode start/stop
  - withdraw
  - active-job snapshot
  - logs tail

This is enough to make the shipping `v0.1` Mission Control loop agent-drivable.

### Phase 2: Snapshot and event quality

- Add snapshot revisioning
- Add structured blocker codes
- Add event stream endpoint
- Add runtime-log control-domain events
- Add richer buy-mode and wallet fields

This is enough to make the control plane reliable for autonomous agents.

### Phase 3: Parity and regression coverage

- Add tests proving UI-click and CLI-command parity for:
  - go online
  - buy-mode start/stop
  - wallet refresh
  - withdraw
- Add tests proving snapshots update after command dispatch
- Add tests proving command events appear in runtime logs
- Reuse headless compute harness where appropriate for payment-path verification

## Acceptance Criteria

The control-plane work should only be considered complete when all of these are true:

- An agent can connect to the running desktop app locally.
- The agent can perform the `v0.1` Mission Control loop from CLI.
- The on-screen Mission Control UI visibly updates after each command.
- CLI status and on-screen status agree.
- Runtime/session logs show the same command and lifecycle truth.
- There is no second hidden state model used only by the CLI.
- The implementation stays inside `apps/autopilot-desktop` except for narrow reusable transport helpers if strictly necessary.

## What Not To Do

- Do not make `autopilot_headless_compute` the control plane for the desktop UI.
- Do not expose renderer coordinates or widget ids as the public CLI contract.
- Do not push Mission Control command semantics into `crates/wgpui`.
- Do not let CLI actions bypass app-owned reducer/action logic.
- Do not require the operator to infer command results only from free-form logs.

## Recommended Improvement Summary

The path to a fully CLI-controllable `v0.1` is not a major rewrite.

The repo already has almost every prerequisite:

- centralized app-owned action handlers
- Mission Control as the product shell
- strong runtime logging
- a headless verification path
- and a proven local remote-control pattern in `codex_remote.rs`

What is missing is the final app-owned bridge:

- a desktop control runtime for Mission Control
- a stable structured snapshot
- a narrow product command set
- and explicit synchronization semantics between command acks, app state, runtime logs, and UI projection

That is the right next step if the goal is to let agents drive the real app while humans can still watch the same truth unfold on screen.
