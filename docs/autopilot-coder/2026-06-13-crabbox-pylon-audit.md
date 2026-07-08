# Crabbox to Pylon Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-13

External reference read: `projects/repos/crabbox` (workspace reference clone of `openclaw/crabbox`)

## 1. Overview of crabbox

Crabbox is a Go remote software testing and execution control plane for
maintainers and AI agents. Its CLI is implemented under `cmd/crabbox` and
`internal/cli`; provider adapters live under `internal/providers`; the optional
coordinator is TypeScript under `worker/src` with a Cloudflare Durable Object
runtime and a Node.js/PostgreSQL runtime under `worker/node`.

The project description in `README.md` is accurate to the implementation shape I
read: the CLI can lease or reuse capacity, sync a dirty checkout, run a command
remotely, stream output, retain logs/events/evidence, and release the box. The
coordinator owns shared-provider credentials, lease state, expiry, cleanup,
usage, spend caps, run records, logs, events, telemetry, portal views, and
artifact upload grants. The data plane for SSH-backed runs stays CLI-to-runner:
SSH readiness, rsync, remote command execution, and normal output streaming do
not traverse the coordinator.

License: MIT, per `LICENSE`. The Go module is `github.com/openclaw/crabbox`
with `go 1.26` in `go.mod`. The Worker package is TypeScript ESM and reports
version `0.28.0` in `worker/package.json`.

## 2. Capability map

### Lease and warm

Crabbox models a box as a lease with a canonical `cbx_...` ID and a friendly
slug. `crabbox warmup` creates a reusable lease; `crabbox run` can create a
fresh one-shot lease or reuse an existing ID/slug. The key implementation is in
`internal/cli/run.go`, `internal/cli/lease.go`, `internal/cli/slug.go`,
`internal/cli/provider_backend.go`, and `worker/src/fleet.ts`.

The brokered path supports AWS, Azure, GCP, and Hetzner through the coordinator.
Direct providers include static SSH hosts, local containers, Apple VZ, Proxmox,
Parallels, XCP-ng, Incus, DigitalOcean, Linode, Namespace Devbox, Semaphore,
Sprites, Daytona, Morph, RunPod, Tenki, and others. Delegated-run providers
include Cloudflare, Docker Sandbox, E2B, Freestyle, Islo, Modal, OpenComputer,
OpenSandbox, Railway, Anthropic Sandbox Runtime, SmolVM, Tensorlake, Upstash
Box, Azure Dynamic Sessions, Blacksmith Testbox, W&B, and Windows Sandbox. The
registered-provider mode lets direct SSH providers publish inventory, sharing,
and WebVNC metadata to the coordinator without giving it cleanup authority or
provider credentials. See `README.md`, `docs/features/providers.md`,
`docs/features/coordinator.md`, and `internal/providers/all/all.go`.

### Sync diff

For SSH leases, Crabbox syncs the Git-managed working set rather than the whole
tree. The manifest comes from `git ls-files --cached --others
--exclude-standard -z`, filtered by built-in excludes, repo config, and
`.crabboxignore`. It computes local and remote fingerprints and skips rsync when
nothing changed. It can seed the remote Git tree from origin so rsync sends only
the local delta, can full-resync, and guards against large accidental transfers
and mass deletions. Native Windows uses an archive sync path instead of rsync.
The source is `internal/cli/repo.go`, `internal/cli/run.go`,
`internal/cli/ssh.go`, `internal/cli/sync_windows_target.go`,
`internal/cli/sync_archive.go`, and `docs/features/sync.md`.

### Remote run

Remote execution is built around `SSHTarget`, SSH readiness probes, per-lease
keys, fallback ports, optional ProxyCommand/SSH config proxy support, and remote
command wrapping for POSIX and Windows targets. The main flow in
`internal/cli/run.go` performs lease acquisition/resolution, optional Actions
hydration, sync, script upload, environment helper setup, preflight probes, the
command over SSH, post-run downloads/artifacts, failure classification, and
cleanup. SSH helpers are in `internal/cli/ssh.go` and `internal/cli/ssh_cmd.go`.

Delegated-run providers implement `DelegatedRunBackend` from
`internal/cli/provider_backend.go`; those providers own sync and execution, and
Crabbox rejects SSH-specific sync flags unless the provider advertises the
needed feature.

