# 2026-03-13 Autopilot Desktop Perf Opportunities Audit

## Purpose

This is a second-pass performance audit for `apps/autopilot-desktop`.

The earlier audits on 2026-03-13 established:

- what had recently been fixed,
- what caused the worst startup / pane-churn beachballs,
- and what the supported harness can now measure.

This audit looks for the next layer of opportunities:

- steady-state redraw waste,
- duplicated background work,
- snapshot / projection refactors,
- and app-owned architecture changes that should make future regressions less
  likely.

## Inputs Reviewed

Product / ownership:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/headless-compute.md`
- `docs/audits/2026-03-13-autopilot-desktop-perf-harness-audit.md`
- `docs/audits/2026-03-13-autopilot-desktop-perf-remediation-audit.md`

Desktop runtime / render loop:

- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/panes/provider_control.rs`
- `apps/autopilot-desktop/src/panes/earnings_jobs.rs`
- `apps/autopilot-desktop/src/panes/presentation.rs`
- `apps/autopilot-desktop/src/panes/rive.rs`
- `apps/autopilot-desktop/src/panes/frame_debugger.rs`

State / projection hot paths:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/autopilot_peer_roster.rs`
- `apps/autopilot-desktop/src/state/nip90_payment_facts.rs`

Control / mirror runtimes:

- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/provider_admin.rs`
- `apps/autopilot-desktop/src/codex_remote.rs`

## Status Notes

- 2026-03-14: `#3532` is implemented. `desktop_control`, `provider_admin`, and
  `codex_remote` now share an app-owned snapshot-domain invalidation layer in
  `apps/autopilot-desktop/src/snapshot_domains.rs`, and desktop-control action
  responses reuse an already-built snapshot when the payload path needs both the
  payload and response metadata.
- 2026-03-14: `#3533` is implemented. Autopilot peer roster and buy-mode target
  selection now consume an app-owned incremental presence index instead of
  replaying every confirmed message in the configured main channel on hot-path
  queries.
- 2026-03-14: `#3534` is implemented. `PaneRenderer` now has an explicit
  inactive-pane render policy, and the heaviest inactive desktop panes
  (`Provider Control`, `Autopilot Chat`, `Log Stream`, payment visualization
  panes, and diagnostics panes) now render through cheap, app-owned preview
  summaries instead of always paying full paint cost.
- 2026-03-14: `#3535` is implemented. `Presentation` and `Rive Preview` now
  track last-applied controller fit/play state in runtime state and only mutate
  the Rive controller when desired state actually changes, while cadence/debug
  snapshots now distinguish controller-sync-pending from truly settled surfaces.

## Executive Summary

The next big responsiveness wins are no longer in obvious startup bugs. They
are in steady-state architecture.

The most important remaining issues are:

1. the app still has at least one clear self-inflicted continuous redraw loop,
2. background pumps can run twice per visual frame,
3. three separate control/mirror runtimes still rebuild and serialize large
   snapshots independently,
4. some important state projections still rescan whole histories whenever their
   cache invalidates,
5. the pane renderer still pays too much for inactive panes.

If we fix only one thing next, it should be the steady-state redraw policy for
provider online mode. That looks like the clearest “the app is busy for no good
reason” path left in the current code.

If we fix the next structural thing after that, it should be unifying snapshot
and projection invalidation across `desktop_control`, `provider_admin`, and
`codex_remote`.

## Findings

### 1. `ProviderMode::Online` is currently treated like a live animation source

Relevant code:

- `apps/autopilot-desktop/src/input.rs`
  - `build_frame_redraw_pressure_snapshot(...)`
  - `should_request_desktop_redraw(...)`
  - `WindowEvent::RedrawRequested`

Current behavior:

- `provider_animating` is true when provider mode is either
  `Connecting` or `Online`,
- `handle_about_to_wait(...)` requests redraw whenever `provider_animating` is
  true,
- `WindowEvent::RedrawRequested` also requests another redraw whenever
  `provider_animating` is true.

Why this is a problem:

- `Connecting` is plausibly animated,
- `Online` is not an animation state; it is a steady-state mode,
- this means the app can keep repainting continuously simply because the
  provider is online, even if nothing visible changed.

This is the strongest remaining candidate for “why does the app still feel
hot / busy even when it looks idle.”

