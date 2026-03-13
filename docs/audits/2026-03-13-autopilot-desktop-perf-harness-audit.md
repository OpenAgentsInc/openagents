# 2026-03-13 Autopilot Desktop Perf Harness Audit

## Scope

This audit covers whole-app desktop performance for the retained MVP shell in
`apps/autopilot-desktop`, with emphasis on:

- startup stalls and beachballs,
- background work that runs even while the UI looks idle,
- repeatable harness coverage through `autopilotctl` / desktop control,
- pane lifecycle performance, especially `Provider Control`.

The goal is not to speculate from logs alone. The goal is to use the running
desktop app, the supported control plane, and OS-level sampling to identify the
actual hot paths and then fix them.

## Inputs Reviewed

Product / ownership:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/headless-compute.md`

Desktop runtime / control plane:

- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/autopilot_peer_roster.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`

## Test Setup

Bridge:

```bash
cd swift/foundation-bridge
./build.sh
./bin/foundation-bridge
curl -s http://127.0.0.1:11435/health
```

Live app:

```bash
cargo autopilot
```

Control plane manifest:

- `~/.openagents/logs/autopilot/desktop-control.json`

Session log:

- `~/.openagents/logs/autopilot/latest.jsonl`

Sampling:

```bash
sample <pid> 2 1 -mayDie
```

HTTP timing:

```bash
python3 - <<'PY'
import json, pathlib, time, urllib.request
m=json.loads(pathlib.Path('~/.openagents/logs/autopilot/desktop-control.json').expanduser().read_text())
req=urllib.request.Request(m['base_url']+'/v1/snapshot', headers={'Authorization':'Bearer '+m['auth_token']})
start=time.time()
with urllib.request.urlopen(req, timeout=30) as resp:
    payload=resp.read()
print((time.time()-start)*1000, len(payload))
PY
```

## Measured Findings

### 1. Startup still performs multiple full Spark wallet syncs

Observed from a clean `cargo autopilot` run:

- first full sync completed in `3.056s`
- second full sync completed in `1.238s`
- third full sync completed in `0.981s`

This happened within the initial startup window without user interaction.

Relevant paths:

- `apps/autopilot-desktop/src/render.rs`
  - `open_startup_pane(...)` queues initial `SparkWalletCommand::Refresh`
- `apps/autopilot-desktop/src/input.rs`
  - `run_startup_spark_wallet_convergence_tick(...)` queues follow-up refreshes
- `apps/autopilot-desktop/src/spark_wallet.rs`
  - `begin_startup_convergence(...)`
  - `startup_convergence_refresh_due(...)`
  - `note_startup_convergence_refresh_queued(...)`
  - `startup_convergence_satisfied(...)`

Root cause:

- startup convergence is still defined as “keep refreshing until wallet balance
  is known and network status is `Connected`”.
- the first refresh can succeed in a user-visible sense while Spark stream
  connectivity stabilizes slightly later.
- that causes multiple expensive full refreshes, even after the wallet already
  has a usable balance and no blocking error.

Why this matters:

- each follow-up refresh runs real Spark I/O and can line up with other startup
  work on a machine that is also building scenes, starting chat lanes, and
  syncing runtime snapshots.
- the user-visible symptom matches the reported beachball pattern.

### 2. Desktop control snapshot work runs on the main thread in `about_to_wait`

OS sample against the live app showed the main thread spending time here:

- `autopilot_desktop::input::handle_about_to_wait`
- `autopilot_desktop::input::pump_background_state`
- `autopilot_desktop::desktop_control::pump_runtime`
- `autopilot_desktop::desktop_control::snapshot_for_state`

Within `snapshot_for_state(...)`, the hot sample included:

- `AutopilotChatState::active_managed_chat_messages`
- `AutopilotChatState::select_autopilot_buy_mode_target`
- `autopilot_peer_roster::build_autopilot_peer_roster`
- `autopilot_peer_roster::parse_autopilot_compute_presence_message`
- `serde_json` serialization used by `snapshot_sync_signature(...)`