### Stream

During brokered SSH-backed runs, Crabbox creates a run record early and appends
ordered run events while the CLI executes directly against the runner. It mirrors
stdout/stderr through bounded event writers and stores a retained command log at
finish. The implementation lives in `internal/cli/run_recorder.go`,
`internal/cli/run_output_events.go`, `internal/cli/control_ws.go`,
`worker/src/fleet.ts`, and `docs/features/history-logs.md`.

### Evidence

Evidence exists at several levels:

- Run records, phase events, retained logs, timings, JUnit summaries, failure
  classification, and telemetry in `internal/cli/run_recorder.go`,
  `internal/cli/results*.go`, `internal/cli/telemetry.go`,
  `internal/cli/timing.go`, and `worker/src/fleet.ts`.
- Local failure captures and stdout/stderr capture paths in
  `internal/cli/run_capture.go` and `docs/features/history-logs.md`.
- Run-scoped artifact glob collection and required artifact checks in
  `internal/cli/run_artifacts.go`.
- Desktop/QA bundles with metadata, screenshots, doctor output, retained run
  logs, video, contact sheets, GIFs, publishing, manifests, list, and pull in
  `internal/cli/artifacts*.go`, `internal/cli/media.go`,
  `worker/src/artifacts.ts`, and `docs/features/artifacts.md`.

### Release

Crabbox treats release as part of the run lifecycle. Fresh one-shot leases are
released unless `--keep`, `--keep-on-failure`, pool-return rules, or stop policy
say otherwise. Warm leases can be stopped explicitly. Brokered release deletes
managed provider resources; registered release only removes the coordinator
registration. Expiry and cleanup are driven by Durable Object alarms or
Node/pg-boss jobs, with provider labels/tags used for reconciliation. Relevant
files include `internal/cli/run.go`, `internal/cli/status.go`,
`internal/cli/pool.go`, `internal/cli/provider_labels.go`,
`worker/src/fleet.ts`, `worker/src/provider-labels.ts`, and
`docs/features/lifecycle-cleanup.md`.

### Providers

Provider shape is explicit. `internal/cli/provider_backend.go` defines
`ProviderSpec`, `ProviderKind` (`ssh-lease`, `delegated-run`,
`service-control`), `CoordinatorMode`, `FeatureSet`, `SSHLeaseBackend`,
`DelegatedRunBackend`, optional artifact, port, copy, cleanup, pause/resume, and
checkpoint interfaces. `internal/providers/all/all.go` registers built-ins by
side effect. The external executable protocol in `internal/providers/external`
is notable because it lets a provider remain outside the Crabbox codebase while
still returning a normal SSH target.

## 3. How it maps to Pylon today

Pylon already has the beginnings of an execution-control architecture, but it is
local-first and assignment/proof focused.

`apps/pylon/scripts/multi-session-run.ts` reads a JSON plan, materializes one
workspace per session, runs `dev-proof-run.ts` in bounded parallel child
processes, emits redaction-scanned heartbeats, writes refs-only outcomes, and
now includes ordered account failover. It is closest to Crabbox's `job` and
multi-lease orchestration concepts, but today it runs local child processes
rather than leasing remote compute or persisting a brokered run stream.

`apps/pylon/src/node/control-sessions.ts` exposes in-process session spawn/list,
cancel, event history, and SSE streaming. It rejects danger modes, materializes a
workspace, runs Codex or Claude Agent in `local_bounded`, records composer and
dev-check events, and writes redaction-scanned proof or failure artifacts. It is
closest to Crabbox's run-record/event model, but lacks a durable coordinator,
lease/resource ownership, reconnectable attach, and cross-process storage.

`apps/pylon/scripts/dev-proof-run.ts` is the retained proof path. It collects a
doctor projection, executes the same composer path used by the TUI, runs a
focused dev check, and writes a typed, public-safe proof artifact. This maps to
Crabbox's evidence philosophy, but Pylon's current evidence is centered on
agent/proof metadata, not host-level resource telemetry, sync timing, retained
stdout/stderr logs, remote artifacts, or cleanup receipts for remote resources.

