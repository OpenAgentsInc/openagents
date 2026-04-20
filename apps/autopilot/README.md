# Autopilot Tauri Shell

This is the Tauri shell for the next Autopilot desktop product surface.

The app uses:

- Tauri 2 for the desktop host and Rust IPC.
- React and TypeScript for the product UI.
- Bun for package management and local frontend commands.
- shadcn/ui and cmdk for the local command dialog.
- Rust-owned Pylon and proof-runtime commands projected into dense operator
  cards.

Run from this directory:

```bash
bun install
bun run tauri dev
```

The first screen is a centered command entry. Press `Command-K` to open the
command dialog. The command dialog can:

- show the Pylon status card
- start, stop, restart, and refresh `pylon serve`
- set the provider mode to `online`, `offline`, `pause`, or `resume`
- open app-owned Pylon logs
- show the local proof-flow card
- run the `cs336-a1`, stale-recovery, and replacement-attempt proof lanes
- run `oa proof doctor`
- stop, reset, refresh, or open artifacts for the active proof namespace
- toggle light/dark mode

Keep privileged state and authority in Rust-backed commands, events, channels,
or lower-level OpenAgents services. Keep the TypeScript UI as product projection
and interaction.

## Programmatic Control

The running Tauri app exposes a loopback-only control plane for agents and
operator scripts. It writes a manifest at:

```text
~/.openagents/autopilot/tauri-control.json
```

From the repo root, inspect the live Tauri app with:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- --json status
```

Drive the Pylon and proof controls through the same Rust command path used by
the React UI:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon status
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon start
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon mode offline
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon stop
cargo run -p autopilot --bin autopilotctl-tauri -- --json smoke \
  --namespace proof.autopilot.ctl.smoke \
  --timeout-ms 180000
```

For a one-command launch-and-control smoke from the repo root:

```bash
scripts/autopilot/tauri-control-smoke.sh
```

That smoke launches the real Tauri app and uses deterministic fake `pylon` and
`oa` binaries by default so the control flow is not blocked by local provider
configuration. Use `--real-binaries` to drive the installed local stack.

For homework-run changes, use the full matrix:

```bash
scripts/autopilot/tauri-homework-matrix.sh
```

That launches the Tauri app and drives `autopilotctl-tauri homework matrix`
through the app-owned control plane. It runs and validates:

- `cs336-a1`
- `cs336-a1-replacement-attempt`
- `cs336-a1-stale-recovery`

The matrix checks status, authority trace, proof summary, object trace,
transport split, worker/validator projections, and closeout signals. Add
`--real-binaries` when the installed local `oa proof` stack is the thing under
test. In real-binaries mode the wrapper creates a disposable app-owned Pylon
home, config, identity, and loopback admin port under
`target/autopilot-tauri-control-smoke/`, so the test does not depend on the
operator's personal `~/.openagents/pylon` state.

The full runbook lives in
`../../docs/codex/AUTOPILOT_TAURI_CONTROL.md`.

## Pylon Process Authority

The Rust side owns Pylon process control in `src-tauri/src/pylon.rs`.
TypeScript calls Tauri commands and renders returned projections. It does not
own provider, wallet, payout, proof, or Nexus state.

Binary resolution is deliberately explicit:

1. `OPENAGENTS_PYLON_BINARY` / `OPENAGENTS_OA_BINARY`
2. bundled sidecar/resource candidates next to the app executable
3. workspace builds under `target/{debug,fast-release,release}`
4. app-managed cache under `~/.openagents/autopilot/bin`
5. existing Pylon bootstrap cache under `~/.openagents/pylon/bootstrap`
6. `PATH`

Autopilot uses an app-owned Pylon home by default:

```text
~/.openagents/autopilot/pylon
```

Override with `OPENAGENTS_AUTOPILOT_PYLON_HOME` or
`OPENAGENTS_AUTOPILOT_PYLON_CONFIG_PATH` when a test needs a specific isolated
home/config. Raw command execution uses `std::process::Command`; it does not
shell through user input.

Pylon serve logs are written under:

```text
~/.openagents/autopilot/logs
```

## Proof Runtime Projection

Autopilot drives local proof lanes through the `oa` binary:

```bash
oa proof run cs336-a1 --namespace <namespace> --workers 1 --validators 1 --json
oa proof run cs336-a1-stale-recovery --namespace <namespace> --workers 1 --validators 1 --json
oa proof run cs336-a1-replacement-attempt --namespace <namespace> --workers 0 --validators 0 --json
oa proof doctor --namespace <namespace> --json
```

Proof runs start in the background and return an immediate `running`
projection. Completion or failure is delivered back to React through Tauri
events, so the command dialog remains usable for stop/reset while the proof
lane is active.

Set `OPENAGENTS_AUTOPILOT_PROOF_ROOT` to force proof artifact reads under an
isolated test root. The namespace is appended to that root.

The card reads the same machine artifacts documented in
`../../docs/pylon/autopilot-proof-contract.md`:

- `run-report.json`
- `authority-state-trace.json`
- `proof-summary.json`
- `object-trace.jsonl`

The UI labels local proof lanes as local simulation and simulated treasury
unless a future authority projection proves otherwise.

The current verification matrix lives in
`../../docs/pylon/autopilot-test-matrix.md`.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
