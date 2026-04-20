# Autopilot Tauri Programmatic Control

This is the programmatic test surface for the Tauri Autopilot shell in
`apps/autopilot`.

Use it when changing the Tauri Pylon card, proof card, command-dialog command
set, or Rust command handlers. Manual clicking is useful for final visual
inspection, but it is not sufficient proof that the app flow works.

## What Exists

The Tauri app starts a local app-owned HTTP control plane from the same Rust
process that backs the UI:

- control plane: `apps/autopilot/src-tauri/src/control.rs`
- Pylon/proof command authority: `apps/autopilot/src-tauri/src/pylon.rs`
- thin CLI: `apps/autopilot/src-tauri/src/bin/autopilotctl-tauri.rs`
- smoke launcher: `scripts/autopilot/tauri-control-smoke.sh`

The control plane calls the same Rust functions that Tauri IPC exposes to
React. It does not create a second proof runtime, a second Pylon supervisor, or
a headless-only state machine. The CLI is intentionally thin: it reads the
manifest, sends a local HTTP request, and prints the JSON projection returned
by the running app.

## Control Manifest

On startup, the app writes:

```text
~/.openagents/autopilot/tauri-control.json
```

Override the path for tests:

```bash
OPENAGENTS_AUTOPILOT_CONTROL_MANIFEST=/tmp/autopilot-tauri-control.json \
  bun run tauri dev
```

The manifest contains:

- `schemaVersion`
- `product`
- `control`
- `baseUrl`
- `authToken`
- `pid`
- `startedAt`

The bearer token is local test authority. Do not paste it into tracked files,
issue comments, logs intended for sharing, or normal chat output.

The server binds to `127.0.0.1:0` by default. Keep it loopback-only unless a
task explicitly requires another bind address.

Useful environment variables:

```text
OPENAGENTS_AUTOPILOT_CONTROL_MANIFEST=/path/to/manifest.json
OPENAGENTS_AUTOPILOT_CONTROL_BIND=127.0.0.1:0
OPENAGENTS_AUTOPILOT_CONTROL_AUTH_TOKEN=<local-token>
OPENAGENTS_AUTOPILOT_CONTROL_DISABLED=1
```

## Run The App

From `apps/autopilot`:

```bash
bun run tauri dev
```

In another shell from the repo root:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- --json status
```

Use a custom manifest when the app was launched with one:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- \
  --manifest /tmp/autopilot-tauri-control.json \
  --json \
  status
```

## CLI Commands

Status:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- --json status
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon status
cargo run -p autopilot --bin autopilotctl-tauri -- --json proof status
```

Pylon process and provider mode:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon start
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon mode offline
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon mode online
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon restart
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon stop
```

Proof runtime:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- --json proof run cs336-a1 \
  --namespace proof.autopilot.ctl.cs336 \
  --workers 1 \
  --validators 1 \
  --timeout-seconds 180

cargo run -p autopilot --bin autopilotctl-tauri -- --json wait proof-completed \
  --namespace proof.autopilot.ctl.cs336 \
  --timeout-ms 240000

cargo run -p autopilot --bin autopilotctl-tauri -- --json proof doctor \
  proof.autopilot.ctl.cs336

cargo run -p autopilot --bin autopilotctl-tauri -- --json proof stop \
  proof.autopilot.ctl.cs336
```

Fast replacement-attempt smoke:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- --json smoke \
  --namespace proof.autopilot.ctl.smoke \
  --timeout-ms 180000
```

The smoke command covers:

- control status
- Pylon status
- Pylon start
- provider mode change to `offline`
- Pylon stop
- proof replacement-attempt run
- proof completion polling
- proof doctor
- proof stop

## Homework Proof Matrix

Issue `#4385` closed only after the local proof runtime gained the proof lanes
needed to shorten `#4368`-class debugging: clean CS336, replacement-attempt,
stale retained-state recovery, authority-state trace, transport split view,
proof doctor, and closure-oriented proof summary.

Issue `#4368` remains live-proof-gated. Its latest comments say local proof is
green for scheduler/artifact/validator/closeout behavior, while remaining
closure depends on live-only Nexus deployment, public stats latency, and
treasury/funding continuity. The Tauri test surface should therefore prove the
local proof matrix from the running app before anyone uses production as a
confirmation surface.

Run the full homework matrix:

```bash
scripts/autopilot/tauri-homework-matrix.sh
```

Equivalent direct CLI command against an already-running Tauri app:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- --json homework matrix \
  --namespace-prefix proof.autopilot.homework.manual \
  --timeout-ms 240000
