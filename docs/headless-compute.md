# Headless Compute

`autopilot-headless-compute` provides three headless runtime surfaces for the current NIP-90 + Spark flow:

- `relay`: tiny local websocket relay for deterministic buyer/provider smoke runs
- `provider`: headless provider mode with a separate Nostr/Spark identity path
- `buyer`: headless Buy Mode loop using the current wallet by default

The same repo now also has an app-owned desktop control plane for the running GUI:

- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/compute_mcp.rs`
- `apps/autopilot-desktop/src/bin/autopilot_compute_mcp.rs`

That control plane is intentionally UI-synced. `autopilotctl` drives the running
desktop app through the same split-shell truth model the user sees on screen:
hotbar shell, Provider Control, Buy Mode, Wallet, Log Stream, and the mirrored
runtime status surfaces all come from the same app-owned snapshot instead of a
separate headless-only state machine.

## Desktop control and `autopilotctl`

`autopilotctl` is the thin CLI client for the running desktop-control runtime.
It can:

- fetch the current desktop-control snapshot
- stream desktop-control event batches
- inspect advanced provider inventory truth for local, clustered, and sandbox
  compute surfaces, including projection source, open quantities, and section
  blockers
- inspect buyer procurement truth for spot and forward RFQs, quote selection,
  accepted orders, and the topology/proof/environment posture of quoted compute
- inspect cluster, sandbox, training, proof, and challenge status through the same
  app-owned snapshot the desktop uses, including delivery acceptance,
  validator outcomes, accepted training outcomes, checkpoint lineage, and
  settlement history when kernel authority is configured
- inspect and mutate the bounded `gemma4:e4b` finetuning surface through
  authenticated desktop control, including project binding, explicit split
  identity, tokenizer/template compatibility receipts, async jobs, checkpoint
  and artifact truth, and bounded promotion state
- list, open, focus, close, and inspect panes in the running desktop shell
- inspect the bounded contributor-beta control surface and its governed
  submission state through the same pane snapshot contract as the GUI
- emit structured desktop perf snapshots from the app-owned `Frame Debugger`
  surface through `autopilotctl perf`
- inspect the active local-runtime truth model (`local_runtime`) and raw GPT-OSS runtime state
- inspect the pooled-inference mesh surface the desktop currently sees, including
  local serving versus proxy posture, routed model inventory, warm replicas, and
  mesh membership
- refresh the active local runtime and wallet state
- inspect Apple FM adapter inventory and current session attachment truth from the
  same desktop snapshot the workbench uses
- load, unload, attach, and detach Apple FM adapters through desktop control
- refresh, warm, unload, and wait on GPT-OSS directly
- bring the provider online or offline
- inspect active-job and buy-mode state
- create, start, pause, reset, retarget, and inspect desktop-owned AttnRes lab
  runs without opening the pane UI
- create, upload, start, wait on, and inspect desktop-owned sandbox jobs
- download sandbox workspace files and declared sandbox artifacts through the
  control plane
- inspect the current Tailnet roster (`tailnet`) sourced from `tailscale status --json`
  through the same desktop-control snapshot the provider-status pane now uses
- inspect the current tunnel-status surface (`tunnels`) that will eventually
  reflect Psionic-backed service exposure when the desktop app starts owning
  those flows directly
- select the managed NIP-28 main channel
- list NIP-28 groups, channels, and recent messages
- send or retry NIP-28 chat messages
- inspect and operate the current internal Forge shared-session lane through
  `autopilotctl forge`, including hosted session discovery, hosted attach, and
  controller handoff commands for the Probe-backed coding shell, with
  no-window Forge-host autostart when no live desktop-control target is
  reachable
- start and stop Buy Mode against the same in-app state used by the GUI
- pull a daily or explicit-window NIP-90 sent-payments report backed by the
  app-owned buyer payment-attempt ledger instead of raw relay rows

`autopilot-compute-mcp` is the model-facing companion surface for the same
desktop-control contract. It speaks MCP over stdio and intentionally sits on
top of the running app's manifest, auth token, and action schema instead of
creating a second hidden compute RPC path.

The current MCP tool surface exposes:

- full compute snapshots and inventory summaries
- provider online/offline requests
- cluster status and topology inspection
- sandbox create/get/upload/start/wait/download operations
- proof and challenge inspection

It does not bypass desktop-control policy or kernel authority. If the desktop
would reject an operation through `autopilotctl`, the MCP layer returns the same
failure through the corresponding tool call.

The desktop control runtime writes and exposes:

- `desktop-control.json` manifest
- `latest.jsonl` session-log alias
- per-session JSONL logs

Those files are the source of truth for programmatic verification because they
prove the UI, the control plane, and the runtime logs stayed in sync.

For NIP-90 payment history, those JSONL files are audit and backfill inputs,
not the primary product read model. The desktop imports recoverable payment
facts from session logs into the app-owned
`~/.openagents/autopilot-nip90-payment-facts-v1.json` ledger with degraded
`log-backfill` provenance, and panes query that ledger instead of reparsing raw
logs on demand. During live desktop operation, that background import
intentionally defers hot session logs and caps the imported byte budget so UI
redraws do not block on large or actively growing JSONL files.

Useful `autopilotctl` starting points:

```bash
autopilotctl status
autopilotctl local-runtime status
autopilotctl local-runtime refresh
autopilotctl apple-fm status
autopilotctl apple-fm list
autopilotctl gpt-oss status
autopilotctl gpt-oss warm --wait
autopilotctl pooled-inference status
autopilotctl pooled-inference topology
autopilotctl pooled-inference export-join-bundle /tmp/mesh-home.join.json --mesh-root /tmp/mesh-home
autopilotctl pooled-inference import-join-bundle /tmp/mesh-home.join.json --mesh-root /tmp/mesh-joiner
autopilotctl wait gpt-oss-ready
autopilotctl attnres status
autopilotctl attnres start
autopilotctl attnres view inference
autopilotctl attnres view loss
autopilotctl attnres sublayer set 4
autopilotctl attnres speed set 5
autopilotctl wait attnres-completed
autopilotctl provider online
autopilotctl chat status
autopilotctl chat messages --tail 20
autopilotctl forge status
autopilotctl forge hosted sessions
autopilotctl forge hosted attach-shared forge-session-1
autopilotctl forge handoff request "taking over triage"
autopilotctl forge handoff accept "accepted from desktop-b"
autopilotctl buy-mode status
autopilotctl nip90-payments daily --date 2026-03-14
autopilotctl nip90-payments window --start 2026-03-14T05:00:00+00:00 --end 2026-03-15T05:00:00+00:00 --json
autopilotctl tailnet status
autopilotctl tunnels status
autopilotctl cluster status
autopilotctl sandbox status
autopilotctl training status
autopilotctl gemma-finetune status
autopilotctl gemma-finetune tenant create design-partner --display-name "Design Partner"
autopilotctl gemma-finetune tenant status --api-key <tenant-api-key>
autopilotctl gemma-finetune project create "Support agent" --tenant-id design-partner --api-key <tenant-api-key> --base-served-artifact-digest sha256:gemma4-e4b-base --hidden-size <hidden-state-width>
autopilotctl gemma-finetune dataset register support-agent-1760000000000 dataset://openagents/support-agent@2026.04 --api-key <tenant-api-key> /tmp/train.jsonl /tmp/validation.jsonl /tmp/holdout.jsonl --baseline-short-path /tmp/baseline-short.jsonl --final-report-path /tmp/final-report.jsonl --chat-template-digest sha256:gemma4-e4b-template --benchmark-ref benchmark://psionic/gemma4/e4b/finetune_eval
autopilotctl gemma-finetune job create support-agent-1760000000000 --api-key <tenant-api-key> --dataset-id support-agent-2026-04-1760000000500
autopilotctl gemma-finetune job get support-agent-1760000000000-1760000000600 --api-key <tenant-api-key>
autopilotctl gemma-finetune job cancel support-agent-1760000000000-1760000000600 --api-key <tenant-api-key>
autopilotctl gemma-finetune promote support-agent-1760000000000-1760000000600 --api-key <tenant-api-key> --checkpoint-id support-agent-r1760000000600-final --reviewer-id operator-1 --review-state approved
autopilotctl remote-training status
autopilotctl remote-training list
autopilotctl remote-training run parameter-golf-runpod-single-h100-live-sample
autopilotctl remote-training stale
autopilotctl remote-training refresh
autopilotctl training launch /tmp/train.jsonl /tmp/held-out.jsonl weather-helper --author "OpenAgents" --description "Repo-native Apple adapter operator run" --license Apache-2.0
autopilotctl training export weather-helper-1760000000000 /tmp/weather-helper.fmadapter
autopilotctl training accept weather-helper-1760000000000
autopilotctl proof status
autopilotctl challenge status
autopilotctl logs --tail 50
autopilotctl perf
autopilotctl perf --json
autopilotctl pane list
autopilotctl pane status provider_control
autopilotctl pane status frame_debugger --json
autopilotctl pane open contributor_beta
autopilotctl pane status contributor_beta --json
autopilotctl pane close provider_control
autopilotctl pane open provider_control
```

`autopilotctl forge` is the first honest programmatic control surface for the
current internal Forge MVP. It does not try to expose every future Forge object
yet. It covers the shared coding-session seam that already exists in the app:

For a narrow user-facing and agent-facing reference focused only on that Forge
CLI, use [`docs/codex/AUTOPILOTCTL_FORGE_CLI.md`](./codex/AUTOPILOTCTL_FORGE_CLI.md).

The repo now also ships a no-window Forge host:

```bash
cargo run -p autopilot-desktop --bin autopilot_headless_forge -- \
  --manifest-path /tmp/openagents-forge-desktop-control.json