Recommended fix:

- split `provider_animating` into:
  - transition animation state,
  - heartbeat / liveness state,
  - and true event-driven invalidation,
- treat `ProviderMode::Online` as event-driven by default,
- if the UI needs a heartbeat, tick it at a coarse cadence such as `1 Hz`
  rather than a continuous redraw loop.

Recommended refactor:

- create an app-owned invalidation enum like:
  - `ImmediateAnimation`
  - `CoarseTicker`
  - `EventDriven`
- and make each desktop subsystem declare which one it needs.

### 2. Background state pumping still happens in two loop phases

Relevant code:

- `apps/autopilot-desktop/src/input.rs`
  - `handle_about_to_wait(...)`
  - `pump_background_state(...)`
  - `WindowEvent::RedrawRequested`

Current behavior:

- `pump_background_state(...)` runs in `handle_about_to_wait(...)`,
- it also runs again inside `WindowEvent::RedrawRequested`.

Why this is a problem:

- during redraw-heavy periods, background lanes can be pumped twice per frame,
- the more the app redraws, the more background work it can accidentally do,
- that is the wrong feedback loop for a responsive desktop app.

This is especially bad when combined with:

- online-mode continuous redraw,
- live Rive surfaces,
- or the frame debugger being open.

Recommended fix:

- separate background work into cadence buckets:
  - every-frame drain-only work,
  - fast interval work,
  - coarse interval work,
  - and truly on-demand work,
- run only one scheduled background pump per event-loop cycle,
- keep redraw handling focused on presentation.

Recommended refactor:

- replace the single `pump_background_state(...)` with a small scheduler that
  owns `next_due_at` timestamps for each subsystem.

### 3. `provider_admin` and `codex_remote` still use the old expensive snapshot pattern

Relevant code:

- `apps/autopilot-desktop/src/provider_admin.rs`
  - `sync_runtime_snapshot(...)`
  - `snapshot_for_state(...)`
  - `snapshot_signature(...)`
- `apps/autopilot-desktop/src/codex_remote.rs`
  - `sync_runtime_snapshot(...)`
  - `snapshot_for_state(...)`

Current behavior:

- `desktop_control` was improved to gate work before building a full snapshot,
- `provider_admin` still builds a full persisted snapshot and hashes it before
  deciding whether to sync,
- `codex_remote` still builds and serializes a full snapshot before deciding
  whether to sync.

Why this is a problem:

- `pump_background_state(...)` calls all three runtimes,
- they all observe overlapping app state,
- and they all independently rebuild projections and serialize snapshots.

This is duplicated work and duplicated architecture.

Recommended fix:

- move snapshot invalidation to shared app-owned domain revisions,
- let each runtime ask whether relevant domains changed before building payloads,
- avoid full serialization for “did anything change” checks.

Recommended refactor:

- add a `ProjectionHub` or `SnapshotDomains` service in
  `apps/autopilot-desktop` that owns:
  - dirty flags,
  - per-domain revision counters,
  - cached projection payloads,
  - and stable signatures derived from domain revisions instead of full JSON.

### 4. Control actions still sometimes build the full desktop snapshot twice

Relevant code:

- `apps/autopilot-desktop/src/desktop_control.rs`
  - `apply_action_request(...)`
  - `snapshot_payload_response(...)`
  - `attach_snapshot_metadata(...)`

Current behavior:

- some action handlers build a snapshot payload,
- then the response path attaches snapshot metadata by building another full
  snapshot.

Why this matters:

- this is not the biggest source of lag in normal idle rendering,
- but it is unnecessary work on the interactive control path,
- and it is evidence that snapshot construction is still too ad hoc.

Recommended fix:

- pass the already-built snapshot through the response metadata path,
- or make action responses consume a shared precomputed revision/signature.

This is a good cleanup target once snapshot ownership is centralized.

### 5. Autopilot peer roster still rescans the full configured main channel history

Relevant code:

- `apps/autopilot-desktop/src/autopilot_peer_roster.rs`
  - `build_autopilot_peer_roster(...)`
  - `parse_autopilot_compute_presence_message(...)`
- `apps/autopilot-desktop/src/app_state.rs`
  - `autopilot_peer_roster(...)`
  - `select_autopilot_buy_mode_target(...)`

