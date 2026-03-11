# Mission Control Pane

Mission Control remains the single earn-first shell in
`apps/autopilot-desktop`. The pane kind stays `PaneKind::GoOnline`.

For the `v0.1` release cut, the production path is the fullscreen one-screen
shell described in `docs/v01.md`. Separate workbench/debug panes remain
available only for dev-mode internal use.

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
- inline Lightning withdraw input + action

### Right Column

`// ACTIVE JOBS`

- one-line summary of the active job, or `Go Online to Start Jobs.`

`// BUY MODE` (feature-gated)

- hidden unless `OPENAGENTS_ENABLE_BUY_MODE=1`
- one-line summary of the latest outbound smoke-test request
- status rows: `State`, `Kind`, `Budget`, `Result`, `Settlement`
- action: `BUY 5050 TEST JOB`

`// LOG STREAM`

- scrolling terminal-style status stream
- shows provider mode, preflight blockers, local-model readiness, UI actions,
  provider results, buy-mode lifecycle updates, and job lifecycle updates

## Buy Mode Contract

Mission Control may expose a small inline `Buy Mode` block for internal and
staging verification, but it must stay constrained enough that it does not
become a second product surface.

Contract:

- gated behind `OPENAGENTS_ENABLE_BUY_MODE=1`
- one outbound smoke-test at a time
- publishes exactly one `kind: 5050` request per click
- fixed spend: `2 sats`
- uses a fixed tiny prompt template chosen for cheap validation of the result
  path
- buyer lifecycle renders inline from the same app-owned state used by the
  existing `Network Requests` pane
- terminal success requires both a provider result and a terminal buyer-wallet
  payment success state
- no general prompt composer, quote board, RFQ editor, or autonomous buy loop
  in Mission Control for `v0.1`

This is a smoke-test lane for the real NIP-90 + Lightning path. It does not
replace hosted starter demand and it should not dilute the earn-first release
copy.

## Local-Model Action Contract

The local-model action is sourced from the same runtime truth that gates
provider mode.

On the `v0.1` production path:

- `START APPLE FM` when the bridge is offline
- `REFRESH APPLE FM` when the bridge is reachable
- `STARTING APPLE FM` while bridge start is already in flight

This action stays inline in Mission Control for the release cut. It should not
require opening a second pane to start or refresh Apple FM.

In dev mode on macOS:

- `OPEN APPLE FM` may still open the separate Apple FM workbench when the
  bridge and system model are ready

In dev mode on the non-macOS NVIDIA CUDA path:

- `OPEN GPT-OSS WORKBENCH`

That button only opens the separate GPT-OSS pane. It does not warm, load,
unload, or debug GPT-OSS directly from Mission Control.

If neither supported lane is available, the action is disabled and Mission
Control says so plainly.

## Provider mode and relay failures

When you click **GO ONLINE**, the app:

1. Starts the NIP-90 provider ingress lane (relay connections).
2. **Provider mode** is derived from that lane and local inference in
   `openagents-provider-substrate::derive_provider_lifecycle`.

**Current behaviour:**

- If **any** configured relay fails to connect (e.g. `Network is unreachable`),
  the lane sets `last_error` and mode **Degraded**. The same error is then
  passed into the lifecycle as `relay_error`, so **provider mode becomes
  Degraded** and the UI shows DEGRADED and the error under `provider.runtime`.
- So “first time I connected it went to degraded state cuz it got an error from
  a relay” is **by design**: one relay failure is enough to mark provider
  ingress as degraded.
- Logs can look noisy because:
  - `relay.connections` gets the relay error (e.g. `Failed connecting relay
    wss://relay.nostr.band: ...`).
  - `provider.runtime` then gets the same error (and sometimes an additional
    “kernel authority unavailable” when hosted control is not configured).
  - Both are cleared when the lane later succeeds (e.g. relay connects or
    lane is retried).

**Implemented (relaxed policy):** Degraded only when **no** relays are
connected (`connected_relays == 0`). If at least one relay is up, provider
mode is Online (or Preview) and `last_error` is cleared so the lifecycle
does not see a relay error. Per-relay failures remain visible in the relay
pane health rows.

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

- `Mission Control` is the release-facing Apple FM surface on macOS
- `Apple FM Workbench` remains available for dev-mode debugging and validation
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
- `NetworkRequestsState` / `NetworkRequestsPaneInputs` for inline buy-mode
  publish/result/payment lifecycle when enabled
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
- `apps/autopilot-desktop/src/state/operations.rs`

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
- enabling `OPENAGENTS_ENABLE_BUY_MODE=1` adds one inline `kind: 5050` /
  `2 sats` buyer smoke-test flow without opening another pane
- buy-mode success/failure state comes from `NetworkRequestsState` and Spark
  wallet truth rather than Mission Control-local inference
- macOS does not expose a competing GPT-OSS local-model pane in the command
  palette
- GPT-OSS-specific loading and troubleshooting are handled in the separate
  `GPT-OSS Workbench` pane rather than in Mission Control