```

The matrix drives these lanes through the running Tauri app:

| Lane | Workers | Validators | Why It Exists |
| --- | ---: | ---: | --- |
| `cs336-a1` | 1 | 1 | Clean homework run with accepted contribution and rewarded closeout. |
| `cs336-a1-replacement-attempt` | 0 | 0 | Reproduces the 4368 replacement assignment/window-seal class without starting worker/validator nodes. |
| `cs336-a1-stale-recovery` | 1 | 1 | Reproduces stale retained worker/validator state and verifies fresh claim/reconcile recovery. |

For each lane, `autopilotctl-tauri homework matrix` verifies:

- `status == completed`
- projected lane matches the requested lane
- worker and validator projections meet the lane expectation
- `run-report.json` exists
- `authority-state-trace.json` exists
- `proof-summary.json` exists
- `object-trace.jsonl` exists
- authority, relay, artifact-store, and node-surface transport projections are
  `ok`
- `proof doctor` returns an `ok` transport split
- clean and stale lanes reach a rewarded closeout signal

The default wrapper uses deterministic fake `pylon` and `oa` binaries to prove
the Autopilot/Tauri control path and projection validation. Use real local
binaries when the installed proof runtime itself is under test:

```bash
scripts/autopilot/tauri-homework-matrix.sh --real-binaries
```

If `--real-binaries` fails because the local provider or proof prerequisites
are missing, keep the failure output as the honest blocker and still run the
default matrix so the app-owned control path is verified.

## One-Command Tauri Smoke

Use this when an agent needs to prove the actual Tauri app can be launched and
controlled programmatically:

```bash
scripts/autopilot/tauri-control-smoke.sh
```

The script starts `bun run tauri dev`, waits for the control manifest, runs
`autopilotctl-tauri status`, runs `autopilotctl-tauri smoke`, stores JSON under
`target/autopilot-tauri-control-smoke/`, and then stops the Tauri app.

By default it supplies deterministic fake `pylon` and `oa` binaries through
`OPENAGENTS_PYLON_BINARY` and `OPENAGENTS_OA_BINARY`. That default is meant to
prove the Tauri app, control manifest, auth, Pylon command routes, proof command
routes, background proof polling, and artifact projection without requiring the
operator's local provider identity or model config to be ready.

Options:

```bash
scripts/autopilot/tauri-control-smoke.sh --status-only
scripts/autopilot/tauri-control-smoke.sh --keep-running
scripts/autopilot/tauri-control-smoke.sh --namespace proof.autopilot.ctl.manual
scripts/autopilot/tauri-control-smoke.sh --timeout-ms 240000
scripts/autopilot/tauri-control-smoke.sh --homework-matrix
scripts/autopilot/tauri-control-smoke.sh --real-binaries
```

Artifacts:

```text
target/autopilot-tauri-control-smoke/tauri-control.json
target/autopilot-tauri-control-smoke/status.json
target/autopilot-tauri-control-smoke/smoke.json
target/autopilot-tauri-control-smoke/tauri-dev.log
target/autopilot-tauri-control-smoke/proof/namespaces/<namespace>/
```

## Expected JSON

`status` returns a snapshot shaped like:

```json
{
  "schemaVersion": 1,
  "product": "Autopilot",
  "control": "tauri",
  "pid": 12345,
  "pylonBinary": {},
  "pylonStatus": {},
  "activeProof": null
}
```

`proof run` returns immediately with `status: "running"` because proof lanes run
in the background. Use `wait proof-completed --namespace <namespace>` to poll
the same app until the projection leaves `idle`, `starting`, or `running`.

## Agent Rule

When changing `apps/autopilot`, run at least:

```bash
cargo check -p autopilot
cargo test -p autopilot --lib
cd apps/autopilot && bun run build
scripts/autopilot/tauri-control-smoke.sh --status-only
```

When changing Pylon buttons, proof buttons, proof projections, proof event
handling, or command-dialog commands that trigger runtime work, run the full:

```bash
scripts/autopilot/tauri-control-smoke.sh
```

When changing homework-run behavior, `#4368` proof behavior, or the retained
state/replacement lanes from `#4385`, run:

```bash
scripts/autopilot/tauri-homework-matrix.sh
```

If the full smoke cannot run because local Pylon/proof prerequisites are
missing in `--real-binaries` mode, document the exact blocker and run the
default deterministic smoke so the Tauri manifest, auth, control server, CLI
path, Pylon routes, proof routes, and projection path are still verified.

## Extending The Contract

When adding a new Tauri command that affects runtime state:

1. Keep the authority in Rust.
2. Add or reuse a serializable projection type.
3. Expose the command through `tauri::generate_handler!`.
4. Add a matching route in `control.rs`.
5. Add a thin CLI command in `autopilotctl-tauri.rs`.
6. Add the command to the smoke script when it is part of the normal operator
   path.
7. Update this document and `docs/pylon/autopilot-test-matrix.md`.

Do not add a second hidden runtime just to make tests easy. Programmatic tests
must exercise the running Tauri app.

## Troubleshooting

If the CLI cannot connect, inspect the manifest path first:

```bash
cat ~/.openagents/autopilot/tauri-control.json
```

If the manifest is stale, restart the app with an explicit manifest path under
`target/` and point the CLI at that file.

If `pylon mode` fails, run:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- --json pylon status
```

The returned blocker codes and config path are the source of truth for the app
projection. Fix the local Pylon config or mark the test as blocked by local
provider prerequisites.

If proof wait times out, run:

```bash
cargo run -p autopilot --bin autopilotctl-tauri -- --json proof status <namespace>
cargo run -p autopilot --bin autopilotctl-tauri -- --json proof doctor <namespace>
```

Then inspect the namespace artifacts under the proof root reported by the JSON
projection.

For isolated proof-root tests, set:

```bash
OPENAGENTS_AUTOPILOT_PROOF_ROOT=/tmp/autopilot-proof/namespaces
```

The app appends the namespace to that root.