Current behavior:

- roster construction iterates every confirmed message in the configured main
  channel,
- it reparses presence payloads from message text,
- cache invalidation is keyed by managed-chat projection revision.

Why this is a problem:

- any new managed-chat message can invalidate the whole roster cache,
- a long-lived channel means repeated whole-history rescans,
- presence data is semantically indexable but currently recomputed by replaying
  the channel.

This is a classic derived-state indexing opportunity.

Recommended fix:

- maintain an incremental `AutopilotPresenceIndex` owned by app state,
- update it when managed-chat projection changes,
- store latest per-peer presence, latest chat activity, and buy-mode
  eligibility summary directly.

Recommended refactor:

- make buy-mode target selection operate on indexed peer rows instead of
  rescanning channel history.

### 6. Pane rendering is still too “paint everything every frame”

Relevant code:

- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/panes/earnings_jobs.rs`

Current behavior:

- the renderer iterates all open panes every frame,
- most pane kinds still paint their full contents even when inactive,
- `Earnings & Jobs` already has a lighter inactive preview, which proves the
  approach is useful but not yet generalized.

Why this is a problem:

- the more panes are open, the more full-fidelity paint work is done,
- text-heavy panes still pay shaping/layout cost when they are not the current
  focus,
- the cost scales with the app surface area, not just the user’s active pane.

Recommended fix:

- add an app-owned inactive-pane rendering policy:
  - `full`
  - `summary`
  - `cached`
- migrate the heaviest panes first:
  - `Provider Control`
  - `Autopilot Chat`
  - `Log Stream`
  - payment visualization panes
  - diagnostics panes

Recommended refactor:

- let pane modules expose both `paint_active(...)` and `paint_inactive_preview(...)`
  or equivalent,
- and keep the current full-paint path only for panes that are actually visible
  enough to justify it.

### 7. Rive pane state is mutated on every paint

Relevant code:

- `apps/autopilot-desktop/src/panes/presentation.rs`
  - `sync_runtime_state(...)`
- `apps/autopilot-desktop/src/panes/rive.rs`
  - `sync_runtime_state(...)`

Current behavior:

- `Presentation` unconditionally sets fit mode and calls `play()` on every
  paint,
- `Rive Preview` unconditionally sets fit mode and calls `play()` or `pause()`
  on every paint.

Why this is risky:

- even if the underlying Rive controller de-dupes redundant state changes,
  this is still unnecessary control churn,
- if controller mutation marks the surface dirty, it can extend redraw life
  longer than needed,
- it makes it harder to reason about when a surface should be considered
  settled.

Recommended fix:

- track last-applied controller state in runtime state,
- only call controller mutators when the desired state actually changes,
- expose a clearer “surface settled and no pending controller change” signal to
  the redraw policy.

### 8. Snapshot signatures still rely on full JSON serialization

Relevant code:

- `apps/autopilot-desktop/src/desktop_control.rs`
  - `snapshot_sync_signature(...)`
- `apps/autopilot-desktop/src/provider_admin.rs`
  - `snapshot_signature(...)`
- `apps/autopilot-desktop/src/codex_remote.rs`
  - inline `serde_json::to_string(...)` hashing in `sync_runtime_snapshot(...)`

Current behavior:

- stable signatures are computed by cloning a snapshot,
- zeroing or normalizing volatile fields,
- serializing to JSON,
- then hashing the JSON string.

Why this is a problem:

- serialization cost grows with snapshot size,
- the app is doing “compute full payload so I can decide whether to compute full
  payload”,
- and three runtimes each repeat the same pattern.

Recommended fix:

- replace “hash serialized payload” with “hash domain revisions + stable leaf
  counters,”
- reserve full serialization for the actual sync/write path.

This is a high-value refactor because it reduces both CPU and architectural
duplication.

### 9. Managed chat convenience accessors still allocate and linearly search

Relevant code:

- `apps/autopilot-desktop/src/app_state.rs`
  - `active_managed_chat_channels(...)`
  - `active_managed_chat_messages(...)`
  - `active_managed_chat_channel(...)`

Current behavior:

- channel/group selection repeatedly linearly searches snapshot vectors,
- some convenience accessors allocate `Vec`s of references.

This is not currently the top bottleneck, but it is part of a pattern:

- high-level UI and control code asks simple semantic questions,
- and the answers are often computed by rebuilding small collections from the
  projection snapshot.

Recommended fix:

- add a lightweight selected-group/channel view cache or index inside
  `AutopilotChatState`,
- make hot-path consumers ask for slices / ids / tails instead of allocating
  new `Vec`s where possible.

### 10. The app still lacks per-pane paint timing and per-runtime pump timing

Relevant code:

- `apps/autopilot-desktop/src/panes/frame_debugger.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/render.rs`

What is missing:

- per-pane CPU paint timing,
- per-runtime background pump timing,
- clear attribution for snapshot build cost by subsystem,
- a long-lived in-memory timing ring buffer available without opening a probe
  pane.

Why this matters:

- the next performance bugs are likely to be “one pane” or “one mirror runtime”
  problems,
- current diagnostics still lean toward whole-frame summaries.

Recommended fix:

- instrument:
  - each pane paint function,
  - each `pump_runtime(...)` branch,
  - each snapshot build/signature path,
  - and each worker-drain stage,
- expose the result both in `Frame Debugger` and through `autopilotctl`.

## Recommended Refactor Program

### Phase 1: Stop unnecessary steady-state redraw

Implement first:

- separate provider transition animation from online steady state,
- stop continuous redraw when online unless a visible animation is active,
- remove redundant controller mutations for Rive panes.

Expected payoff:

- immediate reduction in idle CPU/GPU churn,
- lower event-loop pressure,
- less chance of beachball-like stalls while the app “looks idle.”

### Phase 2: Introduce a background work scheduler

Implement next:

- replace the monolithic `pump_background_state(...)` with cadence buckets,
- make each subsystem explicit about whether it needs:
  - drain-every-loop,
  - fast poll,
  - coarse poll,
  - or event-triggered work,
- eliminate duplicate pumps in both `about_to_wait` and redraw.

Expected payoff:

- lower steady-state contention,
- better control over how much work animation can accidentally trigger.

### Phase 3: Centralize snapshot / projection invalidation

Implement next:

- create a shared projection/snapshot invalidation hub,
- move `desktop_control`, `provider_admin`, and `codex_remote` onto it,
- use revision-based signatures instead of hashing full JSON snapshots.

Expected payoff:

- less duplicated CPU,
- less duplicated architecture,
- easier reasoning about what actually changed.

### Phase 4: Build incremental indexes for long-lived histories

Implement next:

- add incremental peer presence / buy-mode eligibility indexes,
- continue moving historical replay logic out of hot paths,
- cache selected managed channel/group lookups.

Expected payoff:

- performance that scales with recent change rather than total retained history.

### Phase 5: Generalize inactive-pane previews and scene caching

Implement next:

- extend the `Earnings & Jobs` inactive-preview strategy to other heavy panes,
- add pane-level summary/cached render modes,
- eventually support reusable scene snapshots if needed in `wgpui`.

Expected payoff:

- lower frame cost proportional to number of open panes,
- much better responsiveness in multi-pane desktop layouts.

## Measurement Work To Add Alongside Refactors

The next audit cycle should be able to answer these questions directly:

- What is the app’s idle redraw rate when provider mode is online but visually
  static?
- How much CPU time does each background subsystem consume per second?
- Which pane has the highest mean and p95 paint cost?
- Which snapshot runtime is consuming the most serialization time?
- How does pane count change frame time?

To get there, add:

- per-pane paint timers,
- per-runtime pump timers,
- per-snapshot build and signature timers,
- a no-observer-effect timing ring buffer,
- `autopilotctl perf` scenarios that emit JSON summaries.

## Bottom Line

The previous performance fixes removed several worst-case stalls. The next
layer is about not wasting work in the first place.

The biggest remaining opportunities are:

- stop continuous redraw in steady online mode,
- stop pumping all background lanes twice under redraw pressure,
- stop rebuilding three overlapping snapshots independently,
- stop rescanning entire histories for derived peer/buy-mode state,
- and stop painting inactive panes at full fidelity by default.

Those are not micro-optimizations. They are the next major architecture steps
to make the desktop app feel predictably fast instead of “usually okay until it
isn’t.”