```

`autopilotctl forge ...` will autostart that host shape automatically when:

- you did not pass an explicit `--base-url` and `--auth-token`
- the resolved manifest is missing or stale
- no desktop-control target is reachable at the recorded endpoint

If you want the hidden host isolated from a normal desktop session, pass an
explicit `--manifest` path to `autopilotctl` or start
`autopilot_headless_forge` manually with `--manifest-path`.

The repo-owned smoke script for that path is:

```bash
scripts/autopilot/headless-forge-smoke.sh
```

- `autopilotctl forge status [--thread-id <probe-session-id>]`
  - prints the current shared-session/controller state for the active or named
    Probe thread
- `autopilotctl forge hosted sessions`
  - lists visible hosted Forge sessions from the shared shell state
- `autopilotctl forge hosted attach-shared <shared-session-id>`
- `autopilotctl forge hosted attach-probe <probe-session-id>`
  - binds the current desktop to a hosted shared session and loads its Probe
    session through the same attach flow the UI uses
- `autopilotctl forge handoff status [--thread-id <probe-session-id>]`
- `autopilotctl forge handoff request <summary> [--thread-id <probe-session-id>]`
- `autopilotctl forge handoff accept <summary> [--thread-id <probe-session-id>]`
- `autopilotctl forge handoff take <summary> [--thread-id <probe-session-id>]`
- `autopilotctl forge handoff note <summary> [--thread-id <probe-session-id>]`
- `autopilotctl forge handoff human <summary> [--thread-id <probe-session-id>]`
- `autopilotctl forge handoff agent <summary> [--thread-id <probe-session-id>]`
  - all of these operate on the same app-owned participant roster and
    controller-lease truth the desktop shell already projects

The AttnRes command group drives the same persisted Psionic-backed lab state the
desktop pane uses. `autopilotctl attnres status` hydrates the current lab state
from the app-owned controller, and the mutating commands update that same
desktop-owned state machine rather than creating a separate headless AttnRes
runtime.

`autopilotctl perf` is a convenience wrapper over the `frame_debugger` pane
snapshot. It emits the rolling redraw cadence plus grouped and recent timing
data for:

- per-pane paint CPU
- per-runtime background pump work
- desktop-control, provider-admin, and Codex-remote snapshot/signature/sync
  phases

That output is intended for harness automation, so prefer `--json` when you are
collecting or diffing perf runs.

`autopilotctl pane status contributor_beta --json` is the operator and
contributor-facing read path for the bounded external beta. It exposes the
current contributor identity posture, contract and worker-role posture, trust
tier, accepted or rejected or review counts, confirmed and provisional credit
state, review-hold posture, review-queue depth and SLA posture, the current
manual review owner, the Tailnet-first M5 plus NVIDIA pilot roster, the known
external-operator roster, the retained governed-run and XTRAIN and operational
report digests, recent submission state machines, credit dispositions, and the
latest runtime disagreement receipt lineage without creating a separate beta-only
control protocol.

The contributor-beta pane snapshot now also carries the narrow lineage chain
contributors actually need:

- source receipt id
- staging state
- quarantine state
- replay status
- training impact
- held-out impact
- credit disposition and credit reason

That is the bounded answer to "what happened to my submission?" without asking
contributors to read repo code or raw ledgers.

`autopilotctl status` now prints the same app-owned inventory projection summary
the Provider Control pane uses, including the projection source, kernel snapshot
ID when present, per-section product and open-quantity counts, and any current
blocker reason for the local, cluster, or sandbox inventory sections.

It also prints the app-owned buyer procurement summary for compute RFQs and
quotes, including the active quote mode, selected quote IDs, and the quoted
backend, topology, proof posture, environment ref, and sandbox profile where
those fields are present.

`autopilotctl pooled-inference status` is the first dedicated operator view for
the Psionic mesh-backed pooled-inference lane. Set
`OPENAGENTS_PSIONIC_MESH_MANAGEMENT_BASE_URL` to the Psionic management host,
then use the command to inspect the local machine's current pooled-inference
posture: whether this node is serving locally, standing by, or proxying into
the pool, which models are currently warm and targetable, how many warm
replicas exist across the visible mesh, and which reasons are currently driving
the local contribution posture. `autopilotctl pooled-inference topology`
extends that same view with the visible member list and per-node warm-model
inventory.

That same pooled-inference status now feeds the desktop provider inventory and
kernel-market launch product identity. When the mesh is configured, the cluster
inventory section no longer shows a placeholder. Instead it projects the two
current cluster inference product lanes:

- `psionic.cluster.inference.gpt_oss.remote_whole_request`
- `psionic.cluster.inference.gpt_oss.replicated`

Those product IDs are published with explicit `clustered_inference`
execution-kind truth plus `remote_whole_request` or `replicated` topology and
`cluster_attached` provisioning. The current market binding is still
`gpt_oss`-family product identity, but the capability summary and lot metadata
carry the live pooled-inference mesh state: membership posture, targetable
model count, warm replica count, default model, and topology digest.

The inventory and active-job surfaces now also spell out the revenue rule for
each lane instead of treating pooled inference as a generic "cluster" badge:

- `psionic.local.inference.*.single_node` earns when a local delivery is
  accepted and wallet settlement is confirmed.
- `psionic.cluster.inference.gpt_oss.remote_whole_request` earns when the mesh
  serves the whole request and the clustered delivery proof is accepted.
- `psionic.cluster.inference.gpt_oss.replicated` publishes standby capacity,
  but warm replicas alone do not earn. Revenue starts only when a reserve
  window actually sells or the replica is promoted into accepted clustered
  serving.
- sharded pooled-inference lanes are still future scope and are not marketed as
  active wallet-settled products yet.

When an active job or a persisted history row already carries pooled product
identity, the desktop now preserves the same `compute_product_id`,
`market_receipt_class`, and earnings summary through the live status view,
kernel receipt tags, and authoritative history rehydrate. `autopilotctl status`
and `autopilotctl pooled-inference status` therefore tell the operator whether
the current contribution is direct serving revenue, standby capacity that has
not sold yet, or a local single-node delivery path.

The same command group now also owns the multi-machine join and invite path
above Psionic mesh-lane truth:

- `autopilotctl pooled-inference export-join-bundle <output-path>`
- `autopilotctl pooled-inference import-join-bundle <input-path>`

Those commands shell out to `psionic-mesh-lane`, read or update the file-backed
lane root, and then project the resulting join state back through the normal
desktop-control snapshot. The desktop and CLI keep Psionic as the source of
truth for:

- the owned pooled-inference service binary and management surface; this path
  does not depend on a separate external mesh sidecar runtime
- whether the local lane is still `standalone` or has `joined`
- the last selected mesh preference, including label, namespace, cluster id,
  trust digest, and advertised control-plane addresses
- the last imported join bundle, including admission kind, trust posture,
  discovery posture, trust-bundle version, and export/import timestamps

Configure the lane root with either:

- `--mesh-root /absolute/path/to/mesh-root`
- `OPENAGENTS_PSIONIC_MESH_LANE_ROOT=/absolute/path/to/mesh-root`

If `autopilotctl` cannot find a colocated `psionic-mesh-lane` binary, point it
at one explicitly with:

- `OPENAGENTS_PSIONIC_MESH_LANE_BIN=/absolute/path/to/psionic-mesh-lane`

Otherwise it first looks for a sibling binary beside the current desktop
artifacts and then falls back to running the sibling `../psionic` checkout with
`cargo run -p psionic-serve --bin psionic-mesh-lane -- ...`.

Join-bundle export uses the mesh lane's configured service port and derives
advertised control-plane addresses from the current Tailnet self-device IPs
when you do not pass explicit `--advertise <ip:port>` values. That keeps the
common "share this machine with another machine on my Tailnet" path short:

```bash
export OPENAGENTS_PSIONIC_MESH_LANE_ROOT=/tmp/mesh-home
autopilotctl pooled-inference export-join-bundle /tmp/mesh-home.join.json
```

If you need to publish a different address set, override it directly:

```bash
autopilotctl pooled-inference export-join-bundle /tmp/mesh-home.join.json \
  --mesh-root /tmp/mesh-home \
  --mesh-label mesh-home \
  --advertise 100.90.1.10:47470 \
  --advertise 100.90.1.11:47470
