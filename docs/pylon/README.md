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

Use those docs for different questions:

- roadmap and phase tracker: what the admitted-node MVP implementation closed
- launch status: what is still needed for the stronger public launch story to
  be literally true
- training audit: how DiLoCo, DisTrO/DeMo, SparseLoCo, Prime, Templar, and
  other public systems actually relate

The frozen Phase 0 machine-readable contract for that roadmap now lives in:

- `crates/openagents-kernel-core/src/pylon_training.rs`

The default local repo entrypoint is the small terminal shell:

```bash
cargo pylon
```

On first launch, the TUI bootstraps its own local Pylon config and identity under the normal Pylon home path. It does not ask the user to run a manual init step first.

The current provider automation stays in the explicit headless CLI:

```bash
cargo pylon-headless <command>
```

It is still a narrow supply connector. It is not a buyer shell, not a labor runtime, and not a raw accelerator exchange.

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
```

That command is optional. It is meant for operators who choose to connect a
local Pylon to a signed-in OpenAgents account so the web dashboard can show the
node. It does not run during install, bootstrap, or local bring-up by default.
The completion request also carries a NIP-98 signed proof tied to the local
node identity; see `docs/pylon/PYLON_ACCOUNT_LINKING_NIP98.md` for the server
verification contract.

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
2. Open the signed-in dashboard at `https://openagents.com/dashboard`.
3. In the optional linking section, use `Link a Pylon`, then `Generate link command`.
4. Run the generated `pylon account link --base-url https://openagents.com --token <one_time_token>`
   command in the shell where that local Pylon is installed.
5. If you are working from a source checkout instead of an installed `pylon`
   binary, run the equivalent repo command:

```bash
cargo pylon-headless account link --base-url https://openagents.com --token <one_time_token>
```

6. After the command succeeds, refresh `https://openagents.com/dashboard` or
   use the dashboard's `Refresh after linking` action.
7. The linked node should then appear in `My Pylons` with its current label,
   identity, runtime state, ready model, and eligible product summary.

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
4. Install and run Ollama inside Ubuntu.
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
npx @openagentsinc/pylon --version 0.0.1-rc10
npx @openagentsinc/pylon --no-launch
npx @openagentsinc/pylon --download-curated-cache
```

On Windows, run the bootstrap command inside the target WSL Ubuntu shell unless
the operator explicitly wants a separate Windows-native lane.

That launcher checks GitHub for the latest tagged `pylon-v...` release on each default run, or resolves a specific tagged `Pylon` version when `--version` is provided. It then finds the matching release asset for the local machine, verifies the published SHA-256 checksum, caches the binaries locally, runs the `init` / `status --json` / `inventory --json` smoke path, and then drives `pylon gemma diagnose <model>`. It only prefetches the optional Hugging Face GGUF cache when `--download-curated-cache` is set, because the sellable lane still depends on the configured local runtime endpoint rather than the local GGUF cache alone.
The default no-argument path is the intended onboarding lane: it streams terminal
status updates during bootstrap and opens `pylon-tui` automatically when the
smoke path finishes. Use `--no-launch` when you want the same install and
bootstrap flow without handing the terminal to the TUI.
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

Use a direct release asset install only when the operator explicitly does not want the npm bootstrap layer:

```bash
./pylon
./pylon init
./pylon status --json
./pylon inventory --json
./pylon config show
./pylon doctor
./pylon gemma diagnose gemma-4-e2b --max-output-tokens 96 --repeats 3
```

Bare `./pylon` now opens the terminal UI. Use `./pylon-tui` only when you want
to target the shell binary explicitly.

Use a source checkout only when:

- no matching official release asset exists for the machine
- the operator needs the retained Psionic benchmark path and wants to work from source
- the operator is modifying or validating the code itself

## Launch Truth

The market is still the **OpenAgents Compute Market**.

At launch, the first standalone `Pylon` sellable lane is:

- `psionic.local.inference.gemma.single_node`

The broader market direction still includes `inference`, `embeddings`, and later bounded execution. The current operator bring-up in this repo is narrower on purpose: get one honest local Gemma inference lane online first.

Do not describe launch as raw GPU or raw accelerator trading. Accelerator, memory, and platform facts remain capability-envelope qualifiers that refine supply rather than replace product identity.

Current planned-but-not-live surfaces:

- broader embeddings lanes
- pooled inference routing
- broad wallet-shell UX
- sandbox execution as a generally released family

Training remains a planned surface rather than a launched product family, but
`pylon status --json` now projects a bounded `adapter_training_contributor`
capability envelope when the machine can actually prove the local prerequisites:

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
When `pylon serve` is online, the retained training state now also drives an
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

`Pylon` now also carries the first retained training artifact courier and
checkpoint-serving foundation. `apps/pylon/src/lib.rs` can now:

- upload checkpoint, contribution-proof, and score bundles to the frozen
  `gs://` layout with retry and digest verification
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

