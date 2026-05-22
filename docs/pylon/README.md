# Pylon Standalone Operator Guide

`Pylon` is the standalone provider program for the OpenAgents Compute Market.

Current distributed-training planning lives here too:

- `docs/pylon/2026-04-09-pylon-distributed-training-reference-audit.md`
- `docs/pylon/distributed-training-mvp-roadmap.md`
- `docs/pylon/distributed-training-phase-tracker.md`
- `docs/pylon/distributed-training-launch-status.md`
- `docs/pylon/PYLON_DISTRIBUTED_TRAINING_REHEARSAL_MATRIX.md`
- `docs/pylon/PYLON_DISTRIBUTED_TRAINING_APPLE_REHEARSAL_MATRIX.md`
- `docs/pylon/distributed-training-non-blockers.md`

The matching prior-art and terminology review lives outside `docs/pylon/`:

- `docs/training/distributed-llm-training-runs-diloco-distro-demo-sparseloco-audit.md`

The Autopilot desktop projection contract for Pylon process status and local
proof-flow visualization lives in:

- `docs/pylon/autopilot-proof-contract.md`
- `docs/pylon/autopilot-test-matrix.md`

Use those docs for different questions:

- roadmap and phase tracker: what the admitted-node MVP implementation closed
- launch status: what is still needed for the stronger public launch story to
  be literally true
- training audit: how DiLoCo, DisTrO/DeMo, SparseLoCo, Prime, Templar, and
  other public systems actually relate

The frozen Phase 0 machine-readable contract for that roadmap now lives in:

- `crates/openagents-kernel-core/src/pylon_training.rs`

The default local repo entrypoint is the online earning loop:

```bash
cargo pylon
```

On first launch, Pylon bootstraps its own local config, identity, and ledger under the normal Pylon home path, marks the node online, starts the admin/status loop, publishes provider presence when possible, and runs automatic provider and training intake while the process stays alive. The operator does not need to run `init`, `online`, or `serve` separately for the default earning path.

The terminal shell is now explicit:

```bash
cargo pylon-tui
pylon-tui
pylon tui
```

The explicit CLI commands remain available for inspection, debugging, and service-manager installs:

```bash
cargo pylon-headless <command>
```

Pylon is still a narrow supply connector. It is not a buyer shell, not a labor runtime, and not a raw accelerator exchange.

## Local Proof Authority Runtime

The local proof-runtime foundation now has a dedicated authority lane for
running prod-shaped `Nexus` proof work without touching shared environments.

From a source checkout, use the `oa` binary:

```bash
cargo run -p pylon --bin oa -- proof authority up --json
cargo run -p pylon --bin oa -- proof authority status --json
cargo run -p pylon --bin oa -- proof authority down --json
cargo run -p pylon --bin oa -- proof authority reset --json
```

Use the launch modes deliberately:

- `prod-shaped` is the default and boots local `nexus-relay` with embedded
  `nexus-control`
- `debug-authority` boots `nexus-control` directly when you want a narrower
  authority-only debug path

```bash
cargo run -p pylon --bin oa -- proof authority up --mode debug-authority --json
```

The default proof namespace is `authority`. Its state persists under:

```text
~/.openagents/pylon/proof/namespaces/authority
```

That namespace root keeps the prod-shaped authority world together:

- authority env manifest and logs
- relay data and persistent runtime state
- treasury state, wallet material, and receipt log
- local artifact-store objects plus canonical object-path trace output

The artifact adapter intentionally records canonical publication paths in:

```text
~/.openagents/pylon/proof/namespaces/authority/artifacts/object-trace.jsonl
```

That makes proof runs validate the same object naming path the production
authority expects while still writing to local backing storage.

`proof authority status` probes the local authority health/admin/training
surfaces plus the local artifact-store health route so the operator can see
whether the proof world is actually intact before running a lane.

## Local Proof Fleet Manager

The next proof layer is the isolated fleet manager. It boots fresh worker and
validator homes under a named proof namespace so a local run does not reuse the
shared `authority` state root.

Bring up a smoke fleet explicitly:

```bash
cargo run -p pylon --bin oa -- proof fleet up \
  --namespace proof.smoke \
  --workers 1 \
  --validators 1 \
  --json

cargo run -p pylon --bin oa -- proof fleet status --namespace proof.smoke --json
cargo run -p pylon --bin oa -- proof fleet down --namespace proof.smoke --json
cargo run -p pylon --bin oa -- proof fleet reset --namespace proof.smoke --json
```

Drive a real proof lane with a fresh namespace by default:

```bash
cargo run -p pylon --bin oa -- proof run cs336-a1 --workers 1 --validators 1 --json
```

Pin the namespace only when you want to keep the artifacts and logs under a
stable root for inspection:

```bash
cargo run -p pylon --bin oa -- proof run cs336-a1 \
  --namespace proof.cs336-a1.debug \
  --workers 1 \
  --validators 1 \
  --timeout-seconds 60 \
  --json
```

Each namespace now keeps its own:

- authority runtime state and logs
- per-role worker and validator config, log, and training roots
- deterministic admin/checkpoint ports
- local artifact-store objects and trace file
- `fleet-state.json` and `run-report.json` summaries

The worker and validator roots live under:

```text
~/.openagents/pylon/proof/namespaces/<namespace>/fleet/
```

Use the stale retained-state toggles when you want replayable prod-class
retained state without manual filesystem edits:

```bash
cargo run -p pylon --bin oa -- proof run cs336-a1 \
  --workers 1 \
  --validators 1 \
  --stale-worker-state \
  --stale-validator-state \
  --json

cargo run -p pylon --bin oa -- proof run cs336-a1-stale-recovery \
  --namespace proof.cs336-a1.stale \
  --workers 1 \
  --validators 1 \
  --json
```

Use the replacement-attempt lane when you want the authority-side
`lease -> ack -> failure -> replacement claim -> seal -> reconcile` path from
`#4368` without bringing up worker or validator processes:

```bash
cargo run -p pylon --bin oa -- proof run cs336-a1-replacement-attempt \
  --namespace proof.cs336-a1.replace \
  --workers 0 \
  --validators 0 \
  --json
```

`proof run` now writes the first concrete blocker it can observe into
`run-report.json`. In the current local proof world that commonly means a
critical authority caveat or a node-local training issue instead of a generic
timeout.

The retained-state fixture corpus that seeds those lanes lives under:

```text
fixtures/proof/4368/
```

That corpus packages reduced `#4368` regression inputs:

- stale worker and validator retained-state templates
- accepted-but-payout-open closeout templates
- replacement-attempt contribution/reconcile digests

Each proof run now also writes:

- `authority-state-trace.json` for machine-readable authority/node state at the
  point the run stopped
- `proof-summary.json` for the closure-oriented first-red-stage summary

Those artifacts sit beside `run-report.json` under:

```text
~/.openagents/pylon/proof/namespaces/<namespace>/fleet/
```

Use the dedicated doctor lane when you need transport/provenance attribution
without manually reading logs:

```bash
cargo run -p pylon --bin oa -- proof doctor --namespace proof.cs336-a1.debug --json
```

`proof doctor` reports:

- current `oa` / `nexus-relay` / `nexus-control` binary provenance
- current workspace Git branch and commit
- authority env-manifest key presence
- process-level env drift checks for worker and validator nodes
- a transport split view across authority front-door, relay, artifact-store,
  node-admin, and checkpoint surfaces

The intent is narrow: make it obvious whether the first red stage is authority
state, operator drift, hard-gated retained state, or transport/runtime failure
before anyone reaches for production.

Optional account visibility now also has an explicit headless lane:

```bash
cargo pylon-headless account link --base-url https://openagents.com --token <one_time_token>
cargo pylon-headless account refresh --base-url https://openagents.com
```

Those commands are optional. `account link` connects a local Pylon to a
signed-in OpenAgents account so the web dashboard can show the node. `account
refresh` updates an already-linked node's dashboard snapshot and Codex
capability state without consuming a new one-time token. Neither command runs
during install, bootstrap, or local bring-up by default. The completion and
refresh requests also carry a NIP-98 signed proof tied to the local node
identity; see `docs/pylon/PYLON_ACCOUNT_LINKING_NIP98.md` for the server
verification contract.