```

Import records the join bundle into the target lane root by running
`psionic-mesh-lane install --join-bundle ...`. If the local pooled-inference
lane is already running, the file-backed join state may require a lane restart
before the management host reflects the new posture. `autopilotctl` reports
that explicitly through `restart_required` and `restart_hint` so operators do
not mistake "bundle recorded" for "running process already reloaded."

```bash
export OPENAGENTS_PSIONIC_MESH_LANE_ROOT=/tmp/mesh-joiner
autopilotctl pooled-inference import-join-bundle /tmp/mesh-home.join.json
autopilotctl pooled-inference status
```

`autopilotctl tailnet status` is the focused operator view for the same Tailnet
roster now surfaced in the desktop `Tailnet Status` pane. It shells out to
`tailscale status --json`, normalizes the local node plus peer devices into the
desktop-control snapshot, and prints the full relay, IP, traffic, and
last-seen fields without requiring a second remote-only control path. The pane
itself is the concise discovered-device roster, not the full raw dump.

`autopilotctl training status` now surfaces the app-owned training operator
projection sourced from kernel authority and the current desktop runtime. The
payload distinguishes lightweight control-plane orchestration from heavy
artifact-plane staging and includes run counts, accepted outcomes, environment
versions, checkpoint refs, stale-rollout and duplicate-handling counters,
validator verdict totals, sandbox pool readiness, and the currently visible
training runs or participants when those surfaces are available.

The same status surface now also projects the first decentralized adapter
training read model. It includes:

- contributor readiness and inventory truth for the local `adapter_training`
  contributor lane
- the latest projected adapter windows, including window status, upload counts,
  accepted or quarantined or replay-required contribution totals, validation
  gates, and promotion readiness
- projected contribution outcomes, including contributor node, validator
  disposition, aggregation eligibility, and payout-relevant settlement state
- a contributor-focused summary for the current desktop node so operators can
  see whether the local runtime is blocked, awaiting assignment, submitted,
  accepted, quarantined, replay-required, or settlement-ready

Desktop control now also carries a separate bounded `gemma_finetune` surface
for the first `gemma4:e4b` adapter-SFT MVP. This is intentionally narrower than
the Apple adapter operator and narrower than a general finetuning platform:

- it is a design-partner and operator-facing prep surface for the bounded
  CUDA-only `gemma4:e4b` lane
- it now has an app-owned tenant registry with one provisioned API key per
  design partner, explicit quota state, and fail-closed auth on tenant-scoped
  project, dataset, job, and promotion actions
- it records project identity, tenant identity, served-base binding, and the
  frozen Psionic training-family and eval-pack contract the project targets
- it records explicit `train`, `held_out_validation`, `baseline_short`, and
  `final_report` split identity before a job exists
- it surfaces tokenizer/template compatibility, assistant-mask posture, overlap
  review refs, and validation receipts through the same authenticated snapshot
  contract as the desktop shell
- it projects accepted finetune outcomes and customer-visible promoted-model
  inventory rows once a checkpoint clears the bounded promotion gate

This surface does not yet claim broad upload or raw conversation ingestion.
Today the bounded lane expects preprocessed hidden-state supervision files in
the Psionic `GemmaE4bLmHeadSupervisionSample` shape, provided either as JSON
arrays or JSONL. The operator must declare train, validation, and holdout files
explicitly, and the desktop-control layer truthfully rejects template drift
before any training job is created.

The new desktop-control actions are:

- `gemma-finetune-status`
  - returns the operator aggregate view: tenant quota rows, current project and
    dataset state, visible validation receipts, jobs, accepted-outcome rows,
    and promoted-model inventory
- `gemma-finetune-tenant-provision`
  - creates one bounded design-partner tenant record and returns the single
    API key used for tenant-scoped Gemma actions
- `gemma-finetune-tenant-view`
  - returns the authenticated tenant-scoped read model, including quota usage,
    projects, jobs, accepted outcomes, and published inventory rows
- `gemma-finetune-project-create`
  - binds one design-partner project to the bounded `gemma4:e4b` training lane
    and its canonical eval pack, but only when the supplied tenant API key is
    valid for that tenant and the bounded quota is still open
- `gemma-finetune-dataset-register`
  - registers explicit split files, computes split digests and counts, and
    records a validation receipt that captures tokenizer/template compatibility
    plus overlap-review posture under the authenticated tenant
- `gemma-finetune-job-create`
  - queues one bounded async Gemma job against the admitted dataset bound to a
    project, but fails closed if the tenant or the single-lane bounded runtime
    is already saturated
- `gemma-finetune-job-get`
  - returns per-job state, recent events, checkpoints, exported artifacts, and
    the current promotion record if one exists, scoped by the tenant API key
- `gemma-finetune-job-cancel`
  - requests cancellation and leaves the job in explicit `cancel_requested` or
    terminal `cancelled` truth instead of hiding the operator intent
- `gemma-finetune-checkpoint-promote`
  - records the operator review, scores the bounded Psionic promotion decision,
    and either publishes a discoverable promoted-model ref plus accepted-outcome
    and model-inventory truth or leaves the job in explicit `hold_for_review` /
    `reject` posture

`autopilotctl status` now prints a top-level `gemma finetune:` line alongside
the broader desktop snapshot, and `autopilotctl gemma-finetune status` expands
that into per-tenant quota, per-project, per-dataset, per-job, accepted-outcome,
and inventory detail. `autopilotctl gemma-finetune tenant status --api-key ...`
returns the same read model but scoped to one design partner. The job getter
prints recent events plus checkpoint and artifact rows. The create, register,
queue, cancel, and promote commands all operate against the same state the
desktop keeps on disk under
`~/.openagents/logs/autopilot/gemma-finetune.json`.

The current promotion path is intentionally narrow. OpenAgents now records the
real Psionic baseline sweep, trainer progress, checkpoint digests, exported
adapter digests, and operator review packet. The public Psionic scorer is not
yet exported through this repo, so the current eval receipts are explicitly
derived from the bounded held-out loss lane plus the frozen dataset/template
contract checks. The promotion decision surfaces that truth directly instead of
pretending the desktop already owns a broader finetune control plane.

Desktop control now also carries a separate `remote_training` mirror for the
Psionic Google and RunPod lanes. That surface is app-owned and provider-neutral:

- it mirrors the Psionic remote-training run index and retained bundles into an
  app-local cache instead of making panes parse provider storage layouts
- it refreshes active runs on a one-second cadence
- it keeps `stale`, `summary_only`, and full-series posture explicit through
  normalized run-list and selected-run detail state
- it surfaces bundle provenance, cached-bundle paths, heartbeat age, and stale
  reasons through the same desktop snapshot contract the GUI uses

The desktop-control action surface now exposes that same mirror directly:

- `remote-training-status` returns the normalized run list, selected run
  summary, refresh cadence, cache paths, provenance source, stale counts, and
  the last live-source error
- `remote-training-run` returns the selected or requested run together with the
  retained visualization bundle when the app has it cached locally
- `remote-training-refresh` forces an immediate mirror refresh before returning
  the normalized status payload

`autopilotctl remote-training` is the operator-facing wrapper over those
actions:

- `status` prints the same top-level remote-training summary the full desktop
  snapshot now includes
- `list` prints every mirrored run with provider, lane, result, series posture,
  heartbeat age, cache state, and stale diagnostics
- `run [run_id]` prints selected-run detail and retained bundle counts,
  freshness, provenance, and source-root information
- `stale` filters the mirrored run list down to stale entries and their
  freshness failure reasons
- `refresh` forces a one-shot sync before printing the same normalized status
  payload

For the Apple adapter lane, the training command group is now an operator
workflow rather than a status-only surface:

- `autopilotctl training launch ...` imports train and held-out datasets, runs
  the current repo-owned Apple operator flow, runs the Rust-native Psionic
  training/export lane to stage an Apple-valid `.fmadapter`, and records local
  held-out plus runtime-smoke results, including the bridge-reported runtime
  compatibility state used during validation
- `autopilotctl training export ...` materializes the staged package at an
  explicit export path without treating export as accepted market truth
- `autopilotctl training accept ...` is the authority boundary: it registers
  environment, benchmark, validator, checkpoint-family, training-policy,
  eval-run, training-run, and accepted-outcome records through kernel
  authority, but only after rerunning a bridge-backed Apple runtime drift
  check against the exported package so runtime or Background Assets changes
  are surfaced explicitly before publication

The status payload now renders the Apple operator stages separately so launched,
evaluated, exported, and accepted states stay distinct across restart or replay.
It also renders decentralized adapter window and contribution lines so scripted
operation no longer requires dropping down to raw kernel API calls to inspect
the latest contributor outcomes.

For the Apple adapter operator specifically, the same status surface now carries
live run telemetry rather than only coarse terminal states. While a run is
active, `autopilotctl training status` renders:

- a `training operator live:` line with the current phase, heartbeat timestamp,
  run and phase elapsed time, ETA, epoch and step counters, held-out eval
  progress, latest observed loss, and the latest checkpoint path
- recent typed `training operator event:` lines for major lifecycle points such
  as training start, epoch start, step completion, held-out eval samples,
  export, runtime smoke, and acceptance

`autopilotctl training launch ...` now returns immediately after the app-owned
operator run is created. The long-running train/eval/export/runtime-smoke work
continues in the background, and the status/event surfaces above are the
supported way to follow progress without waiting for the original launch command
to exit.

For the Apple adapter lane, that operator status is lifecycle truth, not
benchmark-usefulness truth. The text output now labels authority publication as
`authority_accept` and `authority_outcome`, and it prints an explicit note that
export, runtime smoke, and authority acceptance do not by themselves prove
benchmark-useful adapter quality. The canonical benchmark-useful gate remains
`scripts/release/check-psionic-apple-architecture-explainer-acceptance.sh`.
The latest live acceptance receipt on 2026-03-16 passed only the weak overfit
stage (`520` score bps, `1428` pass-rate bps, `1` improved case) and still
rejected the standard stage (`571` score bps, `1428` pass-rate bps, `1`
improved case), so do not read `autopilotctl training status` as a substitute
for the acceptance harness.

The packaged Apple-lane release checks now also run
`scripts/release/check-psionic-apple-rust-only-gate.sh` before claiming Apple
training readiness. That gate fails if the shipped Apple operator path regresses
back to toolkit-root discovery, Python-interpreter discovery, or authoritative
toolkit shell-outs.

`autopilotctl proof status` now drills into recent delivery proofs instead of
stopping at a count. The payload includes proof posture, topology and
provisioning labels, linked challenge state, settlement outcomes, and review
refs such as proof-bundle refs, activation fingerprints, validator refs, and
runtime/session identity refs when those are present in kernel truth.

`autopilotctl challenge status` now surfaces the validator history against the
same kernel objects: linked delivery proofs, verdicts, reason codes, challenge
result refs, and the current settlement impact summary for the challenged
delivery.

`autopilotctl nip90-payments daily --date YYYY-MM-DD` interprets the supplied
date as the local calendar day on the machine running the CLI, converts it into
an explicit `[start, end)` epoch window, and asks desktop control for the same
wallet-authoritative NIP-90 send report the app can use elsewhere. The payload
includes exact window start/end timestamps, top-line `payment_count` and
`total_sats_sent`, fees and wallet debit totals, the currently connected relay
URLs considered, and the degraded-binding count for any recovered non-top-line
rows.

Useful MCP starting point:

```bash
cargo run -p autopilot-desktop --bin autopilot-compute-mcp -- --manifest \
  ~/.openagents/autopilot/desktop-control.json