`apps/pylon/src/workspace-materializer.ts` validates public GitHub
`git_checkout` assignments, fetches pinned commits, uses a shared bare-repo cache
plus detached worktrees, writes workspace lease records, sweeps TTL-expired
workspaces, and exposes public-safe lease projections. This is a strong local
analogue to Crabbox's sync lease thinking. The gap is dirty-worktree remote
sync: Pylon can materialize a clean pinned commit or accept an injected local
worktree, but it does not yet have Crabbox-style manifest diffing, fingerprint
skip, remote Git seeding, remote workdir metadata, or mass-deletion guardrails.

The new account-quota routing in `apps/pylon/src/account-quota.ts`,
`apps/pylon/src/account-quota-ledger.ts`, and
`apps/pylon/scripts/multi-session-run.ts` is conceptually adjacent to Crabbox's
capacity and cost guardrails. Pylon now detects provider quota text, records a
refs-only quota block, skips unavailable account hashes, and fails over through
an ordered account pool. Crabbox applies the same class of control at the
compute layer: capacity fallback, active-lease caps, monthly reserved-USD caps,
and usage reporting in `worker/src/usage.ts` and `worker/src/fleet.ts`.

## 4. Concrete pull-in candidates

- **CONCEPT: Separate control plane from data plane.** Crabbox's coordinator
  handles auth, provider credentials, lease state, expiry, usage, and evidence,
  while SSH/rsync/command streams go directly from CLI to runner. Pylon should
  keep composer execution data close to the selected runtime and send only typed
  session, lease, quota, artifact, and receipt records to a Pylon coordinator.
  Source: `README.md`, `docs/how-it-works.md`, `docs/features/coordinator.md`,
  `worker/src/fleet.ts`.

- **CONCEPT: Provider-neutral execution backends.** Pylon can define a
  TypeScript/Effect equivalent of Crabbox's `ProviderSpec`, `ProviderKind`, and
  feature flags: local process, local sandbox, SSH lease, delegated sandbox, and
  managed brokered lease. This would keep Codex/Claude session orchestration
  independent from the substrate. Source: `internal/cli/provider_backend.go`,
  `internal/providers/all/all.go`, `docs/features/providers.md`.

- **CODE: Backend interface vocabulary.** The shapes in
  `internal/cli/provider_backend.go` are directly reusable as design patterns:
  `Acquire`, `Resolve`, `Touch`, `ReleaseLease`, `Run`, `Warmup`, `Stop`,
  optional `CollectRunArtifacts`, and capability-gated features. Do not port Go
  code mechanically, but port the interface decomposition into
  `apps/pylon/src` as typed Effect services.

- **CONCEPT: Remote session lease records.** Extend Pylon's workspace lease idea
  to execution leases: `sessionRef`, `runtimeLeaseRef`, provider kind, target,
  state, created/started/completed timestamps, TTL, idle timeout, cleanup policy,
  and cleanup receipt refs. This mirrors Crabbox lease records and Pylon's
  existing local workspace lease records. Source: `worker/src/types.ts`,
  `worker/src/fleet.ts`, `apps/pylon/src/workspace-materializer.ts`.

- **CODE: Dirty checkout manifest and fingerprint algorithm.** Pylon's
  materializer should keep pinned clean checkout semantics for assignment intake,
  but remote execution needs a local dirty-checkout overlay. Crabbox's manifest,
  safe relative path filtering, default excludes, `.crabboxignore` handling,
  changed/deleted path accounting, and sync fingerprint are strong candidates to
  port into TypeScript. Source: `internal/cli/repo.go`,
  `docs/features/sync.md`.

- **CODE: Large-sync and deletion guardrails.** The Pylon remote materializer
  should fail closed when a dirty overlay accidentally includes dependency
  caches, generated output, huge binary trees, or large deletion sets. Crabbox's
  `checkSyncPreflight`, top-directory reporting, dirty-delta scope, and
  mass-deletion sanity concept provide the pattern. Source:
  `internal/cli/repo.go`, `internal/cli/run.go`, `docs/features/sync.md`.

- **CONCEPT: Remote Git seeding plus overlay.** Pylon already fetches pinned
  commits locally. For remote sessions, use the Crabbox model: seed the remote
  workdir from a reachable origin commit, then overlay local dirty tracked and
  nonignored files. This improves latency and makes "agent modifies local
  checkout, verify on fast remote capacity" practical. Source:
  `internal/cli/run.go`, `internal/cli/repo.go`, `docs/features/sync.md`.

