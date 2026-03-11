# Mission Control Pane

Mission Control remains the single earn-first shell in
`apps/autopilot-desktop`. The pane kind stays `PaneKind::GoOnline`.

## Product Rule

Mission Control is FM-first.

- `macOS = Apple Foundation Models via the Swift bridge`
- `non-macOS NVIDIA path = Psionic GPT-OSS CUDA`
- native Metal GPT-OSS is not the macOS MVP local-model lane

The key separation is now explicit:

- Mission Control owns provider readiness, wallet truth, job flow, and the
  Apple FM local-model story on macOS
- GPT-OSS-specific loading, prompt testing, and runtime troubleshooting live in
  the separate `GPT-OSS Workbench` pane, not in the main Mission Control shell

Mission Control may still gate `GO ONLINE` on the active runtime being ready,
but it should not behave like the primary GPT-OSS control panel anymore.

## Layout

Mission Control remains a two-column dashboard.

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

- local-model action
- `DOCUMENTATION`

### Right Column

`// ACTIVE JOBS`

- one-line summary of the active job, or `Go Online to Start Jobs.`

`// LOG STREAM`

- scrolling terminal-style status stream
- shows provider mode, preflight blockers, local-model readiness, UI actions,
  provider results, and job lifecycle updates

## Local-Model Action Contract

The local-model action is sourced from the same runtime truth that gates
provider mode.

On macOS:

- `START APPLE FM` when the bridge is offline
- `REFRESH APPLE FM` when the bridge is reachable but not ready
- `STARTING APPLE FM` while bridge start is already in flight
- `OPEN APPLE FM` when the bridge and system model are ready

On the non-macOS NVIDIA CUDA path:

- `OPEN GPT-OSS WORKBENCH`

That button only opens the separate GPT-OSS pane. It does not warm, load,
unload, or debug GPT-OSS directly from Mission Control.

If neither supported lane is available, the action is disabled and Mission
Control says so plainly.

## GO ONLINE Gate

`GO ONLINE` only unlocks when the active local backend is ready:

- on macOS: Apple FM bridge reachable, model available, and ready
- on the NVIDIA GPT-OSS CUDA path: the configured model is actually resident

Mission Control must not gate macOS provider mode on GGUF artifact presence.
On NVIDIA hosts, Mission Control may reflect whether the local model is ready,
but the actual GPT-OSS load/troubleshooting workflow belongs in the separate
workbench.

## Log Stream Copy

The log stream must reflect backend truth without turning Mission Control into a
second GPT-OSS debugging pane.

Examples on macOS:

- `Apple Foundation Models ready via Swift bridge (...)`
- `Apple Foundation Models unavailable: ...`
- `Apple Foundation Models bridge reachable but not ready yet.`
- `Apple Foundation Models bridge is not running.`

Examples on the non-macOS NVIDIA path:

- `NVIDIA local model ready. Manage GPT-OSS in the separate workbench pane.`
- `Open the separate GPT-OSS workbench to load and validate the NVIDIA local model.`

Do not stream GPT-OSS artifact-path, warm/load-progress, or prompt-playground
details in Mission Control. Those belong in `GPT-OSS Workbench`.

## Pane Visibility Contract

To keep the local-model story clean:

- `Mission Control` and `Apple FM Workbench` are the user-facing Apple FM
  surfaces on macOS
- the `GPT-OSS Workbench` pane is hidden on macOS
- non-macOS builds keep that pane enabled, but it is presented as
  `GPT-OSS Workbench`

## Data Sources

Mission Control is app-owned. It renders from:

- `ProviderRuntimeState` for provider mode, active backend, and blockers
- `AppleFmBridgeSnapshot` / `ProviderAppleFmRuntimeState` for Apple FM truth
- `LocalInferenceExecutionSnapshot` / `ProviderOllamaRuntimeState` for the
  NVIDIA GPT-OSS CUDA gate
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

Related local-model runtime panes live in:

- `apps/autopilot-desktop/src/apple_fm_bridge.rs`
- `apps/autopilot-desktop/src/panes/apple_fm_workbench.rs`
- `apps/autopilot-desktop/src/panes/local_inference.rs`
- `crates/psionic/psionic-apple-fm`

## Definition Of Done

Mission Control is correct when all of the following are true:

- macOS Mission Control speaks Apple FM truth instead of GPT-OSS-specific copy
- the local-model action and `GO ONLINE` gate are honest about backend
  readiness
- provider blockers and log lines come from the same backend/runtime state that
  provider mode uses
- macOS does not expose a competing GPT-OSS local-model pane in the command
  palette
- GPT-OSS-specific loading and troubleshooting are handled in the separate
  `GPT-OSS Workbench` pane rather than in Mission Control