```

Typical MCP clients should launch that stdio server after the desktop app is
already running and the desktop-control manifest exists.

Apple-specific bridge flows still exist for the shipped macOS release path:

```bash
autopilotctl apple-fm status
autopilotctl apple-fm refresh --wait
autopilotctl apple-fm list
autopilotctl apple-fm load /absolute/path/to/adapter.fmadapter --adapter-id fixture-chat-adapter
autopilotctl apple-fm attach sess-1 fixture-chat-adapter
autopilotctl apple-fm detach sess-1
autopilotctl apple-fm unload fixture-chat-adapter
autopilotctl apple-fm smoke-test
```

`autopilotctl apple-fm status` now includes bridge reachability, model readiness,
adapter-inventory support, attach support, loaded adapter count, and the current
active-session adapter if the workbench has one selected. `autopilotctl apple-fm list`
prints the loaded bridge inventory entries with compatibility and attached-session
truth. The mutating `load`, `unload`, `attach`, and `detach` verbs queue the same
Apple FM workbench operations the desktop pane uses, so headless operators and the
GUI stay replay-safe against one app-owned control surface.

Sandbox lifecycle examples:

```bash
autopilotctl sandbox status
autopilotctl sandbox create pythonexec-profile job-1 /tmp/openagents-sandbox \
  --entrypoint-type workspace-file \
  --entrypoint scripts/job.py \
  --expected-output result.txt