For the full operator and product process, including initial link completion,
signed refresh, dashboard visibility, and Codex capability snapshots, see
`docs/pylon/PYLON_ACCOUNT_LINKING_PROCESS.md`.

## Optional OpenAgents Dashboard Linking

Local bring-up is complete even if you never sign in to OpenAgents and never
link a node. Installing `Pylon`, bringing a local runtime online, checking
`pylon status --json`, and running provider commands are already valid on their
own.

Only use account linking if you want the signed-in OpenAgents web dashboard to
show one of your Pylons under `My Pylons`.

Current dashboard flow:

1. Sign in at `https://openagents.com/login` only if you want the web
   dashboard.
2. Open the signed-in Pylon page at `https://openagents.com/pylon`.
3. In the optional linking section, use `Link a Pylon`, then `Generate link command`.
4. Run the generated `pylon account link --base-url https://openagents.com --token <one_time_token>`
   command in the shell where that local Pylon is installed.
5. If you are working from a source checkout instead of an installed `pylon`
   binary, run the equivalent repo command:

```bash
cargo pylon-headless account link --base-url https://openagents.com --token <one_time_token>
```

6. After the command succeeds, refresh `https://openagents.com/pylon` or
   use the dashboard's `Refresh after linking` action.
7. The linked node should then appear in `My Pylons` with its current label,
   identity, runtime state, ready model, eligible product summary, and any
   runtime diagnostic blocker reported by the local Pylon. The signed payload
   also includes a web-safe `capabilities` array. Today Pylon advertises a
   `codex_agent` capability when it can inspect the local Codex runner surface,
   using normalized states such as `ready`, `needs_auth`, or `not_installed`
   without uploading local credential paths or tokens.
8. If the linked-node page later shows stale runtime or Codex readiness, run:

```bash
cargo pylon-headless account refresh --base-url https://openagents.com --json
```

That refresh posts the current runtime and capability snapshot with the same
node-held NIP-98 identity proof. It does not create or transfer account
ownership, and it does not require a fresh link token.

For local diagnostics, `pylon doctor --json` includes a `codex_agent` health
report with the runner kind, optional runner version, status, auth state,
supported actions, required confirmations, and blocker codes. It deliberately
omits raw auth tokens, credential file paths, and local workspace paths from
the JSON output.

Linked Pylons can now run the first conservative brokered Codex chat workload
shape used by `openagents.com` admin chat. The assignment is accepted only when
the requested `workspace_scope` maps to an operator-configured
`codex_workspaces` entry in the local Pylon config:

```json
{
  "codex_workspaces": [
    {
      "id": "repo-42",
      "label": "OpenAgents",
      "root": "/Users/alice/work/openagents"
    }
  ]
}
```

The brokered runner starts Codex in read-only mode with approval requests
rejected. Pylon projects each local Codex notification into a web-safe workload
event such as `run.status`, `assistant.delta`, `tool.start`, `tool.end`,
`patch.preview`, and `pylon.error`, then immediately POSTs that signed event to
openagents.com while the local run is still active. The final completion status
is sent separately after the terminal event.
Runs are bounded by `codex_workload_timeout_seconds` and poll the broker every
`codex_workload_cancel_poll_seconds` while active. Browser cancellation is
acknowledged with `pylon.cancelled` / `run.status: cancelled`; local timeout is
reported as `pylon.timeout` / `run.status: timed_out` and completed as a failed
web completion with a timeout error code because the current web completion API
does not accept `timed_out` as a completion status. Late Codex output after
cancellation or timeout is not forwarded as normal assistant text.
Raw local Codex tokens, browser/WorkOS cookies, and local workspace roots are
not included in the web event payloads.

For a bounded diagnostic poll, run:

```bash
cargo pylon-headless codex workload once --base-url https://openagents.com --json
```

That command first refreshes the linked-node runtime and capability snapshot,
then claims at most one pending `pylon_codex` assignment for the active linked
identity, posts signed workload events as they are produced, posts terminal
completion when the broker has not already made the assignment terminal, and
then exits.

For a live web chat experience, run the long-lived broker poller under the
same service manager that keeps Pylon online:

```bash
cargo pylon-headless codex workload poll --base-url https://openagents.com --interval-seconds 2
```

The web app only queues the assignment. The local poller is assignment intake,
not browser streaming; browser streaming comes from openagents.com after it
accepts each signed event and broadcasts it over Reverb. If the poller is not
running, the web stream will time out waiting for local events even when the
linked-node page shows Codex capability as ready.

The account-link request is signed by the node-held identity key and carries
the local runtime diagnostic snapshot. If the local admin endpoint is stale or
does not explicitly report the same Pylon public key as the signing identity,
the command falls back to a freshly detected local status before completing the
link.

Keep the product posture explicit:

- account linking is optional
- local install and provider bring-up do not depend on login
- the dashboard flow exists for operators who want account-level visibility of
  their nodes
- the one-time token is short-lived, so generate a fresh command if the old one
  expires

## Install Paths

### Windows First: WSL Ubuntu

If the operator is on Windows, prefer WSL Ubuntu as the runtime environment
for `Pylon`.

Use this flow:

1. Install WSL and Ubuntu.
2. Open an Ubuntu shell.
3. Keep the repo, model cache, and build artifacts inside the Linux
   filesystem.
4. Install and run a local Gemma runtime inside Ubuntu.
5. Verify NVIDIA passthrough inside Ubuntu before claiming GPU readiness.

Preferred bootstrap commands on Windows:

```powershell
wsl --install -d Ubuntu
wsl -l -v
wsl -d Ubuntu
```

Inside Ubuntu, verify the shell context:

```bash
uname -sm
pwd
echo "$HOME"
```

Do not default to a checkout under `/mnt/c/...` unless the operator explicitly
wants that path. Prefer:

```bash
cd ~
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
```

Prefer the npm bootstrap lane when the operator already has `npm` or `bun`:

```bash
npx @openagentsinc/pylon
bunx @openagentsinc/pylon
npm install -g @openagentsinc/pylon && pylon
bun install -g @openagentsinc/pylon && pylon
npx @openagentsinc/pylon --version 0.1.16
npx @openagentsinc/pylon --no-launch
npx @openagentsinc/pylon status --json
npx @openagentsinc/pylon --download-curated-cache
```

On Windows, run the bootstrap command inside the target WSL Ubuntu shell unless
the operator explicitly wants a separate Windows-native lane.

That launcher checks GitHub for the latest tagged `pylon-v...` release on each default run, or resolves a specific tagged `Pylon` version when `--version` is provided. It then finds the matching release asset for the local machine, verifies the published SHA-256 checksum, caches the binaries locally, and runs the `init` / `status --json` / `inventory --json` smoke path. It skips Gemma diagnostics by default because hosted homework earning does not require local Gemma weights. Run diagnostics explicitly with `--run-diagnostics`, and prefetch the optional Hugging Face GGUF cache only with `--download-curated-cache`.
The default no-argument path is the intended onboarding lane: it streams terminal
status updates during bootstrap and launches the installed `pylon-tui` binary
when the smoke path finishes. In current releases, the TUI is the user-facing
earning surface and starts/supervises the worker process automatically. Use
`--no-launch` when you want the same install and bootstrap flow without opening
the earning dashboard.
Auto-update belongs to this npm/bun bootstrap lane. The launcher keeps polling
trusted GitHub releases on a six-hour background cadence while the dashboard is
open and restarts from a newer cached archive when one is available.
`GITHUB_TOKEN` or `GH_TOKEN` authenticates those release lookups when a shared
network is close to the unauthenticated GitHub rate limit. `--no-updates` and
`--version` deliberately pin the running cached release.
CLI subcommands are also supported through the package-managed launcher. For
example, `pylon status --json` bootstraps the managed release and then forwards
`status --json` to the installed standalone `pylon` binary instead of opening
the TUI.
If the resolved release does not ship a prebuilt archive for the local
platform, the launcher now falls back to the exact tagged source checkout,
prompts before installing Rust if `cargo` and `rustc` are missing, and builds
`pylon` plus `pylon-tui` locally before continuing into the same smoke path.
For older release tags that still referenced a sibling `spark-sdk` checkout,
that fallback now hydrates the missing sibling repo automatically before
running `cargo build`.
The launcher only caches those standalone binaries under
`~/.openagents/pylon/bootstrap/versions/`. It does not copy or symlink them
into a shared global bin directory, so a global npm or bun install keeps the
package-managed `pylon` command as the stable entrypoint on `PATH`.
The npm bootstrap lane now also emits best-effort anonymous install telemetry
to `openagents.com` so the public stats page can show install starts,
completions, source-build fallbacks, Rust prompts, and smoke-test outcomes.
Set `OPENAGENTS_DISABLE_TELEMETRY=1` to disable that stream, or point
`OPENAGENTS_TELEMETRY_URL` at a non-production endpoint during local validation.
The bootstrap summary now ends with an explicit operator verdict:

