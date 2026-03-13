# Mission Control Split Plan

Mission Control no longer exists as the active earn-first pane in
`apps/autopilot-desktop`.

The old two-column fullscreen `PaneKind::GoOnline` shell was the right
`v0.1` release cut, but the landed desktop architecture is now the split
hotbar shell. The earn loop still stays front-and-center; it is just decomposed
into focused panes instead of one monolith.

`PaneKind::GoOnline` remains only as a hidden compatibility enum path for stale
saved state and legacy tool aliases. It is not a supported pane surface, it has
no pane spec, and it must not grow new UI again.

## Product Rule

- Keep `docs/MVP.md` as the product authority.
- Keep `docs/OWNERSHIP.md` as the authority for app-vs-crate boundaries.
- Do not create "Mission Control Lite". New earn-shell work must either land in
  an existing focused pane or justify a new pane with clear ownership.
- Preserve the seller-first MVP loop: provider readiness, wallet truth,
  earnings/jobs visibility, buyer smoke testing, and replay-safe logs must stay
  one keystroke or one click away.

## Default Shell Contract

The production desktop shell is now the hotbar shell. There is no longer a
`Production vs Dev` shell split for the pane workspace.

Startup behavior:

- Open `Provider Control`.
- Open `Earnings & Jobs`.
- Lay them out as the default earn-first pair.
- Bring `Provider Control` to front.

Hotbar contract:

- `1` -> `Provider Control`
- `2` -> `Nostr Keys (NIP-06)`
- `3` -> `Spark Lightning Wallet`
- `4` -> `Earnings & Jobs`
- `5` -> `Log Stream`
- `K` -> command palette

Non-startup but first-class command-palette panes:

- `Buy Mode`
- `Job Inbox`
- `Active Job`
- `Job History`
- `Relay Connections`
- `Sync Health`
- `Provider Status`
- `Apple FM Workbench`
- `GPT-OSS Workbench`

## Pane Ownership

### Provider Control

Owns:

- `GO ONLINE` / `GO OFFLINE`
- provider mode truth (`offline`, `connecting`, `online`, `degraded`)
- local-runtime truth (`Model`, `Backend`, `Load`, `Control`, `Preflight`)
- blocker and last-action messaging
- local runtime action button
- Apple FM smoke-test button when that lane is active
- provider inventory toggles

Primary data sources:

- `ProviderRuntimeState`
- `LocalInferenceExecutionSnapshot`
- Apple FM bridge/runtime snapshots
- app-owned local runtime capability helpers in `app_state.rs`

Implementation grounding:

- `apps/autopilot-desktop/src/panes/provider_control.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/input/actions.rs`

### Wallet

Owns:

- wallet connectivity, address, and balances
- invoice creation
- Lightning payment / withdrawal flow
- seed and custody-adjacent controls

Canonical panes:

- `Spark Lightning Wallet`
- `Create Lightning Invoice`
- `Pay Lightning Invoice`

Primary data source:

- `SparkPaneState`

Mission Control rule:

- provider/runtime surfaces may reference wallet readiness, but wallet controls
  must live in wallet-owned panes rather than reappearing inside a monolith.

### Earnings & Jobs

Owns:

- sats today / lifetime and job counts
- current uptime and recent result summary
- preview of inbox / active-job / recent-history state
- jump-off actions into `Job Inbox`, `Active Job`, and `Job History`

Primary data sources:

- `EarningsScoreboardState`
- `EarnJobLifecycleProjectionState`
- `JobInboxState`
- `ActiveJobState`
- `JobHistoryState`
- wallet totals for payout truth

Implementation grounding:

- `apps/autopilot-desktop/src/panes/earnings_jobs.rs`

### Buy Mode

Owns the constrained buyer smoke-test lane:

- start / stop buyer loop
- fixed `kind: 5050`
- fixed `2 sats`
- one in-flight request at a time
- app-owned target selection and payment-history ledger

Primary data sources:

- `BuyModePaymentsPaneState`
- `NetworkRequestsState`
- `SparkPaneState`

Mission Control rule:

- buy mode is now its own pane. Do not regress to an inline buyer block inside a
  replacement Mission Control dashboard.

### Log Stream

Owns:

- replay-safe runtime log view
- copy-all action
- mirrored provider/buyer/wallet/session notices

Primary data sources:

- `LogStreamPaneState`
- app-owned mirrored action/error notices from the earn shell

Implementation grounding:

- `apps/autopilot-desktop/src/panes/log_stream.rs`
- `apps/autopilot-desktop/src/runtime_log.rs`

### Nostr Keys

Owns:

- NIP-06 identity material
- reveal/copy/regenerate flows
- the hotbar-visible network identity surface that used to be implicit inside
  Mission Control onboarding/readiness copy

## Phase Ordering

The split shipped in this order:

1. `#3451` Extract `Provider Control`.
2. `#3452` Promote the existing wallet panes and remove wallet duplication.
3. `#3453` Promote `Earnings & Jobs` as the summary pane for earnings and job flow.
4. `#3454` Extract `Buy Mode`.
5. `#3455` Extract `Log Stream`.
6. `#3456` Remove the shell gate and make the hotbar shell the default shell.
7. `#3457` Delete residual Mission Control monolith plumbing.
8. `#3458` / `#3459` / `#3460` update docs and verification around the landed shell.

## Compatibility Rules

- `pane.provider_control` is the canonical provider pane command.
- Legacy tool/runtime aliases such as `mission_control` and `go_online` resolve
  to `Provider Control` for compatibility only.
- There is no `pane.mission_control` registry entry.
- There is no dedicated Mission Control renderer.
- There is no dev-mode-only workspace shell.

## Data Truth

The split changes shell composition, not the truth model.

- Provider truth remains app-owned in `apps/autopilot-desktop`.
- Wallet truth remains explicit and authoritative in `SparkPaneState`.
- Job lifecycle truth remains replay-safe and receipt-backed.
- Buy-mode state remains app-owned and intentionally constrained.
- Log mirroring remains replay-safe and suitable for desktop-control snapshots.

The remaining `MissionControlPaneState` is a thin notice adapter used for
mirrored action/error messages and desktop-control/log-stream continuity. It is
not a pane contract and must not grow monolithic UI state again.

## Implementation Grounding

Current split-shell implementation is grounded in:

- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/desktop_shell.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/panes/provider_control.rs`
- `apps/autopilot-desktop/src/panes/earnings_jobs.rs`
- `apps/autopilot-desktop/src/panes/buy_mode.rs`
- `apps/autopilot-desktop/src/panes/log_stream.rs`

## Definition Of Done

The split remains correct only if all of the following stay true:

- the default shell is the hotbar shell, not a hidden fullscreen singleton
- provider, wallet, earnings/jobs, buy mode, and logs stay independently
  scrollable and independently testable
- no new work reintroduces combined Mission Control-only state, actions, or
  rendering paths
- legacy `GoOnline` compatibility stays thin and invisible to normal users