autopilotctl sandbox upload job-1 scripts/job.py ./job.py
autopilotctl sandbox start job-1
autopilotctl sandbox wait job-1 --timeout-ms 30000
autopilotctl sandbox job job-1
autopilotctl sandbox download-artifact job-1 result.txt --output /tmp/result.txt
```

## Supported local-runtime hosts

The app now exposes one app-owned `local_runtime` contract across the split
desktop shell, desktop control, and `autopilotctl`, but the supported host
stories are still lane-specific:

- macOS Apple Silicon: Apple FM via `foundation-bridge`
- supported non-macOS NVIDIA hosts: GPT-OSS via the in-process Psionic CUDA lane
- retained GPT-OSS Metal/CPU backends can still appear in status/readiness
  views, but `Go Online` currently unlocks sell-compute only for CUDA

Provider Control now renders the active local-runtime lane inline. On supported
NVIDIA/CUDA hosts that means the local-runtime area can show GPT-OSS readiness,
artifact state, load state, model path, and `REFRESH` / `WARM` / `UNLOAD`
actions directly, while the separate GPT-OSS workbench remains the prompt
playground and detailed model-management pane.

## GPT-OSS host bring-up

Use this on a supported non-macOS NVIDIA/CUDA host.

The GPT-OSS runtime reads:

- `OPENAGENTS_GPT_OSS_BACKEND=auto|cuda|metal|cpu`
- `OPENAGENTS_GPT_OSS_MODEL_PATH=/path/to/gpt-oss-20b-mxfp4.gguf`

If `OPENAGENTS_GPT_OSS_MODEL_PATH` is unset, the runtime defaults to:

```text
~/models/gpt-oss/gpt-oss-20b-mxfp4.gguf
```

Recommended bring-up flow:

```bash
export OPENAGENTS_GPT_OSS_BACKEND=cuda
export OPENAGENTS_GPT_OSS_MODEL_PATH=/absolute/path/to/gpt-oss-20b-mxfp4.gguf

