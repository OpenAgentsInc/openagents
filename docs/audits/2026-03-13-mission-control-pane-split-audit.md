# Mission Control Pane Split Audit

Date: 2026-03-13
Branch audited: `main`
Audit type: architecture analysis of the current monolithic Mission Control pane and a path forward for splitting it into focused hotbar panes

## Audit Question

Mission Control is currently a single fullscreen surface that owns everything
in the `v0.1` earn loop. That was the right call for the initial release cut:
one screen, no pane maze, no distractions. But now that the loop is proven and
the codebase has matured, Mission Control has become a monolith. It mixes
provider orchestration, wallet truth, job lifecycle, buy-mode smoke testing,
earnings visibility, local-runtime management, and log streaming into one
two-column layout.

This audit asks:

- What does Mission Control currently own and why?
- What are the costs of keeping it monolithic?
- How should it be decomposed into focused panes on the hotbar?
- What happens to the "dev mode" distinction after the split?
- What is the concrete implementation path?

## Scope

Primary docs reviewed:

- `docs/v01.md`
- `docs/plans/mission-control-pane.md`
- `docs/PANES.md`
- `docs/OWNERSHIP.md`
- `docs/audits/2026-03-11-nip90-compute-mission-control-implementation-audit.md`

Primary code reviewed:

- `apps/autopilot-desktop/src/app_state.rs` (PaneKind, MissionControlPaneState, data sources)
- `apps/autopilot-desktop/src/pane_registry.rs` (PaneSpec, hotbar slots, startup)
- `apps/autopilot-desktop/src/pane_system.rs` (MissionControlPaneAction, layout, hit testing)
- `apps/autopilot-desktop/src/pane_renderer.rs` (paint_go_online_pane, ~840 lines of MC rendering)
- `apps/autopilot-desktop/src/input/actions.rs` (MC action dispatch)
- `apps/autopilot-desktop/src/input/shortcuts.rs` (MC input focus)
- `apps/autopilot-desktop/src/desktop_shell.rs` (DesktopShellMode, dev mode gate)
- `apps/autopilot-desktop/src/render.rs` (shell chrome, hotbar, grid gating)

## Executive Summary

Mission Control was designed as a one-screen earn-first shell for `v0.1`. It
succeeded at that job. But the cost is now visible:

- `paint_go_online_pane` alone is ~840 lines of rendering code covering six
  conceptually distinct sections.
- `MissionControlPaneState` holds scroll offsets, input fields, buy-mode loop
  state, log streams, and wallet action state for domains that should be
  independent.
- `MissionControlPaneAction` is a grab-bag enum of 12 actions spanning wallet
  operations, local-model management, buy-mode toggling, documentation, and
  log copying.
- The fullscreen presentation forces every concern to compete for vertical
  space inside two fixed columns instead of being independently resizable.
- The `OPENAGENTS_ENABLE_DEV_MODE` gate creates a hard binary between "one
  giant pane" and "full workspace with 42 panes." There is no middle ground.

The path forward is to decompose Mission Control into a small set of focused
panes, each assigned a hotbar slot, and remove the dev-mode distinction
entirely. The production shell becomes the multi-pane hotbar shell, not because
we want a pane maze, but because the earn loop itself has multiple natural
surfaces that should be independently visible and independently scrollable.

## What Mission Control Currently Owns

Based on the code in `pane_renderer.rs:paint_go_online_pane` and the plan in
`docs/plans/mission-control-pane.md`, Mission Control currently renders the
following sections in a two-column layout:

### Left Column

1. **Sell Compute** -- provider mode toggle (`GO ONLINE`), status rows for
   Mode, Model, Backend, Load, Control, Preflight, and a contextual hint when
   the local lane is blocked.
2. **Earnings** -- Today, This Month, All Time sats display.
3. **Wallet** -- Status, Address, Balance, inline Lightning withdraw input and
   send action, load-funds / receive-address area.
4. **Actions** -- Local-model action (start/refresh Apple FM or GPT-OSS
   workbench), copy seed phrase, open documentation.

### Right Column

5. **Active Jobs** -- One-line active job summary with lifecycle state, or idle
   placeholder.
6. **Buy Mode** -- Start/stop loop, inline lifecycle status rows (State, Kind,
   Budget, Result, Settlement), payment history link.
7. **Log Stream** -- Scrolling terminal-style log with copy action.