- `fully online`
- `runtime ready`
- `installed but runtime missing`

That verdict is intentionally separate from local cache/download success. The
bootstrap does not auto-install or auto-mutate a local runtime; it tells the
operator exactly what is missing and how to finish the bring-up path.

Prefer an official release asset when one exists for the user's platform. Those archives ship the standalone `pylon` and `pylon-tui` binaries directly, so the operator does not need a Rust toolchain just to bring a node online.

For maintainers cutting a multi-platform Pylon release, use
`scripts/release/pylon-binary-release.sh --version <version> --publish` once
per platform from clean checkouts on the matching host machines. The helper now
creates the GitHub release when the tag does not exist yet and uploads or
replaces the current platform assets when the tag already exists, so
`darwin-arm64` and `linux-x86_64` can land on the same `pylon-v...` tag
without manual asset shuffling.

Release archives cut after 2026-04-27 also include a minimal packaged Psionic
runtime surface at `./psionic`, including
`psionic/target/release/psionic-train`. That packaged runtime is what lets
normal npm-installed Pylons advertise the current homework training capability
without requiring users to clone a sibling Psionic checkout. If public stats
show online Pylons with `homework_worker_training_capability_missing`, first
check whether those nodes are running an older archive or direct binary that
lacks the packaged `./psionic` surface.

Use a direct release asset install only when the operator explicitly does not want the npm bootstrap layer.
Direct release assets are manual-upgrade binaries today. They do not contain a
native GitHub release poller and they do not honor the npm launcher's
`--no-updates` flag because the launcher is not in that process tree. If public
stats still show an older `pylon/<version>` after a newer release exists, treat
that row as a truthful old runtime heartbeat until the operator restarts through
the npm/bun launcher or manually replaces the direct archive:

```bash
./pylon
./pylon init
./pylon status --json
./pylon inventory --json
./pylon config show
./pylon doctor
./pylon gemma diagnose gemma-4-e2b --max-output-tokens 96 --repeats 3
```

Bare interactive `./pylon` opens the same earning dashboard and supervises the
worker. Noninteractive `./pylon` and `./pylon --config-path <path>` remain the
direct worker/service path for automation.

Use a source checkout only when:

- no matching official release asset exists for the machine
- the operator needs the retained Psionic benchmark path and wants to work from source
- the operator is modifying or validating the code itself

## Launch Truth

The market is still the **OpenAgents Compute Market**.

At launch, the first standalone `Pylon` sellable lane is:

- `psionic.local.inference.gemma.single_node`

The first bounded hosted training starter lane is:

- `psion_cs336_a1_demo_v1` on the hosted CS336 Assignment 1 starter network

Users do not choose a CS336-specific command. A paid-training-capable Pylon
running the default `pylon` loop admits its local capabilities and payout
target, asks Nexus for available work, and receives CS336 Assignment 1 starter
work when that is the currently hosted starter lane available to the node.

The current recommended public paid-training Pylon binary for this path is
`pylon-v0.1.16`, exposed through `@openagentsinc/pylon` `0.1.17` or newer. The
`0.1.17` package is a launcher-only release that forwards subcommands such as
`pylon status --json` and uses a bounded background update cadence; it still
runs the latest trusted `pylon-v...` standalone binary for the machine. The
`0.1.16` binary keeps the `0.1.12` homework-earning TUI, the TUI-managed worker lifecycle,
bootstrap behavior that launches the TUI after smoke checks, opt-in-only Gemma
diagnostics/downloads, the Mac-safe Psionic training-worker launch path, the
public-safe signed-artifact path, accepted-work payout projection, validator
intake enabled by default, worker-first/validator-second default role claims,
failed retained-runtime lease retirement, nonfatal scheduler-error handling,
retained snapshot reuse for validator replay retries, and the Autopilot proof
projection fixes needed for a normal node that advertises both worker and
validator roles. `0.1.15` keeps the `0.1.14` long hosted homework ID hashing
and TUI worker-exit status fixes, then blocks terminal window seal until the
worker contribution artifact bundle has uploaded and verified. That prevents
Nexus from sealing a contribution whose signed artifact fetch can still return
404 during validator replay. `0.1.16` adds the packaged Psionic runtime surface
and `psionic/target/release/psionic-train` to the standalone archive, so
normal npm-installed Pylons can advertise homework training capability without
a sibling Psionic checkout. For Psionic-backed homework/training jobs, it still
prefers a current `target/release/psionic-train` binary when the operator has
already built one and otherwise falls back to `cargo run --release`, with
signal and log-tail details in failure receipts instead of only `code -1`.
Older releases may still bring up local Gemma inference; `pylon-v0.1.10`
contains the earlier payout fixes but its TUI did not supervise the worker and
is not enough for the current user path, and `pylon-v0.1.11` can still launch
Mac training through debug Cargo in the failure mode tracked by issue #4414.
`pylon-v0.1.4`
proved public install plus worker artifact sealing, and `pylon-v0.1.5` proved
the earning-loop packaging fixes, but neither is sufficient closeout proof for
hosted CS336 earning because retained failed validator leases can block fresh
paid worker intake. `pylon-v0.1.6` adds validator defaults but can still block
terminal closeout behind artifact/TRN publication during the validator path.
If `npx @openagentsinc/pylon` resolves an older version, update before testing
paid training. If a platform does not yet have a matching `pylon-v0.1.16`
release asset, use the npm bootstrap source fallback
or a newer official release that includes these same paid-training guarantees.
The `0.1.16` release receipt is
`docs/reports/nexus/20260427-pylon-v0.1.16-release.json`. It proves the
packaged Psionic runtime release asset and npm bootstrap smoke for issue #4451.
The `0.1.17` package-only launcher receipt is
`docs/reports/nexus/20260427-pylon-bootstrap-v0.1.17-release.json`. It proves
the CLI forwarding and bounded update-polling tests plus npm publication.

Current source for Pylon v0.2 changes the paid-work registration path: normal
startup no longer creates a Spark payout destination. Operators must configure
`payout_destination` to an external LDK-compatible Lightning target before the
node is eligible for new paid work. Supported v0.2 targets are BOLT12 offers,
BIP353 names, LNURL-pay targets, and per-payment BOLT11 invoices. Spark
destination creation is no longer part of normal Pylon startup or registration,
and Spark-only nodes are not eligible for new paid work after cutover.

This is a temporary compatibility path, not the final wallet product. The
planned built-in LDK wallet work is tracked from
`OpenAgentsInc/openagents#4520`; until those issues land, Pylon does not create
wallet-owned Lightning invoices or send withdrawals from a local LDK wallet in
the normal v0.2 release path.