cargo install --path .
cargo autopilot

autopilotctl local-runtime status
autopilotctl local-runtime refresh
autopilotctl gpt-oss status
autopilotctl gpt-oss warm --wait
autopilotctl wait local-runtime-ready
autopilotctl wait gpt-oss-ready
autopilotctl provider online
```

Useful follow-up checks:

```bash
autopilotctl gpt-oss unload --wait
autopilotctl logs --tail 100
```

Repeatable scripted form:

```bash
scripts/release/check-gpt-oss-nvidia-mission-control.sh
```

Canonical seller-lane perf harness:

```bash
scripts/release/check-gpt-oss-nvidia-seller-lane-perf.sh
```

That harness expects a local standalone Psionic checkout at `../psionic` by
default. Override it with `OPENAGENTS_PSIONIC_REPO=/absolute/path/to/psionic`
if your checkout lives elsewhere.

That harness launches the desktop app with the configured CUDA GGUF, captures
the cold app-owned local-runtime preflight snapshot, runs the canonical
Psionic-only CUDA benchmark pass, warms the seller lane through `autopilotctl`,
and writes a machine-readable artifact at:

```text
target/gpt-oss-nvidia-seller-lane-perf/seller-lane-perf.json
```

The run directory also preserves raw desktop-control snapshots, benchmark case
summaries, app logs, and a short `summary.txt`. Set
`OPENAGENTS_GPT_OSS_NVIDIA_BASELINE_ARTIFACT=/absolute/path/to/prior/seller-lane-perf.json`
to include a baseline/regression comparison section in the new artifact.

Cross-stack launch validation:

```bash
scripts/release/check-compute-launch-program.sh
```

The launch-program harness shells into the standalone Psionic repo for
Psionic-owned test legs. By default it expects that checkout at `../psionic`;
override it with `OPENAGENTS_PSIONIC_REPO=/absolute/path/to/psionic`.

That launch-program harness writes a summary plus per-step logs for the desktop
control plane, Psionic sandbox and cluster lanes, validator service, kernel
authority compute flows, and optional funded/platform-specific legs. See
`docs/COMPUTE_LAUNCH_PROGRAM_RUNBOOK.md`.

Operational notes:

- `local-runtime refresh` always targets the active Provider Control lane, but
  on GPT-OSS it does not load the GGUF by itself
- `autopilotctl local-runtime status` now surfaces seller-lane execution
  posture (`cold`, `warming`, `warm`, `compile_failed`, `cache_invalidated`),
  scheduler posture, last compile-path temperature, execution-plan/kernel-cache
  occupancy, selected device inventory, last cold-compile or warm-refresh
  duration, typed cache invalidation reason, and the last compile-failure
  summary when one exists
- `gpt-oss warm` and `gpt-oss unload` act directly on the configured GGUF model
- `local-runtime refresh` re-reads `OPENAGENTS_GPT_OSS_BACKEND` and
  `OPENAGENTS_GPT_OSS_MODEL_PATH`; if the backend, configured model, or local
  artifact metadata changed, the runtime invalidates prior warm execution state
  and reports the typed invalidation reason through desktop control and
  `autopilotctl`
- `provider online` will still block if the backend is not `cuda`, the GGUF is
  missing, or the configured model is not loaded
- on retained Metal/CPU GPT-OSS hosts, Provider Control and `autopilotctl` stay
  truthful about runtime state but currently point you back to the GPT-OSS
  workbench instead of unlocking sell-compute

## Local smoke run

This uses the current default buyer wallet and creates a fresh provider account under `target/headless-compute-smoke/provider`:

```bash
scripts/autopilot/headless-compute-smoke.sh
```

Useful env overrides:

- `OPENAGENTS_HEADLESS_PROVIDER_BACKEND=auto|apple-fm|canned`
- `OPENAGENTS_HEADLESS_MAX_REQUESTS=1`
- `OPENAGENTS_HEADLESS_BUDGET_SATS=2`
- `OPENAGENTS_HEADLESS_BUYER_HOME=/path/to/funded-home`
- `OPENAGENTS_HEADLESS_RUN_DIR=/path/to/run-dir`
- `OPENAGENTS_HEADLESS_PROVIDER_HOME=/path/to/provider-home`
- `OPENAGENTS_SPARK_NETWORK=mainnet|regtest`

The smoke script now performs a funding preflight with `spark-wallet-cli`
before it boots the relay/provider pair. If the default buyer wallet does not
have at least the requested budget on the selected Spark network, the script
fails early and tells you which `HOME` it inspected.

## Multi-payment roundtrip

This runs multiple paid requests from the default wallet into a fresh provider wallet,
then flips the roles and spends the earned sats back the other way:

```bash
scripts/autopilot/headless-compute-roundtrip.sh
```

Useful env overrides:

- `OPENAGENTS_HEADLESS_FORWARD_COUNT=6`
- `OPENAGENTS_HEADLESS_REVERSE_COUNT=3`
- `OPENAGENTS_HEADLESS_INTERVAL_SECONDS=8`
- `OPENAGENTS_HEADLESS_TIMEOUT_SECONDS=75`
- `OPENAGENTS_HEADLESS_PROVIDER_BACKEND=canned|auto|apple-fm`
- `OPENAGENTS_HEADLESS_BUYER_HOME=/path/to/funded-home`
- `OPENAGENTS_HEADLESS_RUN_DIR=/path/to/run-dir`
- `OPENAGENTS_SPARK_NETWORK=mainnet|regtest`

The roundtrip smoke script defaults to the deterministic `canned` backend so the payment path stays
stable even on machines without Apple Foundation Models. It still uses real NIP-90 requests,
real Spark invoices, and real Lightning settlement.

The forward leg spends from the buyer wallet selected by `OPENAGENTS_HEADLESS_BUYER_HOME`
(or the current shell `HOME` if unset). The script now checks that wallet up front and
fails early if it cannot cover the requested forward leg on the selected Spark network.

`OPENAGENTS_HEADLESS_REVERSE_COUNT` is treated as a ceiling, not a guarantee. After the forward leg,
the script measures the actual sats burned per send from the default wallet and trims the reverse leg
to what the fresh secondary wallet can really afford under the current Lightning fee conditions.

The script emits:

- `summary.txt` human summary
- `summary.json` machine-readable request/payment report
- requested vs executed reverse job counts
- per-phase buyer/provider logs
- Spark status snapshots before, between, and after the two phases

## Packaged app smoke run

This launches the real bundled `Autopilot.app`, points it at a deterministic local relay,
drives it through `autopilotctl`, and verifies the production shell completes the provider
side of the paid loop all the way through settlement:

```bash
scripts/release/check-v01-packaged-compute.sh
```

What it does:

- builds `Autopilot.app`, `autopilotctl`, `autopilot-headless-compute`, and `spark-wallet-cli`
- bundles `FoundationBridge.app` into `Autopilot.app/Contents/Helpers`
- launches the packaged app executable with isolated `HOME` and `OPENAGENTS_AUTOPILOT_LOG_DIR`
- configures the bundle against a local deterministic relay via its settings file
- brings the provider online through `autopilotctl`
- starts a controlled headless buyer targeted to the packaged provider
- asserts on the bundled app's `latest.jsonl` and per-session JSONL logs:
  - request accepted
  - request running
  - request delivered
  - `provider.result_published`
  - `provider.payment_requested`
  - `provider.settlement_confirmed`

Useful env overrides:

- `OPENAGENTS_PACKAGED_RUN_DIR=/path/to/run-dir`
- `OPENAGENTS_PACKAGED_FUNDER_HOME=/path/to/funded-home`
- `OPENAGENTS_PACKAGED_FUNDER_IDENTITY_PATH=/path/to/funded/identity.mnemonic`
- `OPENAGENTS_PACKAGED_BUYER_FUNDING_SATS=50`
- `OPENAGENTS_PACKAGED_BUDGET_SATS=2`
- `OPENAGENTS_PACKAGED_SKIP_BUILD=1`
- `OPENAGENTS_SPARK_NETWORK=mainnet|regtest`

The packaged smoke script is intentionally app-owned verification, not a library-only harness.
It proves the production shell, desktop control runtime, and file-backed logs stay in sync
through the v0.1 paid compute loop.

Before generating funding invoices, the script now checks the configured funder wallet
and fails early if `OPENAGENTS_PACKAGED_FUNDER_HOME` / `OPENAGENTS_PACKAGED_FUNDER_IDENTITY_PATH`
do not point at a wallet that can cover the buyer seed amount on the selected Spark network.

The packaged `.app` verification flow is still the macOS Apple FM release path.
There is not yet a separate packaged GPT-OSS bundle check in this repo. For the
supported NVIDIA/CUDA lane, current operator verification is the running desktop
app plus `autopilotctl`, desktop-control snapshots, and the session JSONL logs.

## Packaged app buyer + seller + chat roundtrip

This is the stronger packaged verification path for the current release cut:

```bash
scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh
```

What it does:

- builds the bundled `Autopilot.app`
- launches both a bundled app and a separate runtime app
- drives both apps entirely through `autopilotctl`
- selects the managed NIP-28 main channel in both apps
- verifies bidirectional NIP-28 chat
- brings both providers online
- funds Spark wallets as needed
- runs paid buyer/seller flows in both directions
- asserts on desktop-control snapshots plus `latest.jsonl` / session logs for:
  - NIP-28 presence and message delivery
  - targeted NIP-90 request dispatch
  - buyer payment settlement
  - provider settlement confirmation
  - `snapshot.session.shell_mode=hotbar`
  - `snapshot.session.dev_mode_enabled=false`

Useful env overrides:

- `OPENAGENTS_AUTOPILOTCTL_RUN_DIR=/path/to/run-dir`
- `OPENAGENTS_AUTOPILOTCTL_FUNDER_HOME=/path/to/funded-home`
- `OPENAGENTS_AUTOPILOTCTL_FUNDER_IDENTITY_PATH=/path/to/funded/identity.mnemonic`
- `OPENAGENTS_AUTOPILOTCTL_FUND_SATS=100`
- `OPENAGENTS_AUTOPILOTCTL_BUDGET_SATS=2`
- `OPENAGENTS_AUTOPILOTCTL_SKIP_BUILD=1`
- `OPENAGENTS_SPARK_NETWORK=mainnet|regtest`

This is the closest thing in the repo to a production-shell end-to-end test.
Use it when the question is not "does the library path work?" but "does the
actual app bundle work when steered programmatically?"

The roundtrip script also preflights the configured funder wallet and will stop
before app launch if it cannot cover the bundle/runtime seed amounts. That keeps
funding failures distinct from split-shell or desktop-control regressions.

## Separate processes

Run a local relay:

```bash
cargo run -p autopilot-desktop --bin autopilot-headless-compute -- relay --listen 127.0.0.1:18490
```

Run a provider on a separate identity/wallet:

```bash
cargo run -p autopilot-desktop --bin autopilot-headless-compute -- provider \
  --relay ws://127.0.0.1:18490 \
  --identity-path ~/.openagents/headless-provider/identity.mnemonic \
  --backend auto
