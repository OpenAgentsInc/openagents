# Mission Control Pane

Mission Control remains the single earn-first shell in
`apps/autopilot-desktop`. The pane kind stays `PaneKind::GoOnline`; the
product rule changed.

## Product Rule

Mission Control is now backend-aware:

- `macOS = Apple Foundation Models via the Swift bridge`
- `non-macOS NVIDIA path = Psionic GPT-OSS CUDA`
- native Metal GPT-OSS is not the macOS MVP local-model lane

That means Mission Control must not present `LOAD GPT-OSS 20B` as the universal
gate anymore. On macOS the pane should speak Apple FM truth throughout its
sell-compute state, CTA, blockers, and log stream.

## Layout

Mission Control is still a two-column dashboard.

### Left Column

`// SELL COMPUTE`

- primary CTA: `GO ONLINE`
- status rows: `Mode`, `Model`, `Backend`, `Load`, `Control`, `Preflight`
- contextual hint below the state rows when the active local lane is blocked

`// EARNINGS`

- `Today`
- `This Month`
- `All Time`
- BIP 177 integer display by default, with the existing legacy BTC toggle

`// WALLET`

- `Status`
- `Address`
- `Balance`

Bottom actions:

- backend-aware local-model action
- `DOCUMENTATION`

### Right Column

`// ACTIVE JOBS`

- one-line summary of the active job, or `Go Online to Start Jobs.`

`// LOG STREAM`

- scrolling terminal-style status stream
- shows provider mode, preflight blockers, local-model readiness, UI actions,
  provider results, and job lifecycle updates

## Backend-Aware CTA Contract

The local-model action is sourced from the same runtime truth that gates
provider mode.

On macOS:

- `START APPLE FM` when the bridge is offline
- `REFRESH APPLE FM` when the bridge is reachable but not ready
- `STARTING APPLE FM` while bridge start is already in flight
- `OPEN APPLE FM` when the bridge and system model are ready

On the non-macOS GPT-OSS CUDA path:

- `LOAD GPT-OSS CUDA` when the configured model is present but not resident
- `LOADING GPT-OSS CUDA` while warm/load is in progress
- `OPEN GPT-OSS CUDA` when the model is ready

If neither supported lane is available, the action is disabled and Mission
Control says so plainly.

## GO ONLINE Gate

`GO ONLINE` must only unlock when the active local backend is ready:

- on macOS: Apple FM bridge reachable, model available, and ready
- on the GPT-OSS CUDA path: the configured model is actually resident

Mission Control must not gate macOS provider mode on GGUF artifact presence.

## Log Stream Copy

The log stream must reflect the active backend truth.

Examples on macOS:

- `Apple Foundation Models ready via Swift bridge (...)`
- `Apple Foundation Models unavailable: ...`
- `Apple Foundation Models bridge reachable but not ready yet.`
- `Apple Foundation Models bridge is not running.`

Examples on the GPT-OSS CUDA path:

- `Local GPT-OSS CUDA is loading.`
- `Local GPT-OSS ready on Psionic cuda.`
- `Local GPT-OSS artifact missing at ...`
- `Local GPT-OSS CUDA is present but not loaded.`

## Pane Visibility Contract

To avoid a split-brain local-model story on macOS:

- `Mission Control` and `Apple FM Workbench` are the user-facing local-model
  surfaces on macOS
- the GPT-OSS `Local Inference` pane is not exposed in the macOS pane registry
  or command palette
- non-macOS builds keep the GPT-OSS `Local Inference` pane enabled

## Data Sources

Mission Control is app-owned. It renders from:

- `ProviderRuntimeState` for provider mode, active backend, and blockers
- `AppleFmBridgeSnapshot` / `ProviderAppleFmRuntimeState` for Apple FM truth
- `LocalInferenceExecutionSnapshot` / `ProviderOllamaRuntimeState` for the
  GPT-OSS CUDA path
- `SparkPaneState` for wallet truth
- `EarnJobLifecycleProjectionState`, `JobInboxState`, and `ActiveJobState` for
  job visibility

There should not be a second inferred status path for Mission Control copy.

## Implementation Grounding

The current implementation lives in:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`

Related Apple FM runtime and workbench code lives in:

- `apps/autopilot-desktop/src/apple_fm_bridge.rs`
- `apps/autopilot-desktop/src/panes/apple_fm_workbench.rs`
- `crates/psionic/psionic-apple-fm`

## Definition Of Done

Mission Control is correct when all of the following are true:

- macOS Mission Control speaks Apple FM truth instead of GPT-OSS-specific copy
- the local-model action button and `GO ONLINE` gate are backend-aware
- provider blockers and log lines come from the same backend/runtime state that
  provider mode uses
- macOS does not expose a competing GPT-OSS local-model pane in the command
  palette
- non-macOS GPT-OSS CUDA flows remain available where that is still the
  truthful runtime path
