# Mission Control Pane

Mission Control remains the single earn-first shell in
`apps/autopilot-desktop`. The pane kind stays `PaneKind::GoOnline`.

For the `v0.1` release cut, the production path is the fullscreen one-screen
shell described in `docs/v01.md`. Separate workbench/debug panes remain
available only for dev-mode internal use.

## Product Rule

Mission Control is local-runtime-first.

- `macOS = Apple Foundation Models via the Swift bridge`
- `supported non-macOS NVIDIA hosts = GPT-OSS via the Psionic CUDA lane`
- retained GPT-OSS Metal/CPU hosts may still surface runtime truth, but they do
  not currently unlock sell-compute
- native Metal GPT-OSS is still not the macOS MVP local-model lane

Mission Control owns provider readiness, wallet truth, job flow, and the active
local-runtime story. The local-runtime area is lane-aware, not Apple-only:

- on macOS it speaks Apple FM truth
- on supported NVIDIA/CUDA hosts it speaks GPT-OSS truth inline
- the separate Apple FM and GPT-OSS workbenches still exist for deeper
  inspection, prompt testing, and runtime-specific debugging

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

On the supported NVIDIA/CUDA GPT-OSS path, Mission Control may render:

- `REFRESH GPT-OSS` when runtime health needs to be re-read
- `WARM GPT-OSS` when the configured GGUF is present but not loaded
- `UNLOAD GPT-OSS` when the configured model is resident

On retained unsupported GPT-OSS host shapes such as Metal/CPU, the local-model
button can still fall back to `OPEN GPT-OSS WORKBENCH` so the runtime state is
truthful without pretending sell-compute is ready.

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
- on the NVIDIA GPT-OSS CUDA path: the configured GGUF model is actually loaded

Mission Control must not gate macOS provider mode on GGUF artifact presence.
On retained GPT-OSS Metal/CPU hosts, Mission Control should stay explicit that
runtime state is visible but sell-compute remains blocked until CUDA is active.

## Log Stream Copy

The log stream must reflect the active backend truth without fabricating a
different local-runtime model for Apple FM vs GPT-OSS.

Examples on macOS:

- `Apple Foundation Models ready via Swift bridge (...)`
- `Apple Foundation Models unavailable: ...`
- `Apple Foundation Models bridge reachable but not ready yet.`
- `Apple Foundation Models bridge is not running.`

Examples on the non-macOS NVIDIA path:

- `GPT-OSS ready via cuda backend (...)`
- `GPT-OSS loading configured model (...)`
- `GPT-OSS artifact missing. Configure a GGUF model before going online.`
- `GPT-OSS backend: METAL`
- `GPT-OSS model path: ...`

Mission Control may stream backend, artifact, load-state, ready-model, model
path, and error truth inline for the active GPT-OSS lane. The separate
`GPT-OSS Workbench` still owns the prompt playground and full model-management
surface.

## Pane Visibility Contract

To keep the local-model story clean:

- `Mission Control` is the release-facing Apple FM surface on macOS
- `Apple FM Workbench` remains available for dev-mode debugging and validation
- the `GPT-OSS Workbench` pane is hidden on macOS
- non-macOS builds keep that pane enabled, but it is presented as
  `GPT-OSS Workbench`
- even when the separate GPT-OSS workbench exists, the Mission Control
  local-runtime area still renders the active GPT-OSS lane inline on supported
  NVIDIA hosts

## Data Sources

Mission Control is app-owned. It renders from:

- `ProviderRuntimeState` for provider mode, active backend, and blockers
- `AppleFmBridgeSnapshot` / `ProviderAppleFmRuntimeState` for Apple FM truth
- `LocalInferenceExecutionSnapshot` / `ProviderGptOssRuntimeState` for the
  NVIDIA GPT-OSS CUDA gate
- `local_runtime_capabilities.rs` for the shared app-owned local-runtime
  capability model consumed by Mission Control, desktop control, and the
  workbench panes
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

- Mission Control speaks the active local-runtime truth instead of inventing
  separate ad hoc Apple-vs-GPT-OSS rules for each operation
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
- supported NVIDIA/CUDA hosts can refresh, warm, and unload GPT-OSS from the
  Mission Control local-runtime area without leaving the screen
- the separate `GPT-OSS Workbench` still owns prompt execution and deeper
  runtime inspection