The transport credentials still stay env-only at runtime. `Pylon` resolves the
persisted credential-source name through Application Default Credentials and
can mint GCS bearer tokens from either `GOOGLE_APPLICATION_CREDENTIALS` or
instance metadata without writing raw secrets into retained state.

`Pylon` now also has the first retained training TRN publication lane. The
same `apps/pylon/src/lib.rs` surface can now:

- publish `kind:39501` training node records from retained run-manifest state
- publish `kind:39511` assignment-accepted and artifact-uploaded receipts
- publish `kind:39520` staged artifact locators after the retained GCS courier
  uploads and re-verifies the underlying objects
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

- an Ollama-compatible local runtime endpoint at `local_gemma_base_url`
  (default `http://127.0.0.1:11434`) that answers `GET /api/tags` and
  `POST /api/chat`, with a Gemma 4 model loaded
- on Windows, prefer that runtime inside WSL Ubuntu rather than a separate
  Windows host install
- on macOS, the shortest supported runtime path today is:
  - `brew install ollama`
  - `brew services start ollama`
  - `ollama pull gemma4:e2b`
- on Ubuntu, Debian, or WSL Ubuntu, install native prerequisites before the
  runtime or a source build:
  - `sudo apt-get update`
  - `sudo apt-get install -y pkg-config libssl-dev curl git zstd`
- on Ubuntu, Debian, or WSL Ubuntu, the shortest supported runtime path today
  is:
  - `curl -fsSL https://ollama.com/install.sh | sh`
  - `ollama serve`
  - `curl http://127.0.0.1:11434/api/tags`
  - `ollama pull gemma4:e2b`
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

If local Gemma supply is not available, `Pylon` should still install and run, but it should report `degraded` or `offline` truthfully rather than pretending healthy supply exists.

If the Windows machine has an NVIDIA GPU, verify passthrough inside WSL before
proceeding:

```bash
nvidia-smi
ls /usr/lib/wsl/lib
```

If `nvidia-smi` fails inside WSL, treat that as a machine/runtime setup
problem first. Once Ollama is running, `ollama ps` plus `nvidia-smi` is the
normal check that the runtime is actually using the GPU.

## Quick Start

Open the local terminal shell:

```bash
cargo pylon
```

If you installed from a release asset instead of a source checkout, run:

```bash
./pylon
```

The first cut is intentionally small. It renders one full-screen transcript shell with:

- whether a Gemma 4-serving path is visible to the node
- live host, CPU, memory, swap, uptime, disk, network, thermal, and power-source state
- a GPU summary and NVIDIA power telemetry when the host can report it
- a built-in Hugging Face Gemma GGUF catalog that shows which curated models are installed, missing, or actively downloading
- a retained transcript area for local shell activity
- a bottom textbox where plain text submits a prompt, `/help` shows the retained shell commands, `/model <model>` targets a Gemma runtime model for future local work, `/uninstall <model>` removes a local Gemma model, and `/download <model>` pulls a curated Gemma GGUF into the local Pylon cache

