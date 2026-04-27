# Autopilot Pylon And Proof Projection Contract

This contract describes the read side that the Tauri Autopilot shell consumes
from Pylon and the local proof runtime. It exists so the desktop UI, Pylon CLI,
and proof fixtures can evolve without turning TypeScript into the source of
authority for provider or Nexus state.

## Ownership

- `apps/autopilot/src-tauri/src/pylon.rs` owns desktop process supervision,
  binary resolution, command timeouts, redaction, and Tauri event emission.
- `apps/pylon` owns provider behavior, provider mode changes, Pylon status
  JSON, proof runs, fleet lifecycle, and proof artifacts.
- The React UI owns presentation only. It renders projections returned by
  Tauri commands and never reconstructs provider, proof, payout, wallet, or
  Nexus truth from raw logs.

## Binary Resolution

Autopilot resolves `pylon` and `oa` in this order:

1. `OPENAGENTS_PYLON_BINARY` / `OPENAGENTS_OA_BINARY`
2. bundled sidecar/resource candidates next to the app executable
3. workspace build outputs under `target/{debug,fast-release,release}`
4. app-managed cache under `~/.openagents/autopilot/bin`
5. existing Pylon bootstrap cache under `~/.openagents/pylon/bootstrap`
6. `PATH`

Packaged Autopilot builds should prefer bundled sidecars or the app-managed
cache. Source-checkout development may use workspace binaries.

## Pylon Status Projection

The `pylon_get_status` command runs:

```bash
pylon --config-path <config> status --json
```

It returns:

```json
{
  "installed": true,
  "configured": true,
  "processState": "running",
  "providerState": "online",
  "desiredMode": "online",
  "pid": 12345,
  "listenAddr": "127.0.0.1:9468",
  "binaryPath": "/path/to/pylon",
  "configPath": "/Users/example/.openagents/autopilot/pylon/config.json",
  "pylonHome": "/Users/example/.openagents/autopilot/pylon",
  "executionBackend": "local_gemma",
  "readyModel": "gemma4:e4b",
  "productsVisible": 2,
  "productsEligible": 1,
  "queueDepth": 0,
  "uptimeSeconds": 42,
  "blockerCodes": [],
  "lastAction": "provider online",
  "lastError": null,
  "lastExitCode": null,
  "lastUpdatedAt": "1776650000000"
}
```

`processState` is Autopilot's process observation. `providerState` and
`desiredMode` come from Pylon. A provider can be `online` while Autopilot is
only observing an externally started process, and a process can be `running`
while the provider is `unconfigured`.

## Pylon Mutating Commands

Autopilot exposes:

- `pylon_start`
- `pylon_stop`
- `pylon_restart`
- `pylon_set_mode`
- `pylon_open_logs`

`pylon_start` launches:

```bash
pylon --config-path <config> serve
```

with `OPENAGENTS_PYLON_HOME` set to the app-owned home unless the caller
overrides it. Logs are written under:

```text
~/.openagents/autopilot/logs
```

`pylon_set_mode` accepts only `online`, `offline`, `pause`, and `resume`. The
Rust command validates that input before spawning Pylon.

## Proof Run Projection

Autopilot drives proof lanes through `oa proof run`:

```bash
oa proof run cs336-a1 --namespace <namespace> --workers 1 --validators 1 --json
oa proof run cs336-a1-stale-recovery --namespace <namespace> --workers 1 --validators 1 --json
oa proof run cs336-a1-replacement-attempt --namespace <namespace> --workers 0 --validators 0 --json
oa proof run a1-minimal-distributed-lm-launch-a --namespace <namespace> --json
oa proof run a1-minimal-distributed-lm-launch-b --namespace <namespace> --json
```

The returned projection is assembled from:

```text
~/.openagents/pylon/proof/namespaces/<namespace>/fleet/run-report.json
~/.openagents/pylon/proof/namespaces/<namespace>/fleet/authority-state-trace.json
~/.openagents/pylon/proof/namespaces/<namespace>/fleet/proof-summary.json
~/.openagents/pylon/proof/namespaces/<namespace>/artifacts/object-trace.jsonl
```

The projection has this shape:

```json
{
  "namespace": "proof.autopilot.cs336.a1.1776650000000",
  "lane": "cs336-a1",
  "status": "accepted",
  "firstRedStage": null,
  "firstRedSubject": null,
  "blockerId": null,
  "detail": null,
  "runId": "run.cs336.a1.demo",
  "windowId": "window.local",
  "assignmentId": "assignment.local",
  "leaseId": "lease.local",
  "membershipRevision": "1",
  "closeoutStage": "accepted",
  "closeoutNextAction": null,
  "closeoutLastError": null,
  "workers": [],
  "validators": [],
  "transport": {
    "authority": "ok",
    "relay": "ok",
    "artifactStore": "ok",
    "nodeSurfaces": "ok"
  },
  "artifacts": {
    "root": "/Users/example/.openagents/pylon/proof/namespaces/proof.autopilot.cs336.a1.1776650000000",
    "runReportPath": ".../run-report.json",
    "authorityTracePath": ".../authority-state-trace.json",
    "summaryPath": ".../proof-summary.json",
    "artifactTracePath": ".../object-trace.jsonl"
  },
  "firstFailedAuthorityWrite": null,
  "localSimulation": true,
  "simulatedTreasury": true,
  "updatedAt": "1776650000000"
}
```

The proof card must show the namespace, lane, run/window/assignment/lease IDs,
first red stage, first red subject, blocker, closeout state, transport split,
worker nodes, validator nodes, and artifact paths. Empty fields should remain
explicitly visible as `none` or `unknown`.

The A1 minimal distributed LM lanes are deterministic local projections. They
do not require live Pylons. `a1-minimal-distributed-lm-launch-a` writes the
canonical participant counters where support/verifier work can count as
`training_accepted_contributors` but not
`training_model_progress_contributors`. `a1-minimal-distributed-lm-launch-b`
writes the promoted-checkpoint projection where accepted local updates enter
aggregate/promotion lineage. Both lanes persist the same run report, authority
trace, proof summary, first-red-stage fields, public stats projection, payout
projection, and signed artifact class projection as the CS336 proof lanes.

Reduced, redacted contract fixtures live under:

```text
fixtures/proof/autopilot/
```

Those fixtures validate that the Rust projections decode with the same
camel-case JSON shape consumed by the TypeScript UI.

## Proof Doctor

Autopilot exposes `proof_doctor`, which runs:

```bash
oa proof doctor --namespace <namespace> --json
```

The doctor output refreshes the transport split projection so the command pane
can show whether the failure is authority front door, relay, artifact store,
node admin/checkpoint surface, retained state, or local operator drift.

## Safety Rules

- Do not shell through user-provided command text.
- Redact token-like, key-like, password-like, mnemonic, and bearer values from
  errors before projecting them into the UI.
- Do not use production Nexus for ordinary UI validation.
- Treat local proof lanes as simulated treasury unless the authority projection
  explicitly states otherwise.
- Keep app-owned Pylon config and logs separate from a user's default Pylon
  home unless the caller passes an explicit override.
- Do not close a Pylon/proof issue from branch-only evidence; close only after
  the implementation lands on the repository default branch.

## Verification

Minimum local verification for this surface:

```bash
cargo check -p autopilot
cargo test -p autopilot --lib
cd apps/autopilot && bun run build
```

Functional proof verification should additionally run at least one local proof
lane from the same commit:

```bash
cargo run -p pylon --bin oa -- proof run cs336-a1 \
  --namespace proof.autopilot.smoke \
  --workers 1 \
  --validators 1 \
  --timeout-seconds 60 \
  --json
```