The prior `0.1.15` release receipt is
`docs/reports/nexus/20260426-pylon-v0.1.15-release.json`. It proves the
artifact-before-seal regression test, release asset, bootstrap smoke, and
production earning drill for issue #4449.
The `0.1.14` release receipt is
`docs/reports/nexus/20260425-pylon-v0.1.14-release.json`. It proves the
darwin-arm64 release asset, the long-homework-ID path hashing regression test,
and the TUI worker-exit status fix.
The `0.1.13` release receipt is
`docs/reports/nexus/20260423-pylon-v0.1.13-release.json`. It proves the public
darwin-arm64 release asset, the npm publish, the `npx @openagentsinc/pylon@0.1.13 --help`
smoke, and the removal of legacy runtime wording from the public onboarding
path.
The `0.1.12` release receipt is
`docs/reports/nexus/20260423-issue-4414-pylon-v0.1.12-release.json`. It proves
the release assets, npm bootstrap behavior, TUI-managed worker, no default
Gemma model download, and the issue #4414 Psionic training launch regression.
The prior `0.1.11` release receipt remains
`docs/reports/nexus/20260423-072712-pylon-v0.1.11-release.json`. Keep using
`docs/reports/nexus/20260423-050434-pylon-v0.1.10-release.json` for the last
fresh npm-installed public release proof that settled a wallet payout.

That recommended Pylon version is necessary but not sufficient. Hosted starter
work also
requires production Nexus to run the corresponding hosted-starter fix set: the
auto-launched starter lane must target online Pylons by
`min_pylon_version=0.1.16`, must not require the provider's build digest to
match the Nexus service build, and must skip exhausted or sealed starter runs
instead of returning `training_scheduler_run_not_schedulable` to the default
Pylon loop. The current public floor is `min_pylon_version=0.1.16` so Nexus
does not assign new accepted-work homework to standalone clients missing the
packaged Psionic runtime required for homework-worker admission. If Nexus is
older, a public `pylon-v0.1.16` or newer node can come online
correctly and still fail to receive fresh starter work. Treat that as a Nexus
deployment/readiness problem, not a user opt-in problem.

For public paid-training onboarding, the user command remains only `pylon`.
Do not ask the user to run a CS336-specific opt-in command, do not ask for
OpenAgents operator bearer tokens, and do not ask for
`GOOGLE_APPLICATION_CREDENTIALS` or
`OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN`. Public artifact transfer should
use Nexus-brokered signed URLs. Public payment proof should be accepted-work
only; periodic placeholder or liveness payouts are not evidence for the
homework earning claim.

The broader market direction still includes `inference`, `embeddings`, broader
training lanes, and later bounded execution. The current operator bring-up in
this repo is narrower on purpose: get one honest local Gemma inference lane
online first, then prove the bounded accepted-work training starter lane without
claiming an open-ended training marketplace.

Do not describe launch as raw GPU or raw accelerator trading. Accelerator, memory, and platform facts remain capability-envelope qualifiers that refine supply rather than replace product identity.

Current planned-but-not-live surfaces:

- broader embeddings lanes
- pooled inference routing
- broad wallet-shell UX
- sandbox execution as a generally released family

Training remains bounded starter work rather than a broad launched product
family, but `pylon status --json` now projects a bounded
`adapter_training_contributor` capability envelope when the machine can actually
prove the local prerequisites:

- a sibling `Psionic` checkout with the machine `psionic-train` surface present
- NVIDIA CUDA telemetry visible to the node
- one admitted H100-class CUDA worker posture
- local disk and non-loopback network posture suitable for checkpoint and
  coordinator participation

That projection is intentionally narrow. `Pylon` does not yet launch or
advertise broader mixed-backend or permissionless training claims.

The retained training shell now also includes the first internal
`psionic-train` supervision core in `apps/pylon/src/lib.rs`. It can launch one
manifest-bound child process, retain per-attempt stdout and stderr logs, watch
heartbeat files under the assigned run root, refuse conflicting assignments,
persist exit state and machine-readable failure receipts, and rotate preserved
attempt history across drain and restart. That foundation is intentionally
internal for now, but it is no longer isolated from the live coordinator path.
When bare `pylon` is online, the retained training state now also drives an
automatic `Nexus` intake pass: `Pylon` admits the node, claims one compatible
lease, acknowledges the assignment, and persists the leased window state into
`training/state/runtime-state.json` without requiring an operator-crafted
command. `Pylon` now also materializes the accepted lease into the exact
`psionic.train.invocation_manifest.v1` family that `psionic-train` already
validates and acknowledges Nexus with that retained machine-manifest path. The
runtime projection freezes the admitted lane, role, operation, work class,
coordination envelope, and current Psionic release/build/environment identity
derived from the sibling checkout, including dirty-tree override posture when
the local Psionic checkout is not clean. Assignment intake now also resolves
the canonical run manifest and latest checkpoint artifacts through the Nexus
training artifact resolver, requests signed read URLs, verifies the returned
payload digests and sizes when authority metadata is available, and stages the
materialized bytes into the retained run root plus the local resolved-artifact
cache under `training/runs/<run_id>/artifacts/resolved/` and
`training/download-cache/resolved/`. When `pylon serve` stays online after that
lease is acknowledged, the same retained state now also drives the existing
`psionic-train` supervisor path automatically: `Pylon` launches the retained
manifest, captures stdout and stderr into attempt-scoped logs under
`training/runs/<run_id>/supervisor/`, updates retained process state and exit
status, and preserves the runtime status packets already emitted by
`psionic-train`. Once that retained runtime reaches a terminal state, `Pylon`
now runs the existing training artifact courier and TRN publication sweep
automatically, then posts the matching Nexus coordination notices for window
progress, checkpoint publication, and failures or refusals. Those receipt
attempts persist retry state in the retained training runtime journal so later
`pylon serve` loops can finish the handoff without an operator moving files by
hand. `dashboard/current_dashboard.json` and `alerts/active_alerts.json` still
remain local operator surfaces today; the automated Nexus intake path currently
tracks the kernel-owned artifact families already modeled in the retained
training layout.

`Pylon` now also has the first training-coordination HTTP client in
`apps/pylon/src/lib.rs`. It wraps the existing kernel training-policy and
training-run lookup routes and defines one idempotent node-side coordination
lane for node admission, run lease, heartbeat, assignment ack, drain notice,
failure notice, window progress, and checkpoint publication. The client keeps
the `Nexus` bearer token env-only through
`OPENAGENTS_PYLON_TRAINING_NEXUS_BEARER_TOKEN`; it does not persist that secret
into `PylonConfig` or the retained runtime-state store. `pylon serve` now calls
that same client automatically on a short interval whenever the provider is in
`online` mode, so training assignment discovery and acceptance use the existing
retained Pylon process instead of a second launcher or a manual operator loop.
The default bare `pylon` command sets that desired mode to `online` before
entering the same service loop.

The #4385 local proof runtime includes a hosted-starter model for this path:

```bash
cargo run -p pylon --bin oa -- proof run cs336-a1-hosted-starter --workers 1 --validators 1 --json
```

That proof lane starts default Pylon nodes, lets their normal lease request
auto-launch the hosted CS336 A1 starter run, then waits for accepted work and
accepted-work payout closeout. The older `cs336-a1` proof lane remains the
explicit admin-launch proof path.

Hosted Nexus now paces homework work automatically without changing the public
user command. Production runs an internal CS336 A1 homework dispatcher every
10 minutes. Each cycle creates a fresh homework run, targets online eligible
Pylons on `pylon-v0.1.16` or newer, allows duplicated starter work across
cycles, pays 25 sats only for accepted closeouts, and caps the automatic cycle
at 6,400 sats. The loop intentionally lives in Nexus rather than in each Pylon:
users only run `pylon` and stay online.

Admins can still pace or prove the amount of homework work offered to online
Pylons with a manual override endpoint. The cron-safe endpoint is:

```bash
curl -X POST "$NEXUS_BASE_URL/v1/admin/homework/cs336-a1/dispatch" \
  -H "Authorization: Bearer $NEXUS_CONTROL_ADMIN_BEARER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "run_count": 3,
    "max_contributors_per_run": 1,
    "amount_sats": 7,
    "total_budget_sats": 21,
    "run_slug_prefix": "cron.hourly",
    "reuse_existing_run": false
  }'
```