The shell keeps submitted input in the transcript, streams the local Gemma reply back into the same view while it is generating, and carries prior user and assistant turns into the next prompt when local Gemma weights are available. The TUI prepends a plain-terminal system instruction on each local chat request so replies avoid Markdown and LaTeX formatting the transcript cannot render. The right column now shows a curated Hugging Face catalog for `gemma-4-e2b`, `gemma-4-e4b`, `gemma-4-26b-a4b`, and `gemma-4-31b`, with live per-model progress bars while downloads are active. That catalog is intentionally separate from the live runtime truth: the `Gemma Models` panel now shows `runtime ready: ...` from the local backend before listing the optional local GGUF cache rows, so a healthy Ollama-loaded `gemma4:e4b` no longer looks "missing" just because no curated cache file was downloaded. Directly under that catalog, the `Pylon Operator` panel now projects the retained operator truth that matters during bring-up: whether the node is still coming online, idle and ready for jobs, actively running intake, or waiting on settlement, along with the current wallet total, 24-hour found and matching demand counts, 24-hour processed and settled counts, the last job result, and online uptime. The operator header now keeps the desired `mode` separate from the live `runtime`, drops the confusing standalone `state` line, and shows an idle online runtime as `ready` while the detail row explains that automatic intake passes are active. The first visible frame is also now neutral during bring-up: before the first status refresh lands, the shell shows `loading current status` instead of painting an offline-looking default state. Downloaded GGUFs land under `~/.openagents/pylon/models/huggingface/`. `/model <model>` persists a preferred Gemma target, maps it to the local runtime naming when possible, and warms that model through the configured local runtime endpoint. `/uninstall <model>` removes the matching cached GGUF and, when `local_gemma_base_url` points at a local Ollama instance, also removes the corresponding local runtime model. The current local chat path accepts the preferred model when it is visible through the configured local runtime endpoint. The `System` block is meant to show what the node can honestly report right now about local capacity and headroom. On Macs that includes power source and battery state. On NVIDIA hosts it can also show `power.draw / power.limit` from `nvidia-smi`. The normal provider automation now lives in the long-running `cargo pylon-headless serve` path below, while explicit `provider run` remains available as a manual one-shot pass. `cargo run -p pylon-tui` remains the direct fallback if you want to bypass the alias.

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

Use the first, third, and fourth commands for normal onboarding. They inspect the optional curated cache, confirm a loaded runtime model is actually answering `/api/chat`, and persist a local first-run diagnostic report without requiring a sibling `psionic` checkout. Use `gemma download ...` only when you intentionally want the local GGUF cache too.

Important:

- `pylon gemma download ...` only downloads GGUF files into `~/.openagents/pylon/models/huggingface/`
- `/model <model>` targets the preferred local Gemma runtime model for future TUI and provider work
- `/uninstall <model>` removes the cached GGUF and also removes the corresponding local Ollama model when Pylon is pointed at a local Ollama runtime
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
When `cargo pylon-headless serve` is running and the node is `online` with
eligible local Gemma supply, Pylon now auto-publishes or refreshes that handler
announcement as part of the normal service loop. `announce publish` remains the
explicit manual path when you want to force the publish step yourself.

The retained provider intake controls also exist in both places:
- TUI: `/provider scan [--seconds <n>]`, `/provider run [--seconds <n>]`
- headless: `cargo pylon-headless provider scan [--seconds <n>]`, `cargo pylon-headless provider run [--seconds <n>]`

The current retained execution scope is narrow and honest. Pylon subscribes to retained inbound `kind:5050` requests on the configured relays, filters targeted jobs, and only accepts work when the provider is online and a local Gemma text-generation path is actually ready. When `cargo pylon-headless serve` is running in `online` mode, that service loop now performs short automatic provider-intake passes so the node actually processes eligible work without requiring a separate manual `provider run`. `scan` still records intake decisions without executing, and `run` remains the explicit one-shot operator path for debugging, manual replay, or forcing the next pass immediately. `run` has two honest paths:

- for unpriced local work, it publishes a `kind:7000` processing update, executes accepted jobs locally, publishes the retained `kind:6050` result, and links those published event IDs back into the local ledger
- for explicit paid requests, it stops at `payment-required`, creates a local Bolt11 invoice through the retained Spark wallet path, publishes that invoice in a `kind:7000` feedback event, and persists the amount plus Bolt11 string in the local ledger

When that invoice is later marked paid in the local wallet, the next `provider run` picks the same job back up, records the settled payment, executes the work, publishes the retained result, and persists the settlement outcome. The retained `jobs`, `earnings`, `receipts`, and `activity` views now project that local NIP-90 provider settlement state directly from the Pylon ledger instead of forcing the operator to reconstruct it from relay logs.

Repeated `provider run` passes now also keep durable replay protection in
`processed-provider-requests.json` beside the rolling `ledger.json` window.
That means old retained request IDs stay blocked even after they roll out of the
recent `jobs` list, and intake subscriptions no longer fail the whole pass just
because one configured relay in the pool is disconnected. The retained rule is:
if at least one relay connects and subscribes, Pylon keeps the pass alive and
dedupes against the durable processed-request set instead of only the bounded
recent-job window.