### Data Sources (from `docs/plans/mission-control-pane.md`)

- `ProviderRuntimeState` for provider mode and blockers
- `AppleFmBridgeSnapshot` / `ProviderAppleFmRuntimeState` for Apple FM truth
- `LocalInferenceExecutionSnapshot` / `ProviderGptOssRuntimeState` for GPT-OSS
- `local_runtime_capabilities.rs` for shared local-runtime capability model
- `SparkPaneState` for wallet truth
- `NetworkRequestsState` / `NetworkRequestsPaneInputs` for buy-mode lifecycle
- `EarnJobLifecycleProjectionState`, `JobInboxState`, `ActiveJobState` for jobs

## Costs of the Current Monolith

### 1. Rendering complexity concentration

`paint_go_online_pane` is one of the largest rendering functions in the
codebase. It handles six conceptual sections with independent scroll regions,
conditional buy-mode rendering, multiple icon-button hit zones, and section
layout calculations that depend on each other. Any change to one section risks
breaking the layout of another.

### 2. State bloat in MissionControlPaneState

`MissionControlPaneState` (defined in `app_state.rs`) currently holds:

- Six independent scroll offsets (`sell_scroll_offset`, `earnings_scroll_offset`,
  `wallet_scroll_offset`, `actions_scroll_offset`, `load_funds_scroll_offset`,
  `active_jobs_scroll_offset`)
- Three text inputs (`load_funds_amount_sats`, `send_invoice`, `withdraw_invoice`)
- Buy-mode loop state (`buy_mode_loop_armed`, `buy_mode_next_dispatch`,
  `buy_mode_dispatch_count`, `buy_mode_last_blocked_notice`)
- Log stream state (`log_lines`, `persisted_log_lines`, `runtime_terminal`)
- Icon click feedback timers
- Local FM summary test state

This state belongs to at least four distinct domain surfaces. Mixing it in one
struct makes it harder to reason about lifecycle, initialization, and cleanup.

### 3. The action enum is too broad

`MissionControlPaneAction` contains:

```
RefreshWallet, CreateLightningReceiveTarget, CopyLightningReceiveTarget,
CopyLogStream, SendLightningPayment, CopySeedPhrase,
OpenLocalModelWorkbench, RunLocalFmSummaryTest, ToggleBuyModeLoop,
OpenBuyModePayments, SendWithdrawal, OpenDocumentation
```

These span wallet operations, local-model management, buy-mode control, and
utility actions. A single action dispatch path handles all of them, which makes
the input reducer harder to follow than it needs to be.

### 4. The dev-mode gate is too coarse

`DesktopShellMode` is a binary `Production | Dev`. In production mode, Mission
Control is fullscreen with no hotbar, no command palette, no other panes. In
dev mode, the full 42-pane workspace appears. There is no graduated middle
ground where a user can see, say, just the wallet and the provider status
alongside their active jobs.

This binary split also means the production release cannot evolve toward a
multi-pane layout without first removing the gate entirely or introducing a
third mode.

### 5. Vertical space pressure

All seven sections compete for space in two columns. The log stream, which
benefits most from vertical space, is compressed into whatever remains after
sell-compute, earnings, wallet, active jobs, and buy mode have claimed their
rows. The earnings section is always visible even when the user has been online
for five minutes and has no earnings yet.

## Proposed Split: Five Hotbar Panes

The monolithic Mission Control should be decomposed into the following focused
panes, each available as a hotbar slot:

### Pane 1: Provider Control (hotbar slot, singleton, startup)

Owns:

- `GO ONLINE` / `GO OFFLINE` toggle
- Provider mode, status rows (Mode, Model, Backend, Load, Control, Preflight)
- Contextual blocker hints
- Local-model action (start/refresh Apple FM, GPT-OSS workbench link)
- Provider inventory toggles (from current `ProviderStatus` pane)

Data sources: `ProviderRuntimeState`, `AppleFmBridgeSnapshot`,
`LocalInferenceExecutionSnapshot`, `local_runtime_capabilities.rs`

This is the core "go online and stay online" surface. It replaces the current
left-column "Sell Compute" section plus the local-model action from the
"Actions" section.

### Pane 2: Wallet (hotbar slot, singleton)

Owns:

- Wallet status, address, balance
- Lightning withdraw input and send action
- Load funds / receive address
- Seed phrase copy (or move to Settings)

Data sources: `SparkPaneState`