```

Run a buyer with the current default wallet:

```bash
cargo run -p autopilot-desktop --bin autopilot-headless-compute -- buyer \
  --relay ws://127.0.0.1:18490 \
  --max-settled-requests 1 \
  --fail-fast
```

Targeting a specific provider on a shared relay:

```bash
cargo run -p autopilot-desktop --bin autopilot-headless-compute -- buyer \
  --relay wss://your-relay.example \
  --target-provider-pubkey <provider-npub-or-hex>
```

## NIP-28 managed chat channels

The desktop app bootstraps from the configured default channel, then keeps the
live NIP-28 subscription set aligned with the managed-chat projection. Runtime
subscription scope is all discovered managed-chat channel ids currently present
in the projection, and the lane subscribes across the app's configured relay
set instead of a single relay.

### Default channel

The primary channel is set by:

- `OA_DEFAULT_NIP28_RELAY_URL` — relay WebSocket URL (default: `wss://relay.damus.io`)
- `OA_DEFAULT_NIP28_CHANNEL_ID` — 64-char hex kind-40 event ID (default: `ebf2e35092632ecb81b0f7da7d3b25b4c1b0e8e7eb98d7d766ef584e9edd68c8`)

### Team test channel (A-5)

A second channel for team testing can still be seeded without touching the
default:

- `OA_NIP28_TEAM_CHANNEL_ID` — 64-char hex kind-40 event ID of the team test channel

When set, the lane worker bootstraps with both channel ids available to the
projection. Both channels appear in the managed chat workspace rail, and the
runtime will continue subscribing to any additional managed-chat channels the
projection discovers later.

**Creating a team channel (one-time, manual):**

```bash
# Using nak (https://github.com/fiatjaf/nak)
nak event --sec <team-nsec> --kind 40 \
  --content '{"name":"oa-team-chat","about":"OpenAgents team test channel","picture":""}' \
  wss://relay.damus.io
# Record the output event ID (64 hex chars) — that is OA_NIP28_TEAM_CHANNEL_ID
```

**Running the app with the team channel:**

```bash
export OA_NIP28_TEAM_CHANNEL_ID=<64-hex-id>
cargo autopilot
```