If the local wallet cannot create an invoice, the provider path fails honestly instead of pretending the request is payable.

The retained wallet controls now also exist in both places:
- TUI: `/wallet`, `/wallet balance`, `/wallet address`, `/wallet invoice <sats> [--description <text>]`, `/wallet pay <bolt11> [--amount-sats <n>]`, `/wallet history [--limit <n>]`
- headless: `cargo pylon-headless wallet status|balance|address|invoice|pay|history`

For operator accounting, the bounded retained `ledger.wallet.payments` list is
no longer treated as the source of truth for credited totals. When `earnings`
needs a local wallet fallback, Pylon now performs a full Spark payment-history
sync, caches a compact credit summary under `ledger.wallet.credits`, and uses
the original payment `created_at_ms` to decide what counts toward "today". That
avoids both lifetime undercounting from the rolling 256-entry window and false
"today" credits caused by later status refreshes rewriting `updated_at_ms`.

The TUI operator sidebar now also treats wallet balance bring-up more honestly.
Its refresh path loads network status plus balance first instead of waiting on
recent payment history, and it renders `pending` when a zero balance is not yet
authoritative. If a retained balance already exists, the sidebar keeps showing
that cached total until a connected live balance replaces it.
The `Lifetime earned` and stacker-rank counters stay keyed to retained provider
settlement history rather than the current wallet balance, so withdrawing sats
does not reduce the amount the node has already earned.
When `cargo pylon-headless serve` cannot keep its Nexus heartbeat or payout-
target sync healthy, the retained runtime snapshot now drops out of `online`
truth and surfaces that control-plane error instead of continuing to claim a
healthy online state.

The retained provider payout controls now also exist in both places:
- TUI: `/payout`, `/payout history [--limit <n>]`, `/payout withdraw <bolt11> [--amount-sats <n>]`
- headless: `cargo pylon-headless payout [--limit <n>]`, `cargo pylon-headless payout withdraw <bolt11> [--amount-sats <n>]`

That path projects retained provider earnings, current wallet balance, and prior withdrawal outcomes from the same local ledger. `payout withdraw` uses the retained wallet send path, persists the resulting withdrawal record locally, and appends a matching relay-activity fact so later transcript views can replay it honestly.

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
- legacy `gpt_oss_*`, `ollama_*`, or Apple-FM-only product names should not be the surfaced launch truth for standalone Pylon onboarding

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

Inspect or operate the standalone Spark wallet:

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

- `pylon serve` does not implicitly force the node online.
- lifecycle is explicit; use `pylon online` / `offline` / `pause` / `resume`
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
- local Gemma runtime base URL (`local_gemma_base_url`; legacy `ollama_base_url` still loads on read)
- inventory-control toggles with `local_gemma_*` names; legacy `gpt_oss_*` and `ollama_*` names still load on read
- declared sandbox profiles

## Headless Service Guidance

`Pylon` is service-style. The simplest supported operational pattern is:

1. initialize once with `cargo pylon-headless init`
2. set desired mode explicitly with `cargo pylon-headless online` or `cargo pylon-headless offline`
3. run `cargo pylon-headless serve` under a local service manager
4. use `cargo pylon-headless status`, `backends`, `products`, `inventory`, `jobs`, `earnings`, `receipts`, and `activity` for observability
5. use `cargo pylon-headless sandbox` when you need the declared runtime/profile view for bounded `sandbox_execution`

While desired mode is `online`, the long-running `serve` loop is the normal
operator path: it refreshes status, heartbeats provider presence, keeps the
announcement current, and runs short automatic provider-intake passes against
the configured relays. `provider run --seconds <n>` remains a manual one-shot
pass rather than the normal way to keep an online node serving jobs.

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
ExecStart=/usr/bin/env cargo pylon-headless serve
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### `launchd` / user-session guidance

On macOS, run the same `cargo pylon-headless serve` command under `launchd`, `tmux`, or another persistent user-session manager. The operational requirement is explicit lifecycle control plus a stable long-running `serve` process, not a specific packaging format.

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

The retained NIP-90 and wallet verification lane is local and explicit. `scripts/pylon/verify_nip90_wallet.sh` sets a fresh standalone Pylon home to `wallet_network=regtest`, checks the retained headless report commands, and then runs the focused local websocket-relay and wallet-hook tests that cover provider intake, buyer submit/watch/pay, payout persistence, and retained activity replay. It does not claim a live funded external Spark regtest backend.