That endpoint creates fresh CS336 A1 homework runs by default, so the same
assignment can be offered again across intervals when the operator intentionally
wants duplicated starter work. It still pays only accepted homework closeouts:
launching a run does not send sats, and periodic placeholder or liveness
payouts must remain disabled for this claim. The default pacing contract is one
fresh run, one contributor per run, one sat per accepted contribution, online
nodes only, `min_pylon_version=0.1.16`, and no active-run reuse. Operators can
raise `run_count`, `max_contributors_per_run`, or `amount_sats` in cron while
using `total_budget_sats` as a per-call cap.

`Pylon` now also carries the first retained training artifact courier and
checkpoint-serving foundation. `apps/pylon/src/lib.rs` can now:

- upload checkpoint, contribution-proof, and score bundles through
  Nexus-brokered temporary signed artifact URLs with retry and digest
  verification
- resolve retained training run bootstrap artifacts through Nexus-issued signed
  read URLs and materialize `run_manifest.json`, `latest_pointer.json`, and the
  current `checkpoint_manifest.json` into the run-scoped local filesystem shape
- download and verify those same bundles into the retained download cache under
  `training/download-cache/`
- expose a bounded local checkpoint HTTP path for recovery clients
- inspect local manifests and artifact state through
  `pylon training artifacts inspect`
- garbage-collect stale downloaded artifacts through
  `pylon training artifacts gc`

Public Pylon operators should not need `GOOGLE_APPLICATION_CREDENTIALS`,
`OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN`, or direct bucket credentials for
the default starter training path. The default persisted artifact credential
source is now `nexus_signed_url`: Pylon asks Nexus for short-lived read or
write authorization for the exact retained artifact object, then uploads or
downloads through the returned URL and re-verifies the digest and byte length
when authority metadata is available. Direct GCS credentials remain an
operator-only fallback for local tests, private deployments, and emergency
debugging; setting a direct GCS bearer token or ADC file explicitly opts into
that older courier path.

If the signed artifact broker fails, the failure is intentionally surfaced in
local operator status instead of being hidden behind a generic closeout stall.
`pylon training status`, `pylon status`, and their JSON variants classify
temporary signed URL failures as `artifact_authorization`, object transfer
failures as `artifact_transfer`, funding shortages as `treasury_balance`, and
payout send or confirmation failures as payout-specific blockers.

Accepted-work payout projection is also intentionally separate from liveness
or placeholder payout records. Pylon only treats a treasury ledger entry as
the payout for accepted homework work when it is tied to the accepted outcome
id for that window. Status output now carries the accepted outcome id,
accepted-work payout id, payment id, and payout reconciliation state so an
operator can distinguish "work accepted but payout pending" from "a different
heartbeat or placeholder payment happened."

`Pylon` now also has the first retained training TRN publication lane. The
same `apps/pylon/src/lib.rs` surface can now:

- publish `kind:39501` training node records from retained run-manifest state
- publish `kind:39511` assignment-accepted and artifact-uploaded receipts
- publish `kind:39520` staged artifact locators after the retained Nexus
  signed-artifact courier uploads and re-verifies the underlying objects
- persist event ids and `a` references into the retained training runtime
  state for later operator/admin projection through
  `pylon training publish [--manifest <path>]`

The retained training node record now also carries one scheduler-readable
training capability envelope derived from local host telemetry and retained
training runtime state. That envelope is published both in the TRN
`kind:39501` content and in explicit capability tags so downstream schedulers
and public stats consumers can read:

- schema version (`provider.training_capability_envelope.v2`)
- capability tier (`tier0_presence` through `tier4_authority`)
- backend families and accelerator inventory
- memory floor and currently available memory
- throughput band
- lease reliability class
- replay capability
- artifact upload latency class
- benchmark-lane availability and runtime-surface detection
- eligible work classes such as `validation_replay`, `evaluation`,
  `adapter_training`, `grouped_replica_stage_execution`, and
  `full_island_local_update_training`
- eligible replica types such as `single_node`, `grouped_replica`, and
  `island`

For the current public launch explanation of weaker-device work classes and how
accepted-work payout differs from uptime or presence alone, use
`docs/pylon/WEAK_DEVICE_WORK_CLASSES_FAQ.md`.

This is still the node-side claim lane, not the final authoritative `Nexus`
publication lane. The current locator status is intentionally `staged`, and
`Nexus` closeout state still remains the authoritative settlement boundary.
The authoritative `Nexus` lane now also publishes typed `kind:39520` training
artifact locators for accepted local updates, reconciled aggregate artifacts,
and promoted checkpoints so relay readers can distinguish round contributions
from aggregate and checkpoint lineage without moving heavy bytes over Nostr.

`Pylon` also now has a retained authority-sync lane for training closeout and
reputation state. `pylon training sync [--json]` will:

- fetch adapter contribution outcomes from `Nexus` for retained manifests
- fetch accepted training outcomes and cache them as accepted sealed-window
  closeout state
- query retained training relays for relevant `kind:1985` `NIP-32` labels that
  target the local node or previously published training events
- persist those caches into the retained training runtime-state store
- fail closed on automatic training readvertisement when a cached hard-gate
  training label such as `trn/build=revoked` still applies

Those retained caches are now projected through the operator surface instead of
staying buried in `state/runtime-state.json` only:

- `pylon training status [--json]`
  - renders the retained training operator report directly: current run,
    active window, current runtime state, any retained leased assignment that
    has been accepted but not launched yet, last checkpoint pointer, validator
    queue, retained capability tier, retained TRN publication pointers, recent
    closeouts, recent refusals or failures, and the resolved Psionic runtime
    root or exact runtime-detection error
- `pylon status`
  - now appends a concise training summary to the top-level provider status,
    including the training headline (`active`, `leased`, `blocked`, `ready`,
    or `inactive`), the retained capability tier, the active or leased
    run/window when present, the last checkpoint ref, validator-queue depth,
    and the most recent retained training issue
- `pylon doctor`
  - now includes a dedicated `training` block covering runtime-surface
    discovery, contributor readiness, retained capability tier and capability
    envelope, the resolved Psionic repo root and source when present, or the
    exact runtime-surface failure when detection cannot succeed,
    checkpoint-serve URL, retained role claims, retention limits, blocked
    reputation labels, and recent retained issues

The shared admin port now also exposes training-aware HTTP routes alongside the
existing provider status routes:

- `GET /v1/training/status`
  - returns the same machine-readable operator report used by
    `pylon training status --json`
- `POST /v1/training/sync`
  - runs the retained closeout and reputation sync lane and returns the sync
    report
- `POST /v1/training/node-record/refresh`
  - republishes the retained `kind:39501` training node record for every
    retained network and updates the stored publication pointer

The node-side publication lane now has two explicit operator commands:

- `pylon training publish [--manifest <path>]`
  - publishes any retained node record, assignment receipt, and staged
    artifact-locator state that does not already have persisted publication
    pointers
- `pylon training refresh [--json]`
  - republishes only the retained node record on demand without replaying the
    full receipt or artifact-locator publication sweep

The retained config now also carries one explicit `training` block for the
future admitted-node lane. That block freezes:

- allowed training networks
- role claims
- local training run root
- artifact credential-source names
- checkpoint serve address
- training authority URL
- training relay list
- validator enablement
- disk quota and retention limits

`Pylon` also now keeps one separate retained runtime-state file under the
training run root at `state/runtime-state.json`. That store is intentionally
separate from the inference ledger and is where cached training manifests,
lease state, window state, active runtime state, latest published TRN ids,
contribution outcomes, accepted closeouts, and retained reputation labels
belong as the training shell grows.

## Prerequisites

Minimum local requirements:

- either an official `Pylon` release asset for the local platform, or a local source checkout plus Rust
- a writable local home/config path

Runtime-specific requirements:

- a local Gemma runtime endpoint at `local_gemma_base_url`
  (default `http://127.0.0.1:11434`) that answers `GET /api/tags` and
  `POST /api/chat`, with a Gemma 4 model loaded
