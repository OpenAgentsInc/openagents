# Autopilot Pylon And Proof Test Matrix

This matrix covers the Autopilot Tauri surface that supervises Pylon and
visualizes local Pylon/Nexus proof flows.

## Static Verification

Run from the `openagents` repo root unless noted.

```bash
cargo test -p autopilot --lib
cargo check -p autopilot
cargo check -p autopilot --bin autopilotctl-tauri
cd apps/autopilot && bun run build
```

Expected result:

- Rust projections decode the reduced contract fixtures.
- Provider mode and proof lane validation reject unsupported values.
- Redaction keeps token-like/key-like values out of projected errors.
- TypeScript compiles the Pylon and proof command surfaces.
- The programmatic Tauri control CLI compiles.

## Programmatic Tauri Control

Use the app-owned control plane before relying on manual UI inspection:

```bash
scripts/autopilot/tauri-control-smoke.sh --status-only
```

Expected result:

- The script launches `bun run tauri dev`.
- The default script supplies deterministic fake `pylon` and `oa` binaries so
  app control flow is not blocked by machine-local provider configuration.
- The running app writes a Tauri control manifest.
- `autopilotctl-tauri status` returns the same Pylon/proof projections that
  React receives through Tauri commands.
- JSON artifacts are written under `target/autopilot-tauri-control-smoke/`.

Run the full control smoke when Pylon/proof command behavior changes:

```bash
scripts/autopilot/tauri-control-smoke.sh
```

Expected result:

- Pylon status, start, mode, and stop commands are driven through the running
  Tauri app.
- The local proof replacement-attempt lane starts in the background, is polled
  through the running Tauri app, and is checked with `proof doctor`.
- The app is torn down by the script unless `--keep-running` is supplied.
- If local Pylon/proof prerequisites are missing, the failing projection shows
  explicit blocker codes or command detail rather than hanging the app when
  running with `--real-binaries`.

Use the installed local stack explicitly when that is the thing under test:

```bash
scripts/autopilot/tauri-control-smoke.sh --real-binaries
```

## Homework Handshake

Use this gate when changing the homework-only Autopilot screen or the control
projection that feeds it:

```bash
scripts/autopilot/tauri-control-smoke.sh --homework-handshake
```

Expected result:

- The actual Tauri app starts and writes a control manifest.
- `autopilotctl-tauri homework status` can read the same homework projection
  that the React screen consumes.
- `autopilotctl-tauri homework handshake` starts Pylon, waits for it to run,
  puts it online, runs the clean `cs336-a1` proof lane, waits for completion,
  runs `proof doctor`, opens artifacts, and then re-reads the final homework
  projection.
- The final homework projection reports accepted homework through closeout and
  payout state rather than recurring liveness payouts.
- The visible homework projection rejects internal wording such as sync-stale,
  Nexus-degraded, treasury-degraded, or payouts-paused status text.

The default fake Pylon implements `training status --json` so this validates
the new homework screen projection, not just the older Pylon/proof diagnostic
surface. Add `--real-binaries` only when the machine-local installed Pylon and
`oa proof` stack are the thing under test.

## Homework Proof Matrix

Use this gate for CS336/Ep224 homework-run work, `#4368` follow-ups, and proof
runtime changes that affect clean, replacement-attempt, or stale-retained-state
lanes:

```bash
scripts/autopilot/tauri-homework-matrix.sh
```

Expected result:

- The actual Tauri app starts and writes a control manifest.
- `autopilotctl-tauri homework matrix` runs through that manifest.
- The matrix covers:
  - `cs336-a1`
  - `cs336-a1-replacement-attempt`
  - `cs336-a1-stale-recovery`
- Each lane reaches `completed`.
- Clean and stale lanes expose a rewarded closeout signal.
- Each lane projects the authority-state trace, proof summary, run report, and
  object trace artifacts.
- Each lane projects authority, relay, and artifact-store transport as `ok`.
- Node-surface transport may be `ok` while nodes are active, `down` after clean
  or stale lanes quiesce workers/validators during rewarded closeout, or
  `unknown` for the zero-node replacement-attempt lane.
- `proof doctor` refreshes the enriched worker/validator projection and
  transport split after the completion snapshot.

The default script uses deterministic fake `pylon` and `oa` binaries so this is
always an Autopilot/Tauri regression gate. Add `--real-binaries` to make it a
local installed proof-runtime gate:

```bash
scripts/autopilot/tauri-homework-matrix.sh --real-binaries
```

