# Autopilot Clean-Room Rebuild Decisions

Date: 2026-04-22

This is the running decision log for the clean-room Autopilot rebuild prompted
by the first-principles Claude Code teardown memo in the root workspace. The
goal is to keep decisions visible while implementation proceeds, not to create
another strategy-only document.

## Current Target

Autopilot is the product surface. The first rebuild slice is a visual desktop
workbench that models real work as durable objects: workspace, session, turn,
tool event, approval, diff, verification result, evidence record, and resume
state. The UI should not start as a terminal transcript or subsystem dashboard.

The first implementation target is:

> Open Autopilot, see one coherent workbench, understand the current repo,
> session, timeline, approvals, diffs, verification, evidence, and earn state
> without needing to know which internal subsystem owns each piece.

## Decisions Made In This Pass

1. Autopilot defaults to `Workbench`, not `Command Console`.

   Reason: the root memo says the customer should see Autopilot and that the
   flagship experience should be a visual desktop app. A command-console
   landing page preserves the terminal-first assumption.

2. The first workbench contract is owned by Rust/Tauri and projected into
   React.

   Reason: policy, persistence, process authority, and engine state must sit
   below the React UI. React displays and commands the product; it is not the
   authority boundary.

3. The visible product object model starts with workspace, session, timeline,
   approval, diff, verification, evidence, and scorecard.

   Reason: these are the load-bearing primitives needed for repo -> session ->
   tools -> approvals -> diff -> verification -> resume. They are also the
   objects that make agent work understandable as software rather than chat
   text.

4. `Pylon` becomes `Earn Runtime` in the visible Tauri shell.

   Reason: users need the ability to go online and earn sats. They do not need
   the internal runtime name as a first-run product concept.

5. `Proof` moves under `Diagnostics` in the visible Tauri shell.

   Reason: proof lanes matter for operator verification, but they should not be
   part of the default customer mental model.

6. The current implementation is deliberately a clean-room shell slice, not a
   full agent-engine replacement.

   Reason: replacing the engine in one pass would turn this into a broad
   rewrite. The correct next step is to define the narrow Autopilot-to-engine
   contract and make the workbench consume that contract.

## Keep

- Tauri as the current desktop shell hypothesis.
- React/Tailwind as the current fastest UI hypothesis.
- Rust-side authority for process, policy, snapshot, and future engine control.
- Existing Pylon/proof controls as diagnostics while the new workbench becomes
  the default product surface.

## Cut Or Hide

- Do not make the command console the default surface.
- Do not expose Pylon, Nexus, Probe, Forge, or Psionic as first-run product
  nouns in the workbench.
- Do not represent internal topology as navigation unless it helps the user
  complete or trust work.
- Do not add decorative UI that does not carry state.

## Next Decisions

1. Define the real `Autopilot UI <-> agent engine` event schema.
2. Decide whether the first engine adapter attaches to Codex, Probe, or a small
   in-repo harness for one task loop.
3. Move the workbench snapshot from demo data to persisted local session state.
4. Add typed approval request and diff artifact commands behind the workbench.
5. Add a programmatic control check so `autopilotctl-tauri` can verify the
   workbench snapshot, not only Pylon/proof diagnostics.

## Implementation Log

2026-04-22:

- Added `autopilot_workbench_snapshot` as a Rust/Tauri command. It returns the
  first clean-room workbench contract: visible product, workspace, session,
  timeline, approvals, diffs, verification, evidence, and scorecard.
- Made `Workbench` the default `apps/autopilot` view instead of the command
  console.
- Added the visual workbench surface in React: left rail for workspace and
  scorecard, center timeline for the active session, and right inspector for
  approvals, diffs, verification, and evidence.
- Renamed visible `Pylon` navigation to `Earn Runtime` while keeping the
  existing internal action IDs and Tauri commands intact.
- Moved visible proof controls under `Diagnostics` while keeping the existing
  local proof command path intact.
- Kept the new scorecard honest: it starts at zero until real session and earn
  data are wired into the workbench snapshot.
- Kept the legacy diagnostics implementation names internal for this slice.
  The customer-facing default surface is the Autopilot workbench; a deeper
  internal rename can happen after the workbench-to-engine contract is real.
- Tightened the diagnostics accessibility label so the visible shell no longer
  describes that panel as proof-first.

Verification run:

- `cargo fmt -p autopilot`
- `cd apps/autopilot && bun run build`
- `cargo check -p autopilot`
- `cargo test -p autopilot --lib`
- `scripts/autopilot/tauri-control-smoke.sh --status-only`

All verification above passed on this implementation pass. The frontend build
still emits the existing Vite chunk-size warning; that is not caused by the
workbench contract and should be handled as a separate bundling optimization.