- on Windows, prefer that runtime inside WSL Ubuntu rather than a separate
  Windows host install
- on Ubuntu, Debian, or WSL Ubuntu, install native prerequisites before the
  runtime or a source build:
  - `sudo apt-get update`
  - `sudo apt-get install -y pkg-config libssl-dev curl git zstd`
- the exact runtime implementation is intentionally operator-chosen; this
  onboarding doc only assumes an endpoint that speaks the current local Gemma
  API shape and has a loaded `gemma4:*` model
- preferred runtime model names are:
  - `gemma4:e2b`
  - `gemma4:e4b`
  - `gemma4:26b`
  - `gemma4:31b`
- the curated Hugging Face GGUF cache under `~/.openagents/pylon/models/huggingface/` is optional and does not make the sellable lane eligible by itself
- sibling `psionic` checkout only if the operator explicitly needs the retained benchmark and validation lane

For packaged training-capable Pylons, runtime discovery now checks these paths
in order before you need to set `OPENAGENTS_PSIONIC_REPO`:

- the explicit `OPENAGENTS_PSIONIC_REPO` override when you set it
- `/var/lib/pylon/psionic`, the hosted-fleet runtime install path
- the source-tree sibling path used by local dev checkouts
- a sibling `psionic` checkout found by walking up from the current working
  directory or executable path
- `~/psionic`, `~/code/psionic`, `~/work/psionic`, and `~/src/psionic`
- `~/.worktrees/psionic*`, `~/code/.worktrees/psionic*`, and
  `~/work/.worktrees/psionic*`

If your Psionic checkout lives somewhere else, set
`OPENAGENTS_PSIONIC_REPO=/absolute/path/to/psionic`. `pylon training status`
and `pylon doctor` now expose the resolved path and source, or the exact
runtime-detection failure when none of those paths work.

For the hosted GCP Pylon fleet, install the packaged runtime with:

```bash
NEXUS_PYLON_RUNTIME_ARCHIVE=/tmp/psionic-runtime-<psionic-sha>.tar.gz \
scripts/deploy/nexus/29-install-pylon-psionic-runtime.sh
```

That script installs `/var/lib/pylon/psionic`, writes a `pylon.service`
drop-in that sets `OPENAGENTS_PSIONIC_REPO=/var/lib/pylon/psionic`, and
restarts the service. This step only proves the runtime surface is present.
Paid work still requires a registered external LDK-compatible payout target for
the Pylon identity.

If local Gemma supply is not available, `Pylon` should still install and run, but it should report `degraded` or `offline` truthfully rather than pretending healthy supply exists.

If the Windows machine has an NVIDIA GPU, verify passthrough inside WSL before
proceeding:

```bash
nvidia-smi
ls /usr/lib/wsl/lib
```

If `nvidia-smi` fails inside WSL, treat that as a machine/runtime setup
problem first. Once the local Gemma runtime is running, use its own status
surface plus `nvidia-smi` to confirm the runtime is actually using the GPU.

## Quick Start

Start the default user-facing earning dashboard:

```bash
cargo pylon
```

If you installed from a release asset instead of a source checkout, run:

```bash
./pylon
```

That command stays in the foreground and is the path a normal provider should
keep running. It opens a minimal homework-focused TUI, initializes local state,
starts a supervised worker process, flips desired mode to `online`, starts the
local admin/status loop, heartbeats provider presence, keeps announcements
fresh, and performs automatic provider and training intake whenever the node has
eligible local supply or hosted training capability for currently available
jobs.

The explicit TUI commands are equivalent:

```bash
cargo pylon-tui
pylon-tui
pylon tui
```

The current TUI is intentionally minimal. It removes the chat composer and
transcript from the default user surface and focuses on the pieces that matter
for earning: node status, wallet/balance state, stacker progress, recent paid
activity, and the active homework-run/window details returned by the local
worker. It should not show internal Nexus treasury recovery text, stale sync
warnings, or Gemma model-management controls during normal paid-homework
bring-up. Keep the window open; the supervised worker stays online underneath
it and receives Nexus-dispatched homework runs automatically. Normal public
users do not run a CS336 opt-in command and do not trigger jobs themselves.

When a node reports provider presence to `Nexus`, that same heartbeat now also
carries a private hosting telemetry snapshot alongside the public-safe launch
summary. `Nexus` retains runtime state, backend availability, inventory rows,
and host facts such as CPU, memory, disk, network, thermal, power, and GPU
telemetry for online Pylons, while the public website remains limited to coarse
aggregate counters and recent public-safe summaries.

Headless Gemma operator commands now exist too:

- `cargo pylon-headless gemma`
- `cargo pylon-headless gemma download remaining --transport curl`
- `cargo pylon-headless gemma diagnose gemma-4-e2b --max-output-tokens 96 --repeats 3`
- `cargo pylon-headless gemma benchmark all --download-missing --mode matrix`

Do not run these Gemma commands as part of normal hosted-homework onboarding.
They are explicit inference diagnostics only: `gemma diagnose` confirms a
loaded runtime model is actually answering `/api/chat`, `gemma benchmark`
measures inference behavior, and `gemma download ...` intentionally pulls the
optional local GGUF cache.

Important:

- `pylon gemma download ...` only downloads GGUF files into `~/.openagents/pylon/models/huggingface/`
- `/model <model>` targets the preferred local Gemma runtime model for future TUI and provider work
- `/uninstall <model>` removes the cached GGUF and any Pylon-managed local runtime shim for that model when one exists
- `pylon gemma download ... --transport curl` is the explicit fallback when the default Rust HTTP transport is unhappy in an SSH/VPN-constrained shell
- `pylon gemma diagnose ...` only benchmarks models that are already loaded in the configured local runtime
- the latest first-run diagnostic report is retained at `~/.openagents/pylon/diagnostics/gemma/latest.json`
- downloaded GGUFs alone do not make supply eligible
- `Pylon` still requires a local runtime endpoint at `local_gemma_base_url` (default `http://127.0.0.1:11434`) that answers `/api/tags` and has a Gemma 4 model loaded
- if `pylon online` reports `degraded` or `NO_ELIGIBLE_SUPPLY`, check that runtime first before falling back to a source build

Treat the full `gemma benchmark` matrix as a retained validation lane, not as required bring-up. That command shells into a compatible Psionic checkout for the real runtime benchmark. It now searches the same common checkout and worktree paths as training runtime detection before you need to set `OPENAGENTS_PSIONIC_REPO=/absolute/path/to/psionic`. If the discovered checkout is stale or missing the retained Gemma benchmark entrypoints, refresh it or point `OPENAGENTS_PSIONIC_REPO` at a clean compatible `psionic` root explicitly.

Pylon now also keeps a focused local ledger at `~/.openagents/pylon/ledger.json`. That file is the retained standalone durability layer for relay state, NIP-90 jobs, invoices, payments, settlements, and local activity replay. It is intentionally narrower than the old archived Pylon database.

The retained relay controls are now exposed in both places:
- TUI: `/relay list`, `/relay add <wss://...>`, `/relay remove <wss://...>`, `/relay refresh`
- headless: `cargo pylon-headless relays`, `cargo pylon-headless relay add <wss://...>`, `cargo pylon-headless relay remove <wss://...>`, `cargo pylon-headless relay refresh`

Relay refresh now reuses the local Pylon node identity for NIP-42 `AUTH` challenges by default. If you need to disable that on a local node, use `cargo pylon-headless config set relay_auth_enabled false`.

The retained provider announcement controls now also exist in both places:
- TUI: `/announce`, `/announce publish`, `/announce refresh`
- headless: `cargo pylon-headless announce`, `cargo pylon-headless announce publish`, `cargo pylon-headless announce refresh`

The current retained announcement scope is one honest local text-generation handler for `kind:5050`. Pylon only publishes it when a local Gemma-backed text-generation path is actually eligible.
When bare `pylon` is running, or when `pylon serve` is running with the node
already marked `online`, eligible local Gemma supply auto-publishes or refreshes
that handler announcement as part of the normal service loop. `announce publish`
remains the explicit manual path when you want to force the publish step
yourself.