Real-binaries mode bootstraps a disposable app-owned Pylon home, config,
identity, and loopback admin port under
`target/autopilot-tauri-control-smoke/`. It should not fail with
`CONFIG_MISSING` or `IDENTITY_MISSING`; those codes indicate a harness
regression.

## Pylon Status Projection

```bash
cargo run -p pylon --bin pylon -- status --json
```

Expected result:

- `listen_addr`, `desired_mode`, `snapshot.runtime`, `snapshot.availability`,
  and `snapshot.inventory_rows` are present.
- Autopilot can project process state separately from provider state.
- Missing local Gemma or other blockers remain explicit blocker codes, not
  generic UI failure.

## Pylon Process Control

Manual packaged-app or `bun run tauri dev` validation after the programmatic
smoke:

1. Open Autopilot.
2. Confirm the top menu bar shows `Autopilot`, `View`, `Pylon`, `Proof`, and
   `Help`.
3. Open `View > Pylon`.
4. Open `Pylon > Start Serve`.
5. Open `Pylon > Refresh`.
6. Open `Pylon > Provider Mode > Online`.
7. Open `Pylon > Provider Mode > Offline`.
8. Open `Pylon > Restart Serve`.
9. Press `Command-K`.
10. Confirm Command-K rows show breadcrumb paths such as
    `Pylon > Provider Mode > Online`.
11. Run `Stop Pylon Serve`.
12. Run `Open Pylon Logs`.

Expected result:

- Duplicate starts do not spawn a second managed child for the same app-owned
  process manager.
- The status card shows process state, provider state, binary path, config
  path, Pylon home, blocker codes, last action, last error, and exit code.
- Logs open under `~/.openagents/autopilot/logs`.
- No user-provided command text is passed through a shell.
- Long-running proof commands do not block Pylon status refresh or stop/reset
  commands.

## Local Proof Replacement Smoke

Use the replacement-attempt lane for a fast end-to-end proof visualization
smoke that does not start worker or validator processes.

```bash
cargo run -p pylon --bin oa -- proof run cs336-a1-replacement-attempt \
  --namespace proof.autopilot.ui.smoke \
  --workers 0 \
  --validators 0 \
  --timeout-seconds 60 \
  --json
```

Expected result:

- Autopilot receives an immediate `running` projection and remains usable while
  the proof lane completes in the background.
- `status` is `completed`.
- The run report detail says the replacement attempt was sealed and reconciled
  locally.
- The proof namespace writes:
  - `fleet/run-report.json`
  - `fleet/authority-state-trace.json`
  - `fleet/proof-summary.json`
  - `artifacts/object-trace.jsonl`
- Autopilot can project authority, relay, artifact-store, node-surface, worker,
  validator, simulated-treasury, and closeout stages.

Clean up after the smoke:

```bash
cargo run -p pylon --bin oa -- proof authority down \
  --namespace proof.autopilot.ui.smoke \
  --json
```

The down command may return non-zero when the post-down probes report that the
authority and artifact-store processes are no longer running. Confirm cleanup
with:

```bash
ps -p <authority_pid>,<artifact_store_pid> -o pid=,command=
```

Expected result:

- No process remains for those PIDs.

## Local Proof Doctor

```bash
cargo run -p pylon --bin oa -- proof doctor \
  --namespace proof.autopilot.ui.smoke \
  --json
```

Expected result:

- The doctor output separates authority front door, relay, artifact store,
  node admin/checkpoint surfaces, process provenance, and workspace provenance.
- The Autopilot proof card can refresh the transport split without reading raw
  logs.

## UI Inspection

Run:

```bash
cd apps/autopilot
bun run tauri dev
```

Inspect:

- default view is the sparse command console message plus one exact-action
  input box
- menu items, Command-K rows, and pane-local buttons use the same action labels
- `Command-K` opens the command dialog in the active theme
- no static runtime/evidence placeholder cards are shown
- Pylon and Proof cards remain inside the fixed viewport
- card content scrolls internally when dense rows exceed the viewport
- local proof surfaces are labeled as local simulation/simulated treasury

## Release Packaging

For packaged validation, provide approved `pylon` and `oa` binaries through one
of the resolver paths documented in:

- `apps/autopilot/src-tauri/binaries/README.md`
- `docs/pylon/autopilot-proof-contract.md`

Expected result:

- The packaged app resolves a bundled or app-managed binary before falling back
  to `PATH`.
- Source-checkout-only workspace paths are not required for the packaged app.