- **CODE: SSH target and readiness model.** A Pylon remote runner should copy
  the `SSHTarget` ideas: explicit user, host, key/cert, known hosts file, port,
  fallback ports, target OS, Windows mode, ready check, ControlMaster opt-out,
  and proxy command. The wait loop's TCP/auth/ready distinctions would produce
  better operator diagnostics than a plain command failure. Source:
  `internal/cli/ssh.go`, `internal/cli/ssh_cmd.go`.

- **CONCEPT: Early run handles and attachable streams.** Pylon control sessions
  already emit SSE events in memory. Adopt Crabbox's durable `run_...` handle
  before leasing/materialization, append ordered events, and support attach by
  run/session ID after process restart. Source:
  `internal/cli/run_recorder.go`, `internal/cli/run_output_events.go`,
  `internal/cli/control_ws.go`, `docs/features/history-logs.md`,
  `worker/src/fleet.ts`.

- **CODE: Bounded stream/event retention.** Crabbox distinguishes live event
  chunks from retained logs, caps stream events, stores larger retained logs, and
  emits truncation markers. Pylon should use the same distinction so agent output
  stays inspectable without leaking unlimited raw terminal text into public
  projections. Source: `internal/cli/run_output_events.go`,
  `internal/cli/run_recorder.go`, `docs/features/history-logs.md`.

- **CONCEPT: Evidence bundles beyond proof JSON.** Pylon proofs should be able
  to reference run logs, verification artifacts, screenshots, videos, contact
  sheets, and manifests as evidence refs rather than embedding raw data. Crabbox
  has a clear artifact bundle shape with metadata, files, publish manifests, and
  pull-time SHA256/size verification. Source: `internal/cli/artifacts.go`,
  `internal/cli/artifacts_manifest.go`, `internal/cli/artifacts_publish.go`,
  `docs/features/artifacts.md`.

- **CODE: Required-artifact gates.** Pylon verification commands could require a
  report, patch summary, test output, or screenshot manifest before a session can
  close out. Crabbox's safe relative glob validation, remote existence check,
  and artifact tarball collection are portable patterns. Source:
  `internal/cli/run_artifacts.go`, `docs/features/artifacts.md`.

- **CONCEPT: Timing schema and phase markers.** Pylon should add stable timing
  for materialization, lease, sync, composer, command/dev-check, artifact
  capture, and cleanup. Crabbox's `--timing-json` and `CRABBOX_PHASE:<name>`
  markers would map well to multi-session diagnosis. Source:
  `internal/cli/timing.go`, `internal/cli/run_phase.go`,
  `docs/features/history-logs.md`.

- **CONCEPT: Capacity/cost guardrails as the compute analogue of account
  routing.** Pylon's account-quota ledger handles model-account exhaustion. A
  remote execution layer needs analogous compute budgets: active sessions per
  owner/org, max remote leases, reserved runtime cost, idle timeout, TTL, and
  provider fallback attempts. Source: `worker/src/usage.ts`,
  `worker/src/fleet.ts`, `docs/features/cost-usage.md`,
  `docs/features/capacity-fallback.md`.

- **CODE: Registered external/BYO host path.** Pylon does not need to own every
  provider at first. Crabbox's static SSH and external executable provider show
  how to accept an existing host or an out-of-process lifecycle adapter while
  still using one sync/run/evidence path. Source: `docs/providers/ssh.md`,
  `docs/providers/external.md`, `internal/providers/ssh`,
  `internal/providers/external/protocol.go`,
  `internal/providers/external/lifecycle.go`.

- **CONCEPT: Warm pools and reusable sessions.** Crabbox's `warmup`, `prewarm`,
  ready-pool borrow/return, and job stop policies are relevant to Pylon
  overnight or multi-session runs. Pylon can pre-materialize workspaces or warm
  remote runners, then assign sessions with explicit return policies. Source:
  `internal/cli/run.go`, `internal/cli/prewarm.go`, `internal/cli/ready_pool.go`,
  `docs/features/jobs.md`.

- **CONCEPT: Pond-style grouped orchestration.** Pylon multi-session currently
  manages independent local sessions. Crabbox's pond labels, slugs, peer
  discovery, exposed ports, and bulk release are useful for multi-agent
  integration tasks where sessions need stable roles, shared service endpoints,
  or cleanup as a group. Source: `internal/cli/pond*.go`,
  `docs/features/pond.md`.