The retained provider intake controls also exist in both places:
- TUI: `/provider scan [--seconds <n>]`, `/provider run [--seconds <n>]`
- headless: `cargo pylon-headless provider scan [--seconds <n>]`, `cargo pylon-headless provider run [--seconds <n>]`

The current retained execution scope is narrow and honest. Pylon subscribes to retained inbound `kind:5050` requests on the configured relays, filters targeted jobs, and only accepts work when the provider is online and a local Gemma text-generation path is actually ready. The default bare `pylon` loop performs short automatic provider-intake passes so the node actually processes eligible work without requiring a separate manual `provider run`. `scan` still records intake decisions without executing, and `run` remains the explicit one-shot operator path for debugging, manual replay, or forcing the next pass immediately. `run` has two honest paths:

- for unpriced local work, it publishes a `kind:7000` processing update, executes accepted jobs locally, publishes the retained `kind:6050` result, and links those published event IDs back into the local ledger
- for explicit paid requests, the current v0.2 paid-work path depends on the
  Nexus-dispatched accepted-work payout flow and the registered external
  LDK-compatible payout target. The old retained Spark invoice path is not part
  of the normal v0.2 paid-work eligibility path.

When payout settlement is observed, the retained `jobs`, `earnings`, `receipts`,
and `activity` views project that local provider settlement state from the Pylon
ledger instead of forcing the operator to reconstruct it from relay logs.

Repeated `provider run` passes now also keep durable replay protection in
`processed-provider-requests.json` beside the rolling `ledger.json` window.
That means old retained request IDs stay blocked even after they roll out of the
recent `jobs` list, and intake subscriptions no longer fail the whole pass just
because one configured relay in the pool is disconnected. The retained rule is:
if at least one relay connects and subscribes, Pylon keeps the pass alive and
dedupes against the durable processed-request set instead of only the bounded
recent-job window.

If the current runtime cannot create or pay with a local wallet, the provider
path fails honestly instead of pretending the request is payable.

The retained wallet controls now also exist in both places, but in the current
v0.2 external-target release they should be treated as status, retained ledger,
and compatibility surfaces. Real local LDK receive/send behavior is planned in
`OpenAgentsInc/openagents#4520`.
- TUI: `/wallet`, `/wallet balance`, `/wallet address`, `/wallet invoice <sats> [--description <text>]`, `/wallet pay <bolt11> [--amount-sats <n>]`, `/wallet history [--limit <n>]`
- headless: `cargo pylon-headless wallet status|balance|address|invoice|pay|history|lock`

Wallet runtime selection is now explicit. `wallet_runtime_kind=external_target`
is the default compatibility runtime and keeps `payout_destination` as the
paid-work settlement target. `wallet_runtime_kind=mock` exists for deterministic
tests. `wallet_runtime_kind=ldk_node` is a placeholder that reports
`unavailable` until the real LDK node runtime lands. `pylon wallet status
--json` reports the selected kind as `runtime.runtime_kind`.

The default wallet recovery model is one phrase, not two. Pylon derives the
future LDK Node 64-byte node entropy from the existing Pylon identity mnemonic
with HKDF-SHA256 over the BIP39 seed and the domain label
`openagents-pylon/ldk-node/v1/<wallet_network>`. Status output only exposes the
redacted derivation metadata: source, version, domain label, and a SHA-256
digest of the derived entropy. It must not print the raw mnemonic or raw node
entropy. Advanced operators may use `wallet_entropy_override_path` or `pylon
wallet entropy import|export <path>` for explicit entropy files, but that is an
escape hatch rather than the normal setup path. Directly sharing the mnemonic
bytes without HKDF domain separation is compatibility-only and should not be the
default.

Pylon also prepares the built-in LDK wallet storage root before the real node
runtime owns funds. The default layout is:

```text
~/.openagents/pylon/wallet/ldk/
  node/
  sqlite/
  backup-staging/
  wallet-lock
  backup-manifest.json
  last-registration.json
```

The directories are private (`0700` on Unix) and wallet metadata files are
private (`0600` on Unix). Startup refuses world-readable recovery material,
including the identity mnemonic and any explicit entropy override file. The
single-writer guard lives at `wallet-lock`; inspect it with `pylon wallet lock
status [--json]` and clear it with `pylon wallet lock clear [--json]` only
after the report says the owning process is stale. An active lock means another
Pylon process is using the wallet state and must be stopped first.

For operator accounting, the bounded retained `ledger.wallet.payments` list is
not the final source of truth for built-in LDK wallet balances. Current v0.2
status and earnings projections should be read as retained ledger/accounting
state until the local LDK wallet runtime and payment-history sync land.

The TUI operator sidebar now also treats wallet balance bring-up more honestly.
Its refresh path loads network status plus balance first instead of waiting on
recent payment history, and it renders `pending` when a zero balance is not yet
authoritative. If a retained balance already exists, the sidebar keeps showing
that cached total until a connected live balance replaces it.
The `Lifetime earned` and stacker-rank counters stay keyed to retained provider
settlement history rather than the current wallet balance, so withdrawing sats
does not reduce the amount the node has already earned.
When the long-running Pylon service loop cannot keep its Nexus heartbeat or payout-
target sync healthy, the retained runtime snapshot now drops out of `online`
truth and surfaces that control-plane error instead of continuing to claim a
healthy online state.

The retained provider payout controls now also exist in both places:
- TUI: `/payout`, `/payout history [--limit <n>]`, `/payout withdraw <bolt11> [--amount-sats <n>]`
- headless: `cargo pylon-headless payout [--limit <n>]`, `cargo pylon-headless payout withdraw <bolt11> [--amount-sats <n>]`

That path projects retained provider earnings, current retained wallet/accounting
state, and prior withdrawal outcomes from the same local ledger. In the current
external-target v0.2 path, real local wallet withdrawal remains planned LDK
wallet work rather than a guarantee of the release binary.

The retained transcript observability commands now also exist in the shell:
- TUI: `/jobs [--limit <n>]`, `/earnings`, `/receipts [--limit <n>]`, `/activity [--limit <n>]`
- headless: `cargo pylon-headless jobs [--limit <n>]`, `cargo pylon-headless earnings`, `cargo pylon-headless receipts [--limit <n>]`, `cargo pylon-headless activity [--limit <n>]`

Those views stay ledger-backed. They can still replay retained provider jobs, earnings, receipts, and relay activity even when there is no live provider service answering local HTTP routes.

The retained ledger writes are now file-replace atomic, so the TUI and
headless JSON views no longer transiently fall back to an empty local ledger
while a concurrent provider pass is rewriting `ledger.json`. The `jobs` view
also now overlays payment evidence from retained settlement rows onto matching
job IDs, which means `payout_sats` and `payment_pointer` stay visible even when
an older live recent-job row still says `completed_local` instead of carrying
the later wallet credit detail itself.

The first retained buyer controls now also exist in both places:
- TUI: `/job submit [--bid-msats <n>] [--model <id>] [--provider <pubkey>] [--request-json <json>] <prompt>`, `/job watch [<request_event_id>] [--seconds <n>]`, `/job history [--limit <n>]`, `/job replay <request_event_id>`, `/job approve <request_event_id>`, `/job deny <request_event_id>`, `/job policy [show|auto|manual]`
- headless: `cargo pylon-headless job submit [--bid-msats <n>] [--model <id>] [--provider <pubkey>] [--output <mime>] [--request-json <json>] <prompt>`, `cargo pylon-headless job watch [<request_event_id>] [--seconds <n>]`, `cargo pylon-headless job history [--limit <n>]`, `cargo pylon-headless job replay <request_event_id>`, `cargo pylon-headless job approve <request_event_id>`, `cargo pylon-headless job deny <request_event_id>`, `cargo pylon-headless job policy [show|auto|manual]`