This already exists as the `Spark Lightning Wallet` pane (`PaneKind::SparkWallet`,
hotbar slot 3). The split simply promotes it to be the canonical wallet
surface instead of duplicating wallet controls inside Mission Control.

The inline withdraw input currently in Mission Control
(`mission_control.withdraw_invoice`, `mission_control.send_invoice`) moves
into this pane or the existing `Pay Lightning Invoice` pane.

### Pane 3: Earnings and Jobs (hotbar slot, singleton)

Owns:

- Earnings: Today, This Month, All Time
- Active job summary with full lifecycle detail
- Job inbox preview (count, latest request)
- Job history link or inline recent-jobs list

Data sources: `EarningsScoreboardState`, `EarnJobLifecycleProjectionState`,
`JobInboxState`, `ActiveJobState`

This merges the current "Earnings" section, "Active Jobs" section, and the
existing `Earnings Scoreboard` pane into one focused earn-visibility surface.

### Pane 4: Buy Mode (hotbar slot, singleton, feature-gated)

Owns:

- Start/stop buy-mode loop
- Inline lifecycle: published -> feedback -> result -> invoice paid -> settled/failed
- Payment history
- Buy-mode dispatch cadence and targeting status

Data sources: `NetworkRequestsState`, `SparkPaneState` (for buyer wallet
settlement)

This extracts the current right-column "Buy Mode" section into its own pane.
The existing `Buy Mode Payments` pane (`PaneKind::BuyModePayments`) can be
merged into this pane or kept as a detail drill-down.

Feature gate: this pane can remain behind `OPENAGENTS_ENABLE_BUY_MODE` or
simply be visible by default as current code already does. The important
change is that it no longer competes for space inside the provider control
surface.

### Pane 5: Log Stream (hotbar slot, singleton)

Owns:

- Scrolling terminal-style log stream
- Copy-all action
- Filter controls (future: by domain)

Data sources: `MissionControlPaneState.log_lines`,
`MissionControlPaneState.runtime_terminal`, runtime log mirroring from
`runtime_log.rs`

This extracts the current right-column "Log Stream" into its own
independently-scrollable pane. The log stream is the section that benefits
most from having its own vertical space.

## Removing the Dev Mode Distinction

### Current state

`desktop_shell.rs` defines `DesktopShellMode::Production` vs `Dev`. The gate
is `OPENAGENTS_ENABLE_DEV_MODE=1`. In production mode:

- Mission Control is fullscreen
- No hotbar
- No dots-grid workspace chrome
- No command palette
- No floating-pane frame for Mission Control

In dev mode:

- Full pane workspace with hotbar, grid, command palette
- All 42 panes available

### Proposed change

Remove the binary `Production | Dev` distinction. The default shell becomes:

- **Hotbar** visible by default, with the five focused panes above as the
  default hotbar slots
- **No dots-grid background** (the grid was never useful; it was visual filler)
- **Command palette** available via `K` shortcut for power users to open any
  of the remaining panes (relay connections, sync health, agent panes, etc.)
- **No fullscreen enforcement** -- panes open as resizable windows by default

The remaining 37 panes that are not on the hotbar stay accessible through the
command palette. They are not hidden behind a dev-mode gate; they are simply
not promoted to the hotbar.

This means:

- `DesktopShellMode` enum can be removed or simplified
- `OPENAGENTS_ENABLE_DEV_MODE` env var is no longer needed
- The `PanePresentation::Fullscreen` path for Mission Control is no longer the
  production default
- The dots-grid can be removed entirely or kept as an opt-in aesthetic toggle

### What "dev mode" panes become

Panes that were previously dev-mode-only (workbenches, debug panes, protocol
diagnostics) simply become command-palette-accessible panes. No gate needed.
The distinction between "production" and "dev" was always about UI chrome, not
about pane availability. Once the chrome is unified, the gate has no purpose.

## Implementation Path

### Phase 1: Extract Provider Control pane

1. Create `PaneKind::ProviderControl` (or repurpose `PaneKind::GoOnline`).
2. Move the sell-compute section rendering from `paint_go_online_pane` into a
   new `panes/provider_control.rs` module.
3. Move the local-model action rendering into the same module.
4. Create a `ProviderControlPaneState` struct with only the provider-relevant
   scroll offset and state.
5. Create a `ProviderControlPaneAction` enum with only provider-relevant
   actions (`ToggleOnline`, `RefreshLocalModel`, `OpenWorkbench`).