- **CODE: Cleanup receipts and idempotent release.** Pylon already records
  workspace cleanup receipts. Extend that discipline to remote boxes:
  release should be safe to call repeatedly, should mint a retained cleanup
  receipt, and should distinguish "provider resource deleted" from
  "registration removed". Source: `internal/cli/run.go`, `worker/src/fleet.ts`,
  `apps/pylon/src/workspace-materializer.ts`.

- **CONCEPT: Brokered artifact upload grants.** Crabbox's brokered publishing
  keeps object-store credentials in the coordinator and gives the CLI scoped
  upload URLs with size constraints. Pylon's evidence publishing should use the
  same asymmetry rather than handing storage credentials to local agent
  processes. Source: `worker/src/artifacts.ts`,
  `docs/features/artifacts.md`.

## 5. Risks, licensing, and non-goals

Crabbox is MIT licensed, so code or design can be reused if the MIT notice and
copyright obligations are preserved. Because this repository is TypeScript/Bun
and Effect-oriented, direct Go ports should be selective. The right pull-in is
mostly contracts, data shapes, and control-flow patterns.

Do not import Crabbox's full provider matrix into Pylon as a first step. Pylon
needs a small execution-provider service boundary first, likely with local
process, static SSH, and one delegated sandbox path. Cloud-provider lifecycle
should only follow after typed authority, payment, account-quota, cost, and
cleanup boundaries are explicit.

Do not broaden Pylon's public claims merely because Crabbox has a feature.
Remote execution, artifact publishing, spend caps, and portal history would all
need Pylon-native proof, redaction, and promise gates before public copy can
claim them.

Crabbox's retained logs and local captures may contain secrets. Pylon's public
projection rules are stricter than Crabbox's CLI-local debug posture, so any
borrowed log/artifact path must pass Pylon's existing
`assertPublicProjectionSafe` and `scanProofSerialization` gates, with raw logs
stored as private evidence unless explicitly redacted.

Crabbox's delegated providers vary widely in semantics. Some own lifecycle and
sync; some expose SSH helpers; some are visibility-only. Pylon should avoid a
single "remote runner" abstraction that pretends all providers can do the same
thing. Feature flags and kind-specific contracts are required.

Remote dirty-checkout sync is not a replacement for Pylon's pinned
`git_checkout` assignment contract. Public assignments should remain pinned and
safe; dirty overlays are an operator/session execution feature.

## 6. Recommended next issues

1. Define `PylonExecutionProviderSpec` and Effect services for local process,
   static SSH lease, delegated run, artifact collection, lease touch, and
   release. Ground the vocabulary in `internal/cli/provider_backend.go`, but keep
   it TypeScript-native.

2. Add a Pylon remote workspace sync design: Git-managed manifest, default
   excludes, `.pylonignore` or repo config excludes, dirty-delta estimates,
   fingerprint skip, remote Git seeding, full-resync, and large-sync/deletion
   guardrails.

3. Persist control-session events outside process memory. Start with a local
   storage backend for `sessionRef`, ordered events, retained log refs, artifact
   refs, and cleanup receipts; leave a coordinator backend as an interface.

4. Add a static SSH execution prototype for Pylon: materialize/sync a workspace
   to an existing host, run a verification command remotely, stream bounded
   stdout/stderr events, collect a required artifact, and release only the
   Pylon registration/workdir.

5. Extend retained proof artifacts with private evidence refs: timing summary,
   run log ref, workspace sync ref, artifact manifest ref, and cleanup receipt
   ref. Keep raw logs out of public projections by default.

6. Add compute quota routing alongside account quota routing: active session
   caps, TTL/idle timeout defaults, owner/org attribution, and a refs-only
   rejection receipt when a run cannot acquire compute.

7. Design a small warm-session pool for `multi-session-run.ts`: prewarm N local
   or remote execution slots, assign sessions under bounded concurrency, and
   return/release slots with explicit success/failure policy.

8. Add required-artifact gates to `dev-proof-run.ts` and control sessions so a
   verification can demand machine-readable evidence, not just command success.

9. Add a Pylon "pond" analogue only after remote sessions exist: grouped
   session labels, role slugs, exposed service refs, peer discovery, and bulk
   cleanup for multi-agent integration environments.