That path publishes a retained `kind:5050` buyer request to the configured relays and persists the outbound request locally in the Pylon ledger. It already supports plain prompt text and structured JSON payload mode. The watch path subscribes to retained `kind:7000` feedback and `kind:6050` results for local buyer jobs, streams those updates into the transcript, and persists the observed payment-required and result state back into the same retained ledger record. `job history` projects the retained buyer ledger back into a short local summary list. `job replay` expands one retained request back into its stored lifecycle, settlement state, and matching relay activity.

When a provider returns `payment-required`, Pylon now keeps the invoice amount, provider pubkey, Bolt11 string, and final payment outcome in the same retained buyer record. Manual buyer mode uses `/job approve <request_event_id>` or `/job deny <request_event_id>`. Auto-pay mode is explicit and off by default. Use `/job policy auto` or `cargo pylon-headless job policy auto` to enable it, and `/job policy manual` to turn it back off.

Initialize a standalone config and identity:

```bash
cargo pylon-headless init
```

With a release asset install, use the same commands through the shipped binary:

```bash
./pylon init
./pylon status --json
./pylon inventory --json
./pylon config show
./pylon online
```

If the shipped binary is current, `status`, `inventory`, `config show`, and `doctor` should all agree on the current standalone lane:

- backend naming should be `local_gemma`
- the launch product should be `psionic.local.inference.gemma.single_node`
- legacy `gpt_oss_*` or older runtime-specific product names should not be the surfaced launch truth for standalone Pylon onboarding

Inspect status:

```bash
cargo pylon-headless status
cargo pylon-headless status --json
cargo pylon-headless online
cargo pylon-headless announce
cargo pylon-headless announce publish
cargo pylon-headless provider scan --seconds 5
cargo pylon-headless provider run --seconds 5
cargo pylon-headless job submit --model gemma4:e2b --bid-msats 21000 "write a haiku about bitcoin"
cargo pylon-headless job watch --seconds 30
cargo pylon-headless gemma
cargo pylon-headless gemma download remaining --transport curl
cargo pylon-headless gemma diagnose gemma-4-e2b --max-output-tokens 96 --repeats 3
```

Run the retained Psionic benchmark lane only when the operator explicitly needs it:

```bash
cargo pylon-headless gemma benchmark all --download-missing --mode matrix --peer-base-url http://127.0.0.1:18080
```

Inspect provider truth:

```bash
cargo pylon-headless backends
cargo pylon-headless products
cargo pylon-headless sandbox
cargo pylon-headless inventory
cargo pylon-headless jobs
cargo pylon-headless earnings
cargo pylon-headless receipts
cargo pylon-headless activity
```

Inspect the retained wallet status and ledger surfaces:

```bash
cargo pylon-headless wallet status
cargo pylon-headless wallet balance
cargo pylon-headless wallet address
cargo pylon-headless wallet invoice 21 --description "pylon receive"
cargo pylon-headless wallet pay <bolt11> --amount-sats 21
cargo pylon-headless wallet history --limit 10
cargo pylon-headless payout --limit 10
cargo pylon-headless payout withdraw <bolt11> --amount-sats 21
```

In the current v0.2 external-target release, address, invoice, pay, and payout
withdrawal commands may report that the local wallet runtime is unavailable.
That refusal is expected until the built-in LDK wallet tracker lands.

Move the node through explicit lifecycle controls:

```bash
cargo pylon-headless online
cargo pylon-headless pause
cargo pylon-headless resume
cargo pylon-headless offline
```

Run the local admin/status loop:

```bash
cargo pylon-headless serve
```

Important:

- bare `pylon` / `cargo pylon` initializes local state, forces desired mode to
  `online`, and then enters the same long-running service loop.
- `pylon serve` does not implicitly force the node online.
- lifecycle is explicit for `serve`; use `pylon online` / `offline` / `pause` /
  `resume` when you manage `serve` directly under a service manager.
- status should show `unconfigured`, `ready`, `online`, `paused`, `draining`, `degraded`, `offline`, or `error` truthfully
- when sandbox supply is declared, `status`, `backends`, `sandbox`, `jobs`, and `receipts` should surface execution classes, profile IDs, termination reasons, and failure reasons without inventing a separate sandbox-only provider model
- `cargo run -p pylon -- <command>` remains a direct fallback if you do not want the alias

Hosted Nexus treasury operator details now live in
[`docs/nexus-treasury.md`](../nexus-treasury.md). That runbook covers payout
target registration, the `nexus-control treasury ...` CLI, funding-target
generation, and the public-safe treasury counters exposed on `/api/stats`.

## Config and Paths

Default home:

```text
$HOME/.openagents/pylon
```

Important overrides:

- `OPENAGENTS_PYLON_HOME`
- `OPENAGENTS_PYLON_CONFIG_PATH`

The generated config currently includes:

- node label
- payout destination
- identity path
- admin sqlite path
- admin listen address
- wallet network
- wallet API key env var
- wallet storage dir
- local Gemma runtime base URL (`local_gemma_base_url`)
- inventory-control toggles with `local_gemma_*` names
- declared sandbox profiles

## Headless Service Guidance

`Pylon` is service-style. The simplest supported operational pattern is:

1. run `pylon` from an installed binary, or `cargo pylon` from a source checkout
2. keep that process online for all relevant jobs
3. use `pylon status`, `backends`, `products`, `inventory`, `jobs`, `earnings`, `receipts`, and `activity` for observability
4. use `pylon sandbox` when you need the declared runtime/profile view for bounded `sandbox_execution`

The explicit service-manager pattern remains available for operators who want a
split lifecycle:

1. initialize once with `pylon init`
2. set desired mode explicitly with `pylon online` or `pylon offline`
3. run `pylon serve` under the service manager

While desired mode is `online`, the long-running loop refreshes status,
heartbeats provider presence, keeps the announcement current, and runs short
automatic provider and training-intake passes. `provider run --seconds <n>`
remains a manual one-shot pass rather than the normal way to keep an online
node serving jobs.

The plain-text `jobs` view is intentionally terminal-oriented: it prints older
jobs first so the newest completed block lands closest to the shell prompt.
Use `jobs --json` when you want the structured machine-readable report instead
of the prompt-oriented text layout.

### `systemd` example

```ini
[Unit]
Description=OpenAgents Pylon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/path/to/openagents
Environment=OPENAGENTS_PYLON_HOME=/var/lib/openagents/pylon
ExecStart=/usr/bin/env pylon
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### `launchd` / user-session guidance

On macOS, run the same `pylon` command under `launchd`, `tmux`, or another persistent user-session manager. The operational requirement is a stable long-running online Pylon process, not a specific packaging format. Use the split `pylon online` plus `pylon serve` pattern only when you intentionally want lifecycle control outside the default entrypoint.

The current binary-first distribution lane is GitHub Releases with per-platform archives. Source checkout plus Cargo remains the fallback for unsupported platforms and local development.

## Verification and Release Discipline

Do not treat Pylon as shipped because the binary compiles.

Before calling it launch-ready, use:

- [PYLON_VERIFICATION_MATRIX.md](./PYLON_VERIFICATION_MATRIX.md)
- [`scripts/pylon/verify_standalone.sh`](../../scripts/pylon/verify_standalone.sh)
- [`scripts/pylon/verify_nip90_wallet.sh`](../../scripts/pylon/verify_nip90_wallet.sh)

Those materials cover:

- backend detection
- launch-product derivation
- sandbox runtime/profile detection and declared execution classes
- lifecycle transitions
- restart and replay expectations
- local observability surfaces
- local relay and wallet roundtrip coverage for the retained NIP-90 lane
- receipt and earnings visibility, including sandbox failure and termination detail
- Autopilot parity checks
- rollout and launch-truth gates

The retained NIP-90 and wallet verification lane is local and explicit.
`scripts/pylon/verify_nip90_wallet.sh` sets a fresh standalone Pylon home to
`wallet_network=regtest`, checks the retained headless report commands, and then
runs the focused local websocket-relay and wallet-hook tests that cover provider
intake, buyer submit/watch/pay, payout persistence, and retained activity replay.
It does not claim a live funded Spark regtest backend or a production-ready
built-in LDK wallet.