6. Register the pane in `pane_registry.rs` with a hotbar slot (slot 1 or a new
   slot layout).
7. Mark it as `singleton: true, startup: true`.

Key files:
- `apps/autopilot-desktop/src/app_state.rs` (new state struct)
- `apps/autopilot-desktop/src/pane_registry.rs` (new PaneSpec)
- `apps/autopilot-desktop/src/pane_system.rs` (new action enum, layout, hit testing)
- `apps/autopilot-desktop/src/pane_renderer.rs` (move rendering)
- `apps/autopilot-desktop/src/panes/provider_control.rs` (new module)

### Phase 2: Promote Wallet pane to hotbar default

1. The existing `SparkWallet` pane already exists at hotbar slot 3.
2. Remove the duplicated wallet controls from the old Mission Control state.
3. Remove `MissionControlPaneState.load_funds_amount_sats`,
   `send_invoice`, `withdraw_invoice` and the corresponding scroll offsets.
4. Update `input/shortcuts.rs` to remove `mission_control_inputs_focused`
   references to those fields.
5. Ensure the wallet pane's withdraw flow works standalone without Mission
   Control being open.

Key files:
- `apps/autopilot-desktop/src/app_state.rs` (remove fields from MC state)
- `apps/autopilot-desktop/src/input/shortcuts.rs` (update focus checks)
- `apps/autopilot-desktop/src/pane_system.rs` (remove MC wallet hit zones)

### Phase 3: Create Earnings and Jobs pane

1. Create `PaneKind::EarningsJobs` (or extend/replace `EarningsScoreboard`).
2. Move earnings display, active job summary, and job inbox preview into a
   new `panes/earnings_jobs.rs` module.
3. Create corresponding state and action types.
4. Register on hotbar.

Key files:
- `apps/autopilot-desktop/src/panes/earnings_jobs.rs` (new module)
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`

### Phase 4: Extract Buy Mode pane

1. Create `PaneKind::BuyMode` (or repurpose `BuyModePayments`).
2. Move buy-mode loop state out of `MissionControlPaneState` into a dedicated
   `BuyModePaneState`.
3. Move buy-mode rendering from `paint_go_online_pane` and
   `paint_mission_control_buy_mode_panel` into `panes/buy_mode.rs`.
4. Register on hotbar (conditional on feature gate if desired).

Key files:
- `apps/autopilot-desktop/src/panes/buy_mode.rs` (new or extended module)
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`

### Phase 5: Extract Log Stream pane

1. Create `PaneKind::LogStream`.
2. Move log stream state (`log_lines`, `persisted_log_lines`,
   `runtime_terminal`) out of `MissionControlPaneState`.
3. Move log rendering into `panes/log_stream.rs`.
4. Register on hotbar.

Key files:
- `apps/autopilot-desktop/src/panes/log_stream.rs` (new module)
- `apps/autopilot-desktop/src/app_state.rs`

### Phase 6: Remove dev mode gate and unify shell

1. Remove `DesktopShellMode` enum or collapse it to a single mode.
2. Remove `OPENAGENTS_ENABLE_DEV_MODE` env var handling.
3. Make the hotbar the default shell chrome.
4. Remove or gate the dots-grid behind an aesthetic toggle.
5. Update `render.rs` to always render the hotbar and command palette.
6. Remove the `PanePresentation::Fullscreen` path for the startup pane.
7. Update `pane_registry.rs` hotbar slot assignments to reflect the new five-pane layout.
8. Update the startup pane set: instead of one fullscreen `GoOnline`, open
   `ProviderControl` (or whatever it becomes) as the focused startup pane in a
   normal windowed presentation alongside the hotbar.

Key files:
- `apps/autopilot-desktop/src/desktop_shell.rs` (simplify or remove)
- `apps/autopilot-desktop/src/render.rs` (remove dev-mode gating)
- `apps/autopilot-desktop/src/input.rs` (remove dev-mode gating)
- `apps/autopilot-desktop/src/pane_registry.rs` (update hotbar slots)
- `apps/autopilot-desktop/src/pane_system.rs` (remove fullscreen path)

### Phase 7: Clean up residual Mission Control code

1. Remove or minimize `MissionControlPaneState` -- it should either be deleted
   entirely or reduced to a thin adapter if any cross-pane coordination state
   is still needed.