Measured idle snapshot HTTP latency was about `23.9ms` for a `22 KB` payload.

Relevant paths:

- `apps/autopilot-desktop/src/desktop_control.rs`
  - `sync_runtime_snapshot(...)`
  - `snapshot_for_state(...)`
  - `snapshot_sync_signature(...)`
  - `desktop_control_nip28_status(...)`
- `apps/autopilot-desktop/src/app_state.rs`
  - `active_managed_chat_messages(...)`
  - `autopilot_peer_roster(...)`
  - `select_autopilot_buy_mode_target(...)`
- `apps/autopilot-desktop/src/autopilot_peer_roster.rs`
  - `build_autopilot_peer_roster(...)`
  - `select_autopilot_buy_mode_target_with_policy(...)`
  - `parse_autopilot_compute_presence_message(...)`

Root causes:

- `sync_runtime_snapshot(...)` builds a full snapshot before deciding whether a
  sync is actually due.
- snapshot generation duplicates work:
  - it computes buy-mode target selection,
  - then separately computes the full peer roster,
  - and both paths scan the same managed chat channel history.
- NIP-28 status builds a full vector of active channel messages and then trims
  it to the last 16 entries, instead of taking a tail directly.
- snapshot signatures serialize the full snapshot JSON on the main thread.

Why this matters:

- this work runs even when the operator is not actively using `autopilotctl`.
- it competes with UI responsiveness in the same main-thread cadence that
  handles pane interaction, hover, redraw policy, and window events.
- it helps explain “no obvious log, but the app hangs for a moment”.

### 3. The supported perf harness cannot drive pane lifecycle today

`autopilotctl` currently covers:

- provider online/offline
- local runtime / GPT-OSS / Apple FM status and actions
- wallet refresh
- buy mode
- logs / events / waits
- chat selection and send

It does not currently cover:

- pane list
- pane open
- pane focus
- pane close
- pane snapshot / pane-local status

Relevant paths:

- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`

But the app already has a typed pane lifecycle bridge:

- `apps/autopilot-desktop/src/input/tool_bridge.rs`
  - `execute_pane_list(...)`
  - `execute_pane_open(...)`
  - `execute_pane_focus(...)`
  - `execute_pane_close(...)`
  - `execute_pane_action(...)`

Root cause:

- supported desktop-control actions never surfaced the existing pane tool-bridge
  capability.

Why this matters:

- we cannot claim “whole-app perf harness” coverage if the supported control
  path cannot open, close, focus, and inspect panes.
- `Provider Control` open/close lag is exactly the kind of scenario the harness
  should be able to replay.

### 4. Pane churn exposed a second main-thread stall in NIP-90 session-log backfill

After adding pane lifecycle support and replaying `Provider Control` open/close
through the control plane, a clean macOS sample without the Frame Debugger open
showed a different stall:

- `autopilot_desktop::state::nip90_payment_facts::Nip90PaymentFactLedgerState::sync_from_background_tick`
- `autopilot_desktop::state::nip90_payment_facts::Nip90PaymentFactLedgerState::sync_from_current_truth_with_session_log_dir`
- `autopilot_desktop::state::nip90_payment_facts::Nip90PaymentFactLedgerState::refresh_log_backfill_cache`
- `serde_json::from_str(...)`

The key observation was not just “JSON parse is expensive.” It was:

- the app had a retained historical session log around `636 MB`,
- background payment-fact sync still reparsed session-log backfill on the main
  thread,
- live pane churn kept touching the current session log, which changed the
  directory signature and forced backfill refreshes at the worst possible time.

Relevant paths:

- `apps/autopilot-desktop/src/state/nip90_payment_facts.rs`
  - `sync_from_background_tick(...)`
  - `sync_from_current_truth_with_session_log_dir(...)`
  - `refresh_log_backfill_cache(...)`
  - `load_log_backfill_facts_from_session_dir(...)`

Root cause:

- session logs were treated as a fully reparsed background input even though the
  docs already position them as backfill, not the primary live read model.
- there was no “hot file” defer policy and no byte budget for backfill imports.

Why this matters:

- this creates exactly the kind of “no relevant logs, but the app beachballed”
  failure mode the user reported.
- it can be triggered by normal UI usage even though the expensive work is
  conceptually unrelated to the pane being clicked.

## Assessment

The current lag is not one bug. It is the overlap of:

- overly aggressive Spark startup convergence refreshes,
- desktop-control background snapshot work on the main thread,
- missing pane lifecycle coverage in the supported harness,
- session-log backfill reparsing on the main thread with no live import budget.

The second issue is especially important because it creates UI cost even when
the operator is not visibly doing anything. The sample shows the control plane
itself is contributing work inside the app event loop.

## Fix Plan

### Fix 1: expose pane lifecycle through desktop control and `autopilotctl`

Add app-owned desktop-control actions for:

- pane list
- pane open
- pane focus
- pane close
- pane snapshot / pane status

Then add matching `autopilotctl pane ...` subcommands so the supported harness
can replay:

- open/focus/close loops,
- pane-specific status snapshots,
- reproducible `Provider Control` stress runs.

### Fix 2: stop Spark startup from doing unnecessary follow-up full refreshes

Change startup convergence so it does not keep issuing heavy refreshes once the
wallet has a good startup snapshot.

Minimum acceptable behavior:

- first successful refresh can clear “reconciling” when balance is known and no
  blocking error remains,
- follow-up polling should be reduced or eliminated,
- startup should not perform repeated full Spark syncs just to flip a cosmetic
  network-status label.

### Fix 3: reduce desktop-control snapshot cost on the hot path

Do all of the following:

- gate background snapshot builds before calling `snapshot_for_state(...)`
- avoid computing the buy-mode roster twice in one snapshot
- avoid building a full active-message vector only to trim to a short tail
- keep snapshot serialization / signature work off the most frequent idle path

### Fix 4: rerun the harness and capture post-fix numbers

After the above changes:

- relaunch the desktop app
- verify startup Spark sync count falls
- verify pane open/close is scriptable through `autopilotctl`
- replay repeated `Provider Control` open/close
- re-sample the main thread
- confirm the previous desktop-control snapshot hotspot is materially reduced

### Fix 5: treat session-log backfill as bounded historical import, not live replay

Change NIP-90 payment-fact backfill so that live desktop operation:

- defers hot session logs during background refresh,
- skips oversized session-log files,
- caps total imported session-log bytes per background refresh,
- keeps the current product read model in the persisted payment-fact ledger
  instead of reparsing raw JSONL history on the hot path.

## Expected Outcome

After the fixes:

- startup should stop thrashing Spark syncs,
- the desktop-control runtime should stop rebuilding expensive chat/buy-mode
  projections on every idle loop,
- pane lifecycle perf will be measurable and reproducible through the supported
  harness instead of manual clicking,
- large or actively growing session logs should stop freezing the UI during
  normal pane interaction.

## Post-Fix Rerun

Measured after the control-plane, Spark, desktop-control, and session-log
backfill fixes landed:

- startup Spark syncs: one `Building SparkWallet` and one full
  `sync_wallet_internal` during initial launch
- desktop-control snapshot HTTP:
  - about `1.31 ms` average
  - `12.44 ms` max in a 12-sample burst
  - payload around `20.4 KB`
- `Provider Control` pane replay through desktop control:
  - `80` close/open cycles
  - `0` errors
  - open avg `7.99 ms`, max `10.05 ms`
  - close avg `8.82 ms`, max `46.55 ms`

Most important qualitative change from the post-fix macOS sample:

- the previous `refresh_log_backfill_cache(...)` / `serde_json::from_str(...)`
  main-thread stall no longer dominated the sample,
- the remaining visible cost during an artificial open/close stress loop was
  ordinary pane paint text shaping, mostly from `Earnings & Jobs` temporarily
  becoming the active pane when `Provider Control` closed.