2. Remove `paint_go_online_pane` and associated layout/hit-testing helpers.
3. Remove `MissionControlPaneAction` enum.
4. Update tests in `pane_registry.rs` that assert Mission Control is the
   singleton startup pane.

## Proposed Hotbar Layout

| Slot | Pane              | Icon | Shortcut |
|------|-------------------|------|----------|
| 1    | Provider Control  | `>`  | `1`      |
| 2    | Nostr Keys        | `N`  | `2`      |
| 3    | Wallet            | `S`  | `3`      |
| 4    | Earnings & Jobs   | `E`  | `4`      |
| 5    | Log Stream        | `L`  | `5`      |
| K    | Command Palette   | `K`  | `K`      |

Buy Mode can occupy slot 6 or remain command-palette-only depending on product
preference.

Chat (`<>`, slot 1 currently) can shift to the command palette or get a
dedicated shortcut. The hotbar should prioritize the earn loop, not the chat
surface, for the near-term product.

## Risks and Mitigations

### Risk: Pane coordination

Some current Mission Control behavior depends on cross-section visibility.
For example, the log stream mirrors provider-mode changes, and the earnings
section reacts to active-job settlement. After the split, these sections live
in different panes that may or may not be open simultaneously.

**Mitigation:** The underlying state sources (`ProviderRuntimeState`,
`SparkPaneState`, `EarnJobLifecycleProjectionState`) are already independent
of Mission Control. The pane split does not change state flow; it only changes
which pane renders which slice of state.

### Risk: Hotbar discoverability

Users accustomed to the one-screen Mission Control may not realize they need
to open additional panes to see earnings or logs.

**Mitigation:** The startup pane set should open both Provider Control and
Earnings & Jobs by default. The hotbar with labeled slots provides clear
affordance for the other surfaces. This is strictly better than the current
state where dev-mode users already need to discover panes via command palette.

### Risk: Regression in the earn loop

The `v0.1` acceptance criteria require completing the full earn loop without
leaving the screen.

**Mitigation:** The earn loop does not require all information on one screen.
It requires the user to be able to go online, see a job, see earnings, and
withdraw. Those actions span Provider Control and Wallet, which are both on
the hotbar and can be open simultaneously as tiled or floating windows. The
headless compute harness (`headless_compute.rs`) and the packaged roundtrip
script (`check-v01-packaged-autopilotctl-roundtrip.sh`) should be run against
the split layout to verify no regression.

### Risk: Large diff

The split touches rendering, state, actions, input handling, registry, and
shell chrome across at least 8 files.

**Mitigation:** The phased approach above keeps each PR focused on one
extracted pane. Phase 1 (Provider Control) is the hardest because it
establishes the pattern; subsequent phases follow the same structure. Phase 6
(remove dev mode gate) should be the last code change, applied only after all
five panes are extracted and verified.

## Recommendations

1. **Start with Phase 1** (Provider Control extraction). It establishes the
   pattern and delivers the most architectural value because it separates the
   provider orchestration surface from everything else.

2. **Do Phase 2 early** (promote wallet). This is mostly deletion of
   duplicated code and delivers immediate state-struct simplification.

3. **Phase 6 last.** Do not remove the dev-mode gate until all five panes are
   extracted. The gate provides a safety net during the transition.

4. **Do not create a "Mission Control Lite" pane** that is just a smaller
   version of the current monolith. The point is to decompose, not to shrink.

5. **Update `docs/plans/mission-control-pane.md`** after the split to reflect
   the new architecture. The plan doc currently describes the two-column
   monolith; it should describe the five-pane hotbar layout instead.

6. **Update `docs/v01.md`** to note that the one-screen fullscreen shell was a
   `v0.1`-specific release constraint, not a permanent architecture decision.

## Follow-On Issues

- Extract Provider Control pane (Phase 1)
- Promote Wallet pane to hotbar default and remove MC wallet duplication (Phase 2)
- Create Earnings and Jobs pane (Phase 3)
- Extract Buy Mode pane (Phase 4)
- Extract Log Stream pane (Phase 5)
- Remove dev mode gate and unify shell (Phase 6)
- Clean up residual Mission Control code (Phase 7)
- Update `docs/plans/mission-control-pane.md` to reflect post-split architecture
- Update `docs/v01.md` to clarify fullscreen was a release-cut constraint
- Run headless compute harness and packaged roundtrip against split layout
