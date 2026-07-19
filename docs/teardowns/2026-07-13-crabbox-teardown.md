# Crabbox Teardown — 2026-07-13

Read-only architecture and product audit of the open-source Crabbox repository
(`openclaw/crabbox`), pinned to commit
[`32e9ebca0ee4215cd71d5292004c15e49706af82`](https://github.com/openclaw/crabbox/tree/32e9ebca0ee4215cd71d5292004c15e49706af82)
("feat: expose AWS instance-profile state in inspect (#1073)", committed
2026-07-13), cloned to `projects/repos/crabbox` and read as reference material
only.

Crabbox matters to OpenAgents differently than every prior teardown subject.
Codex and Claude Code are engines. T3 Code is a supervision control plane over
foreign engines. Executor is a capability substrate. Crabbox is the first
audited product occupying the **execution-infrastructure seam underneath all
of them**: a generic remote software testing and execution control plane —
`crabbox run -- pnpm test` leases a runner on managed cloud capacity, an
existing SSH host, or a delegated sandbox provider. Syncs the dirty working
tree. Runs the command. Streams output. Records durable evidence. And releases
the target. It overlaps OpenAgents' own remote-execution surfaces
(`crates/oa-codex-control`, the GCE capacity lease lifecycle, Pylon no-spend
assignments) more directly than any prior subject.

Evidence labels (per [README](./README.md)):

- **[source]** — observed directly in the pinned source tree
- **[schema]** — encoded in a typed wire contract, storage record, or receipt
  field set
- **[docs]** — stated by the repository's own checked-in documentation
- **[test]** — encoded in a checked-in test, CI workflow, or release check
- **[history]** — supported by the pinned Git history (full history present)
- **[public]** — corroborated by a named public source, fetched 2026-07-13
- **[vision]** — stated as intended direction, not present in the audited
  implementation
- **[inferred]** — concluded from several observations
- **[limitation]** — a boundary on what this audit can establish

No Crabbox source or user state was modified. No coordinator, provider
account, or lease was exercised. Source proves intended implementation at this
commit. It does not prove every provider path is exercised in every release.

## TL.DR

Crabbox is a **provider-neutral lease/sync/run/evidence control plane with no
agent engine and no isolation ambition of its own**. A Go CLI
(`cmd/crabbox`, `internal/cli`) mints a per-lease SSH key, asks a broker for a
lease, waits for a bootstrap marker, Git-seeds and rsyncs the local diff, runs
the command over SSH streaming output, records phase-tagged run events to the
coordinator, and releases. One logical `FleetCoordinator` — deployable as a
Cloudflare Worker fronting a single Fleet Durable Object or as Node.js +
PostgreSQL/pg-boss — owns provider credentials, lease state, spend caps,
expiry alarms, run history, and live portal bridges, while the data plane
(SSH, rsync, command I/O) always runs directly CLI → runner. [source] [docs]

```text
developer machine / agent          coordinator (optional)        provider
--------------------------         ----------------------        --------
crabbox CLI (Go)                   FleetCoordinator              AWS/Azure/
  per-lease SSH key   --HTTPS-->   CF Worker + Durable Object    GCP/Hetzner/
  manifest+fingerprint             or Node.js + PostgreSQL       Daytona ...
  git seed + rsync                 lease state, provider creds,  (brokered 5)
  run + stream + events            spend caps, alarms, portal    +72 direct/
      |                                                          delegated
      +------- SSH data plane, never through the broker ------> leased runner
```

The five most important findings:

1. **77 provider adapters behind one capability-hook seam.** `internal/
   providers/` holds 77 adapter packages — five brokered clouds (AWS, Azure,
   GCP, Hetzner, Daytona), dozens of direct SSH/hypervisor targets
   (Firecracker, Proxmox, XCP-ng, Incus, KubeVirt, Apple VZ via a signed Swift
   `vmd` daemon), and delegated sandboxes (E2B, Modal, Cloudflare Containers,
   Lambda Firecracker microVMs, a TDX-attested Phala path). Core dispatches by
   declared feature set, never `provider == aws` branches. The rule is written
   law in `VISION.md` and `AGENTS.md`. [source] [docs]
2. **The lease is a real, honest lifecycle object.** `cbx_<12 hex>` IDs,
   crustacean slugs, `active|released|expired|failed` states,
   `expiresAt = min(createdAt+ttl, lastTouchedAt+idleTimeout)`, heartbeats
   carrying telemetry, reserved-versus-estimated cost, fail-closed cleanup
   that refuses to mark a lease `expired` while the machine may still exist,
   and destructive recovery gated on exact ownership proof ("labels, names,
   and IDs alone are not ownership proof"). [schema] [docs]
3. **Evidence is a first-class product surface — up to and including a signed
   run receipt.** Every brokered run gets a durable `run_` record with
   phase-tagged events, chunked logs, telemetry rings, parsed JUnit results,
   failure classification (`blockedStage`, `retryLikely`), a stable
   `--timing-json` schema, portable replayable failure capsules, and an
   optional ed25519-signed receipt that `crabbox verify` checks — but the
   verifier prints `trust=self-signed`: it proves integrity of the runner's
   own claim, not third-party or settlement-grade truth. [source] [schema]
4. **Credential custody is asymmetric and deliberately documented.** The
   coordinator holds raw provider credentials in its runtime secret
   environment (an authority holder, not a thin relay), while the runner never
   receives broker credentials and the CLI tracks a **credential-destination
   provenance lattice** — every credential destination is tagged by the trust
   class of the config source that selected it (trusted file / repository /
   environment / flag), so repository config cannot silently route an
   operator credential to a repo-chosen host. `SECURITY.md` names that exact
   cross-trust routing as an in-scope vulnerability class. [source] [docs]
5. **It is a two-founder, ten-week, agent-assisted execution factory with
   unusually serious release engineering.** 1,869 commits between 2026-04-30
   and 2026-07-13, PR numbers past #1073, Cursor/Claude/factory-droid
   co-author trailers, `CLAUDE.md` symlinked to `AGENTS.md`, and a
   draft-first, separate-trust-domain release pipeline (pinned SSH signer
   policy, credential-free candidate builds, Developer ID + notarization
   gates, byte-exact release notes) that has already publication-blocked its
   own `v0.37.0` tag over an ad-hoc re-signing trust defect. [history]
   [docs] [public]

For OpenAgents, Crabbox is the closest external implementation of the
lease/receipt seam the Cloud crates and Pylon already own — strong convergent
evidence for the lifecycle-safety and evidence-verb design, and a clean marker
of what remains unclaimed: countersigned settlement-grade receipts, broker-only
credential redemption on the execution path, and containment truth.

## 1. Identification and scope

| Field | Value | Evidence |
| --- | --- | --- |
| Repository | `openclaw/crabbox` | [source] |
| Commit | `32e9ebca0ee4215cd71d5292004c15e49706af82` | [source] |
| Commit time | 2026-07-13 17:41 -0700 | [source] |
| License | MIT | [source] `LICENSE` |
| First commit | 2026-04-30 ("Initial commit") | [history] |
| Total commits | 1,869 in ~10.5 weeks | [history] |
| Merged PR numbers | ≥ #1073 | [history] |
| Languages | Go 1.26 CLI (~431k tracked Go lines incl. tests, `internal/cli` alone 317 files / ~170k lines), TypeScript coordinator (~46k lines under `worker/`), one Swift daemon (`vmd/`) | [source] |
| CLI framework | `alecthomas/kong`, ~56 top-level commands | [source] `internal/cli/cli_kong.go` |
| Coordinator runtimes | Cloudflare Worker + one Fleet Durable Object (`idFromName("default")`), or Node.js + PostgreSQL + pg-boss | [source] [docs] |
| Providers | 77 adapter packages, 77 provider doc pages | [source] `internal/providers/`, `docs/providers/` |
| Tests | 328 Go `_test.go` files, 37 worker Vitest files, ~150 repo-script tests, race-detector CI, coverage threshold | [source] [test] |
| Latest release | v0.38.0 (2026-07-11). V0.38.1 open | [public] [history] `CHANGELOG.md` |
| Traction | ~1.1k stars, 138 forks. Homebrew tap install | [public] |
| Positioning | "generic remote software testing and execution control plane". "Warm a box, sync the diff, run the suite." | [source] `README.md` |

Org context [public]: the `openclaw` GitHub org (personal-AI-assistant
project, flagship `openclaw/openclaw` at ~383k stars, website openclaw.ai) is
led by Peter Steinberger. Crabbox launched 2026-04-30 as its remote-testbox
sibling ("Too many agents, too many test suites, one very tired Mac"), and the
repo root doubles as a native OpenClaw plugin exposing `crabbox_run`,
`crabbox_warmup`, `crabbox_status`, `crabbox_list`, and `crabbox_stop` tools.
The workspace's `projects/covenant/repos/openclaw` clone is a downstream
`one-covenant` fork of that flagship assistant repo — same upstream org as
Crabbox, not a name collision and not a Crabbox fork. Docs are generated to
<https://crabbox.sh/>. The org also hosts `crabfleet` ("Mission control for
agent runs"), whose runtime adapter Crabbox serves via `crabbox adapter`.
[public] [source]

`CLAUDE.md` is a symlink to `AGENTS.md`, which pins the build/test gate
(`go vet`, `go test -race`, worker `oxlint`/`tsc`/Vitest), mandates
provider-neutral core ("No `provider == aws/gcp/...` logic in core"), requires
full GitHub URLs for issue references, and — notably — mandates scrubbing the
parent project from the product: "New code, docs, tests, and examples should
not mention OpenClaw, Peter, or other project/person-specific workflows."
Commits carry Conventional Commit prefixes. [source] [history]

## 2. Repository anatomy

| Path | Role | Scale |
| --- | --- | --- |
| `cmd/crabbox` | CLI entrypoint | thin `main.go` |
| `cmd/crabbox-apple-vm-helper` | Native Apple VZ helper binary | thin |
| `internal/cli` | Command implementations, config, sync, run, coordinator client, claims, receipts, bridges | 317 files / ~170k lines |
| `internal/providers/*` | 77 provider adapter packages | bulk of remaining Go |
| `internal/station` | Phase-gated Station profile primitive (disabled) | small |
| `internal/applevmhelper` | Apple VZ helper runtime, vmd install/signing | ~15 files |
| `worker/src` | Runtime-neutral `FleetCoordinator`: fleet, auth, usage, portal, provider modules, bridges | ~40 modules |
| `worker/node` | Node runtime: server, pg storage, pg-boss scheduling, AWS deployment | 6 modules |
| `vmd/` | Swift `Virtualization.framework` daemon for the `apple-vm` provider | 9 Swift files |
| `runtimes/aws-lambda-microvm` | Lambda Firecracker microVM runner image | Dockerfile + Go |
| `deploy/aws` | ECS Fargate coordinator template | 1 file |
| `release/records` | Protected release records (incl. the blocked v0.37.0) | — |
| `scripts/` | ~150 release/verification/live-smoke scripts, most with paired `.test.js` | — |
| `docs/` | 27 top-level docs + `commands/` (55), `features/` (~70), `providers/` (77) | ~4,200 lines in the core eight docs alone |

All [source]. Go dependencies of note: AWS SDK v2 (including
`service/lambdamicrovms`), Azure ARM SDKs, `cloud.google.com/go/compute`,
`firecracker-go-sdk` + CNI, `lxc/incus`, Daytona SDK, Alibaba OpenSandbox SDK,
`google/go-tdx-guest` (Intel TDX quote verification for the Phala provider's
dstack attestation path — `internal/providers/phala/attestation.go`),
`zitadel/oidc`, `nhooyr.io/websocket`. [source] `go.mod`

The docs system deserves note: every command has a doc page kept in sync with
`--help` by `scripts/check-command-docs.mjs`, every provider has a page, a
`docs/source-map.md` maps each user-visible behavior to its implementing file,
and CI runs a docs link/build check. This is documentation treated as a tested
surface. [source] [test]

## 3. Execution modes: one CLI, four provider relationships

`loadBackend` (`internal/cli/provider_backend.go`) selects one of four modes
per provider [docs] [source]:

- **Brokered** — provider declares coordinator support *and* a broker URL is
  configured. Exactly five: `aws`, `azure`, `daytona`, `gcp`, `hetzner`.
  Lease lifecycle goes through the coordinator. SSH/rsync/execution stay
  CLI → runner.
- **Direct** — the CLI drives the cloud/host API itself with local
  credentials. No central history, no spend caps. The brokered five fall back
  here without a broker URL. Every other SSH-lease provider always runs here.
- **Registered direct** — `broker.mode: registered` keeps direct lifecycle
  but registers lease metadata/heartbeats with the coordinator for inventory,
  sharing, and portal bridges. "The coordinator never receives provider
  credentials or directly calls a registered provider". Deletes are
  generation-fenced and user-confirmed. [docs]
- **Delegated** — the provider owns sync and execution end to end (E2B,
  Modal, Cloudflare, Azure Dynamic Sessions, Docker Sandbox, Blacksmith,
  W&B, …). The CLI calls `Warmup`/`Run` and rejects local-sync flags.

The adapter contract is capability-shaped: each provider declares a
`ProviderSpec` feature set (`ssh`, `crabbox-sync`, `desktop`,
`workspace-checkpoint`, …) and core dispatches by feature. On the coordinator
side the lease-create path calls provider hooks (`prepareLeaseConfig`,
`prepareLeaseCreate`, `createServerWithFallback`, `finalizeLeaseCreate`,
`refreshLeaseAccess`, `hourlyPriceUSD`) instead of branching on names. A
`delegated-runner-contract` doc defines the minimum portable runner shape a
new delegated provider must prove before merging, and `VISION.md` requires
funded providers to show "real create, use, and destroy proof with zero
residue before merge, including cleanup after partial failure." [docs]
[source]

Two adapters are architecturally unusual:

- **Apple VZ (`apple-vm`)** ships a privileged Swift daemon
  (`vmd/Sources/vmd/`) that must be codesigned with the
  `com.apple.security.virtualization` entitlement. The Go helper prepares
  instance assets, installs and signs a managed copy, and spawns it to
  `serve` or `probe`. The release pipeline treats the embedded VMD as an
  executable trust path (§10). [source]
- **External (`external`)** lets a configured executable implement the
  provider protocol — a plugin seam — but its SSH endpoint, resource names,
  and output-trust contract must be repeated in trusted user config
  (`ssh.trustProviderOutput` plus a content-hash approval), so a repository
  cannot smuggle a lifecycle binary or SSH destination past the operator.
  [source] [docs]

## 4. The lease state machine

The lease is Crabbox's central object [schema] [docs]:

- **Identity**: `cbx_<12 hex>` canonical ID plus a crustacean slug generated
  from a stable hash (collisions get a 4-hex suffix). Either resolves
  anywhere `--id` is accepted.
- **States**: `active | released | expired | failed` in coordinator state.
  finer machine-level labels (`ready`, `leased`, `running`) live on provider
  resources for direct-mode cleanup and the portal grid, and are explicitly
  not the coordinator's authoritative state.
- **Expiry**: `expiresAt = min(createdAt + ttl, lastTouchedAt +
  idleTimeout)`. TTL default 5400 s capped at 86400 s, idle default 1800 s.
  Heartbeats bump `lastTouchedAt`, attach best-effort telemetry (60-sample
  ring), refresh provider SSH ingress for known source CIDRs, and are
  rejected at/after `expiresAt` — after the deadline, cleanup owns the lease.
- **Claims**: a local JSON claim binds a lease to a repo checkout under the
  state directory. `run --id` refuses cross-repo reuse without an explicit
  `--reclaim`. [docs] `docs/concepts.md`
- **Cost**: creation reserves worst-case cost (`hourlyRate × ttl`) against
  monthly budgets (global/per-owner/per-org, `CRABBOX_MAX_*`). Over-budget
  requests fail HTTP 429 `cost_limit_exceeded` before provisioning. Elapsed
  "estimated cost" is reported separately by `crabbox usage`. Rates resolve
  override → live provider pricing (AWS spot history, Hetzner catalog) →
  static default. [docs] [source] `worker/src/usage.ts`
- **Cleanup**: Durable Object alarms or pg-boss jobs reap expired leases.
  a failed provider delete keeps the lease `active` with `cleanupAttempts`/
  `cleanupError`/`cleanupRetryAt` and a 5-minute retry "rather than marking
  the lease `expired` while the machine may still exist." An AWS orphan sweep
  releases or terminates "only exact resources retained in coordinator lease
  state." `crabbox cleanup` refuses to run at all when a coordinator is
  configured, because a CLI sweep could delete live brokered leases. Direct
  cleanup follows conservative label rules (skip `keep=true`, skip
  running/provisioning until expiry + 12 h, never touch unlabeled machines).
  [docs] [source]
- **Ownership law**: `VISION.md` makes lifecycle safety the repo's one-page
  constitution — destructive/reuse operations need verified ownership bound
  to the exact provider, resource, and claim. Adoption may bind unclaimed
  resources but "must never silently retarget an already-bound claim".
  lifecycle paths fail closed when ownership or inventory checks fail. Claims
  persist enough non-secret metadata "to route and guarantee cleanup without
  persisting credentials." [docs]

Around the lease, higher-level primitives compose rather than complicate it:
`warmup`/`prewarm` (warm reusable boxes), `pond` (emergent peer groups via a
shared label, with peer discovery and an SSH `-L` mesh), `checkpoint`
(`chk_…` workspace archives or provider-native snapshots with restore/fork),
`shard` (fork a checkpoint into N parallel leases and merge their test
results), and an org-scoped ready reserve the workspace API replenishes
atomically while demand is active. [docs] [source]

## 5. The coordinator: an authority holder with a bypassed data plane

One logical `FleetCoordinator` (`worker/src/fleet.ts`) runs on two supported
runtimes behind a `CoordinatorRuntime` seam: Cloudflare (Durable Object
storage, DO alarms plus a scheduled Worker, hibernating WebSockets) and
Node.js (PostgreSQL `crabbox` schema, pg-boss `crabbox_jobs`, in-process
`ws`). Both expose the same API, auth, portal, provider adapters, cost
controls, and bridges. State does not migrate automatically between them, and
the Node runtime is single-replica because lifecycle serialization and bridge
ownership are process-local. [source] [docs]

Authority split [docs] [source]:

- The coordinator owns lease identity/state, **provider credentials** (in
  runtime secret env), server create/delete, expiry, pool listing, cost
  controls, usage accounting, run history, and audit lookups.
- The CLI owns local config and claims, per-lease SSH key generation
  (ed25519, RSA for AWS/Azure Windows), SSH readiness waits, rsync, remote
  execution, and output streaming.
- Runners never hold broker credentials. Ordinary command execution never
  routes through the coordinator. Only the live bridges (WebVNC,
  code-server proxy, mediated egress, `/v1/control`) relay runner traffic.

Auth (`worker/src/auth.ts`): Bearer required on every non-health route —
admin token, shared token, or a signed `cbxu_` user token (HMAC-SHA256,
180-day default) minted after GitHub OAuth verifies allowed-org membership.
the token keeps the OAuth credential encrypted under the session secret so
membership can be revalidated. Optional Cloudflare Access JWT supplies
identity. Raw Access headers are ignored. The portal converts exactly one
`__Host-crabbox_session` host-only cookie into bearer authority, rejects
duplicate session cookies (the #1072 fix at HEAD-1), requires same-origin
`Origin` on mutations and viewer upgrades, and gates lease-controlled
code-server HTML onto a separate per-lease origin
(`CRABBOX_CODE_ORIGIN_TEMPLATE`), failing closed when unset. [source] [docs]

The API surface is small and legible: lease lifecycle
(`/v1/leases` CRUD + heartbeat/release/share/tailscale), runs
(`/v1/runs` + events/telemetry/finish/logs), bridges (ticketed WebVNC/code/
egress), and service/admin (`/v1/usage`, `/v1/pool`, `/v1/runners`,
`/v1/images` + promote, `/v1/admin/*`). Bridge tickets live 120 s. A separate
fail-closed ECS Fargate deployment owns SSM-only "private AWS workspaces"
with no SSH path and a server-side instance allowlist — client labels cannot
select its placement. [docs] [source]

Read against T3's relay: T3's hosted plane is deliberately incapable of
execution (identity, tunnels, push only), while Crabbox's coordinator is a
genuine authority holder — it can provision and destroy cloud resources and
holds the provider keys that do so. Crabbox recovers some of T3's thinness by
keeping the data plane direct and the runner credential-free. The residual
concentration is provider-credential custody plus admin-token power.
[inferred]

## 6. Sync: content-diff shipping with guardrails

The sync pipeline (`internal/cli/run.go`, `sync_plan.go`, `sync_archive.go`)
is local-first and dirty-checkout-native [docs] [source]:

1. **Manifest** — NUL-delimited changed/deleted file list from
   `git ls-files --cached` plus `--others --exclude-standard`. Size-checked
   by a preflight. Previewable via `crabbox sync-plan` without a box.
2. **Fingerprint short-circuit** — a hash of commit, dirty-file metadata,
   and manifest. Matching local/remote fingerprints skip rsync entirely.
3. **Git seed** — the runner clones/fetches the configured base ref first so
   rsync ships only the diff.
4. **rsync** — `--files-from` against the manifest (native path on Windows).
   deletions pruned.
5. **Finalize** — remote Git hydrates the worktree against the base ref/SHA,
   a mass-deletion sanity guard runs, and the new fingerprint is recorded.

Alternative seeds: `--fresh-pr` (remote fresh checkout of a GitHub PR,
optionally with the local patch applied) and GitHub Actions hydration
(`crabbox actions hydrate` executes supported setup steps from the repo's own
workflow over SSH — same runtimes as CI without GitHub write access. A
`--github-runner` fallback registers an ephemeral self-hosted runner when
full Actions semantics are required). Environment forwarding is
allowlist-only (defaults: `CI`, `NODE_OPTIONS`). Secrets never travel as
argv. The boundary statement is crisp: Crabbox owns leasing, connectivity,
sync, recording, cleanup. **The repository owns runtimes, dependencies,
services, and secrets**. [docs]

## 7. Evidence: the run record, and a self-signed receipt

This is the section that matters most to OpenAgents' receipts thesis.

**Durable run records.** Every brokered run creates a `run_` record early and
mirrors progress so evidence survives the terminal [schema] [source]
`worker/src/types.ts`:

- `RunRecord`: id, leaseID(s), owner/org (plus every recorded backing lease
  owner, so shared-lease/replacement activity stays auditable), provider,
  target, class/serverType, command, label, state
  (`running|succeeded|failed`), phase, exitCode, syncMs/commandMs/durationMs,
  logBytes/logTruncated, `blockedStage`, `retryLikely`, parsed
  `TestResultSummary`, telemetry summary, timestamps, eventCount.
- Phase-tagged events (`run.started`, `leasing.started`,
  `bootstrap.waiting`, `sync.*`, `actions.hydrate.*`, `command.started`,
  stdout/stderr chunks, `command.finished`, `lease.released`) are durable and
  streamable: `crabbox attach <run-id>` follows an active run, `events`
  replays lifecycle, `logs` returns retained output (64 KiB chunks, 8 MiB
  cap), `results` prints parsed JUnit suites/failures, `history` lists runs.
- Telemetry samples (load/memory/disk/uptime) ride heartbeats and a per-run
  endpoint so long commands render trend lines, bounded to a 60-sample ring.
- `--timing-json` emits one stable machine-readable sync/command/total schema
  across all providers. `bench` records and aggregates cross-provider timing
  ledgers. [docs]

**Artifacts and QA proof.** `--require-artifact` turns proof JSON into a
post-run gate. `--download remote=local` pulls evidence back. Failed runs
save `.crabbox/captures/*.tar.gz` bundles by default. `desktop proof`
captures metadata + screenshot + diagnostics + MP4 + contact sheet in one
publishable bundle. `artifacts publish` uploads and comments inline-ready
assets on a PR. Artifact publication accepts only regular files from the
selected bundle. The `hermetic-agent-evidence` doc positions all of this
deliberately: Crabbox is the run-evidence layer for agent workflows —
"Crabbox should not judge model output, store reasoning traces, decide
whether a test is correct, or deliver model credentials." [docs] [source]

**Failure capsules.** `crabbox capsule from-actions <run-url>` captures a
failing CI run into a portable, replayable bundle. `capsule replay` reruns
it — reproducibility as an evidence artifact. [docs]

**The signed run receipt.** `crabbox run --attest <path>` writes a JSON
receipt signed with a locally generated ed25519 key
(`<user-config>/crabbox/attest/id_ed25519.pem`). `crabbox verify` validates
it [source] [schema] `internal/cli/attest.go`:

- Closed field set, schema_version 1: `generated_at`, `provider`,
  `lease_id`, `slug`, `run_id`, `command`, `exit_code`, `command_ms`,
  `actions_url`, `log_sha256`, `public_key`, `signature`. Unknown fields,
  duplicate keys, and trailing JSON values are rejected. The signature covers
  canonical bytes minus the signature field.
- Verification output is honest about its own trust model — it prints
  `PASS … trust=self-signed`. The key is generated and held by the same
  machine that ran the command. There is no coordinator countersignature, no
  binding to the durable run record's server-side log digest, and no
  identity binding beyond the key fingerprint. It is an integrity seal on a
  self-claim. [source] [inferred]

This is the sharpest single comparison point with OpenAgents: Crabbox
independently concluded that remote runs need signed receipts with hash-bound
logs and schema-versioned closed field sets — and stopped exactly where
OpenAgents' thesis begins, at self-attestation without settlement or
counterparty truth (§15). [inferred]

## 8. Credential custody: an asymmetric, provenance-tracked model

Three custody zones [docs] [source]:

- **Coordinator-side**: raw provider credentials (AWS/Azure/GCP/Hetzner/
  Daytona) live in the coordinator runtime's secret environment. Brokered
  Daytona shows the most careful variant: the coordinator uses its API key to
  create the sandbox and mints an *expiring* SSH access token that the CLI
  receives as a secret identity — and ready pools stay disabled for Daytona
  precisely because a stored endpoint would outlive the rotating token.
- **Runner-side**: no broker credentials on the box, ever. Project secrets
  arrive only through the repo's own setup or the explicit env allowlist.
- **CLI-side**: the credential-destination provenance system
  (`internal/cli/credential_provenance.go`) tags every credential and every
  destination (about sixty tracked provider endpoints/keys) with the source
  class that configured it — `trustedFile`, `repository`, `environment`,
  `flag` — and refuses combinations where repository-sourced config selects
  the destination for a higher-trust credential. Repository-selected SSH
  hosts cannot inherit keys, agents, or SSH config from a more trusted
  source. External-provider endpoints must be re-declared in trusted user
  config with content-hash approvals. `CRABBOX_CONFIG` pointing inside the
  repo stays repository-classed even through symlinks. [source] [docs]

Supporting hardening: credentialed HTTP redirects are confined to the exact
configured origin (scheme/host/port) on the documented clients. GitHub login
rejects callback origins that differ from the selected broker before the
browser opens. Provider diagnostics run an exact-value redaction pass
(configured secrets, auth headers, URL userinfo, common secret JSON fields,
bearer values, PEM keys) across doctor output and streamed errors. Actions
runner registration tokens travel over SSH stdin, not remote argv. User
config files are written 0600 and `doctor` flags broader permissions.
The explicitly stated limitation: captured output, downloaded artifacts, and
failure bundles are **not** automatically scrubbed. [docs] [source]

Contrast with the OpenAgents invariant set: OpenAgents' broker-only rule
("Workrooms consume capabilities through brokers or local gateways, not raw
provider secrets on disk… A codex turn with no materialized grant fails
closed", `docs/cloud/INVARIANTS.md`) is stronger on the execution path —
runner-visible credentials are broker-redeemed, per-turn, reclaim-safe.
Crabbox is stronger on a seam OpenAgents has not formalized: **typed trust
classes for configuration sources deciding credential destinations**.
[inferred]

## 9. Isolation: truth stated plainly, containment delegated

Crabbox's security documents are unusually honest [docs] `SECURITY.md`:

- "It is not designed to isolate mutually adversarial tenants, hostile users
  on a shared host, or untrusted operators behind one coordinator."
- Repository configuration is "executable project automation, like a
  Makefile" — reviewed, not sandboxed.
- Crabbox-created resources "are development execution environments, not a
  uniform security sandbox. Isolation depends on the selected provider."
- Local container socket pass-through "gives the lease authority over the
  host container engine."
- In-scope vulnerability classes are enumerated concretely (auth bypass,
  cross-owner access, cross-trust credential routing, destructive actions
  against resources Crabbox cannot strongly identify as its own, integrity
  failures in installed artifacts). Hostile repo config and adversarial
  co-tenants are explicitly out of scope.

So containment is a provider property, selectable but not enforced: the same
CLI verb runs against a Firecracker microVM, a TDX-attested Phala CVM, a
Docker socket on the developer's laptop, or a bare SSH host, and Crabbox does
not record which isolation class the evidence was produced under beyond the
provider name. There is no execution-profile negotiation, no
effective-containment record in the run record or receipt, and no policy
engine that could refuse a low-isolation provider for a high-stakes run.
Station profiles (§12) are the acknowledged future seam for supervised
workloads and scoped model-credential delivery, currently phase-gated off.
[source] [inferred]

Where Crabbox *does* enforce, it enforces supply-chain integrity on runners:
managed Windows bootstrap pins OpenSSH/Git/TightVNC/WSL rootfs downloads to
embedded SHA-256 digests and fails closed. Managed Linux pins NodeSource/
Docker/Google APT signing keys into isolated `signed-by` keyrings with
reviewed fingerprint rotation and no unpinned fallback. [docs]

## 10. Release engineering: separate trust domains, and a blocked tag

The "Release verification" badge is backed by a genuinely serious pipeline
[docs] `docs/security.md` §Release Integrity, `docs/RELEASING.md`. [Source]
`.github/release-allowed-signers`, `release/records/`, `scripts/`:

- The release identity is an annotated signed tag verified against a
  repository-pinned SSH signer policy. The peeled commit must be an ancestor
  of protected `main`. Existing tags are never rewritten.
- Orchestration and verification run from the protected-default workflow
  commit, not the tagged candidate. Candidate builds and execution receive no
  GitHub/OIDC/Homebrew/signing/publication credentials.
- Every macOS executable archive member — including the Apple VM helper's
  eventually executed embedded VMD — must be Developer ID signed
  ("OpenClaw Foundation (FWJYW4S8P8)") with hardened runtime, secure
  timestamp, exact identifiers, and (for VMD) exact tracked entitlements.
  notarization must be accepted before packaging. Native Apple Silicon and
  Intel verifier jobs bind their proof to the same tag object, commit,
  workflow SHA, release ID, asset IDs, sizes, and digests.
- The draft has an exact eight-asset inventory and release notes byte-equal
  to the tagged changelog section. Publication and the Homebrew tap update
  are separately authorized gates. Homebrew proof re-fetches public records
  credential-free, installs on a clean host, re-verifies the installed
  binary, and repeats the metadata comparison after execution.
- Cancellation is fail-closed and non-destructive: operators record state
  but never heuristically delete, replace, rewrite, or republish while a
  gate is stopped.
- The precedent proves the gates are real: the `v0.37.0` release shipped
  **source-only** because the tagged Apple VM helper ad-hoc re-signs the
  embedded VMD at runtime, breaking Developer ID trust. The tag is preserved
  and explicitly publication-blocked in its protected release record, and a
  new signed tag with a byte-preserving fix is required. [docs] [public]

Roughly 150 `scripts/*` files implement this plus per-provider live smokes,
most with paired Node test files. CI runs gofmt/vet/race across all Go
modules, a coverage threshold, repo-script tests, docs checks, a GoReleaser
snapshot, and the full worker gate on every push and PR. [source] [test]

## 11. History: a ten-week, two-founder, agent-assisted factory

- 1,869 commits from 2026-04-30 to 2026-07-13 (~25/day sustained). PR
  numbers from #11 to #1073. [history]
- Authorship is a two-person core: Peter Steinberger (872) and Vincent Koc
  (693), then a steep tail (Coy Geek 89, Yossi Eliaz 57, ~30 others).
  [history]
- Agent involvement is visible but co-pilot-shaped rather than
  committer-shaped: 73 commits carry agent co-author trailers (Cursor 41,
  factory-droid[bot] 33 among trailer lines, Claude Opus/Fable 18), a
  `clawsweeper[bot]` lands maintenance PRs, and `CLAUDE.md` → `AGENTS.md`
  gives agents the same gate humans run. Compare T3 Code's 277
  agent-prefixed commits: Crabbox's history reads as human-led with agent
  assistance, at comparable velocity with a quarter the headcount. [history]
  [inferred]
- The commit style is disciplined: Conventional Commits, PR-numbered, with
  release-note-bearing CHANGELOG sections (45 releases to 0.38.x in ten
  weeks). [history] [source]

## 12. Vision versus implementation

Stated destination beyond the audited implementation, labeled per the README
convention:

- **[vision] Marketplace credits gateway** — an OpenRouter-like credits
  layer for sandbox capacity: one customer credential, Crabbox credits,
  intent-based provider routing over broker-owned credentials, prices, and
  policy. Implemented today: status API, quote API, CLI `marketplace
  status`/`quote`, docs, tests. Explicitly not implemented: payment capture,
  durable credit ledger, reservation, lease enforcement, provider
  settlement. The doc even specifies that the billing operator must be
  deployment configuration, naming BYOK versus managed-provider modes.
  [docs] `docs/features/marketplace-credits.md` [source]
- **[vision] Station profiles** — durable supervised workloads bound to warm
  leases, staged as three separately reviewed phases (generic station →
  agent profile → scoped `modelAccess` credential delivery). Today only the
  config surface and fail-closed phase gates exist, everything disabled by
  default. `modelAccess` is explicitly barred from the ordinary env
  allowlist. [source] `internal/station/` [docs]
- **[vision] Agent runtime bridge** — contract-only doc for launching a
  repo-owned agent harness inside a lease behind one HTTP/SSE control API,
  with Crabbox owning lifecycle/auth/evidence and the harness owning the
  model loop. [docs] `docs/features/agent-runtime-bridge.md`

The pattern across all three: Crabbox writes the security boundary and the
product boundary down *before* shipping the feature, and gates the code
fail-closed until each phase is reviewed. [inferred]

## 13. Security assessment

### Strong choices

- Honest trust-model documentation with an enumerated in-scope/out-of-scope
  vulnerability taxonomy, including cross-trust credential routing as a
  named class. [docs]
- Credential-destination provenance: typed config-source trust classes gate
  which sources may select destinations for which credentials. [source]
- Data plane bypasses the control plane. Runners never hold broker
  credentials. Per-lease SSH keys. Expiring brokered Daytona tokens with
  ready-pool disablement where rotation would break. [docs] [source]
- Fail-closed lifecycle: cleanup never marks `expired` while the machine may
  exist. Destructive recovery requires exact ownership proof. Orphan sweeps
  touch only exact retained resources. `cleanup` refuses to run beside a
  coordinator. [docs]
- Redirect origin confinement, callback-origin pinning before browser login,
  exact-value diagnostic redaction, tokens over SSH stdin, 0600 config with
  doctor checks. [docs] [source]
- Supply-chain pinning on managed runners (SHA-256 installers, `signed-by`
  APT keyrings, no unpinned fallback) and a release pipeline with separate
  trust domains that has already blocked its own release over a signing
  defect. [docs] [public]
- Portal browser hardening: host-only single session cookie, same-origin
  mutation gating, per-lease code-server origins failing closed. [source]

### Residual risks and gaps

- **The coordinator is a credential concentrator.** Raw provider keys for
  five clouds plus admin/shared bearer tokens live in one service whose
  compromise mints and destroys infrastructure. There is no broker-redeemed
  per-lease grant model on the provider path. [inferred]
- **No containment truth in evidence.** Run records and receipts carry the
  provider name but not the isolation class actually in effect. Nothing
  refuses a low-isolation provider for a sensitive run. [inferred]
- **The receipt is self-signed.** Integrity of a self-claim. No
  countersignature, no server-side log-digest binding, TOFU key with no
  identity or transparency layer. [source]
- **Shared/admin token modes are coarse.** A shared automation token conveys
  owner identity via spoofable headers (`X-Crabbox-Owner` from env/git
  config) — acceptable in the stated trusted-team model, but the stated
  model is the boundary. [docs]
- Captured output, artifacts, and failure bundles are unscrubbed by design.
  review-before-share is a human step. [docs]
- Trusted-team multi-tenancy: ponds, shared leases, and the portal assume
  cooperative operators. None of it survives an adversarial tenant, and the
  docs say so. [docs]

## 14. Comparison with the reference set and OpenAgents

Where T3's §6 vocabulary applies: a Crabbox **runner** is close to an
`ExecutionEnvironment` (one reachable execution locus with its own work
root), the lease's host/user/port plus network mode (`public` versus
`tailscale`) is its `AccessEndpoint` set, and the coordinator plays
launcher-plus-registry — but Crabbox binds identity to the *lease* (a
time-bounded reservation with cost semantics), which neither T3 nor
OpenCode has: T3 environments are long-lived and free, Crabbox runners are
metered and disposable. Access and launch are likewise separate concerns
(registered mode is access-only, brokered mode is launch-plus-access).
[inferred]

| Dimension | Crabbox | T3 Code | OpenAgents direction |
| --- | --- | --- | --- |
| What it is | Lease/sync/run/evidence control plane, no agent engine | Supervision control plane over five engines | Control plane and owned runtime, receipted |
| Central object | Lease (`cbx_`, TTL/idle, cost-reserved) | Thread in an environment | Assignment/placement with closeout |
| Control plane | Authority holder: provider creds, spend caps, lifecycle | Thin relay: identity, tunnels, push. Cannot execute | Worker owns admission/billing. `oa-codex-control` owns placement. Broker-only credentials |
| Data plane | Direct CLI→runner SSH/rsync, broker bypassed | Direct client→environment WebSocket | Direct Pylon/runner execution, receipts to authority |
| Evidence | Durable run events/logs/JUnit/telemetry/timing/capsules + self-signed receipt | Internal completion signals only | Countersigned usage/closeout receipts, exact token truth, settlement states |
| Credential custody | Coordinator holds raw provider keys. Runner credential-free. CLI provenance lattice | Harness-owned. Relay holds none | Broker-redeemed per-turn grants. Refs-only receipts. Fails closed |
| Containment | Explicitly none of its own. Provider-dependent. Honestly documented | None of its own. Default danger-full-access, undocumented posture | Fail-closed profiles + effective-containment receipts |
| Economics | Reserved/estimated cost, spend caps, 429 refusal. Credits gateway [vision] | Free/BYOK | Usage-truth pre-spend + settlement rails |
| Release integrity | Separate trust domains, pinned signers, blocked-tag precedent | Unsigned artifacts can ship | Signed ledger + rollback proof (DMG-1 lane) |
| Sandbox providers | 77 adapters incl. Firecracker, TDX/Phala, Apple VZ | n/a | GCE lease lifecycle + Firecracker/Cloud-VM provisioner |

Against OpenAgents' own surfaces specifically [inferred, sources:
`docs/cloud/README.md`, `docs/cloud/INVARIANTS.md`,
`crates/openagents-cloud-contract/src/lib.rs`, the Khala→Pylon runbook in
`CLAUDE.md`]:

- **Lease versus assignment.** Crabbox's lease
  (`expiresAt = min(ttl, idle)`, heartbeat-touch, reserved cost, fail-closed
  cleanup) is the same lifecycle `oa-codex-control` drives for
  `gce.ephemeral.standard.v1` (acquire → ready → in_use → release, "never
  leak a running instance", idempotent verified release). Crabbox adds
  vocabulary OpenAgents lacks: **reserved versus estimated cost** as
  distinct ledger concepts, heartbeat-rejected-after-expiry as a hard rule,
  and cleanup-retry state that refuses to lie about machine existence.
- **Evidence versus closeout.** Crabbox's run record beats the OpenAgents
  runbook's own recorded gaps on *operational observability*: early run
  handles, `attach`, durable phase events, JUnit normalization, timing
  schemas, failure classification, capsules. OpenAgents' closeout beats
  Crabbox on *truth class*: `paymentMode: "no-spend"`,
  `settlementState`, `payoutClaimAllowed`, exact token rows
  (`usage_truth='exact'`), owner-scoped redacted traces, refs-only receipt
  hygiene, and receipts that flow to a counterparty authority instead of a
  self-signed file. Neither subsumes the other. The union is the product.
- **Coordinator versus broker-only.** Crabbox's coordinator "owns provider
  credentials" as a design statement. OpenAgents' invariant is the opposite
  ("not raw provider secrets on disk… broker-redeemed, owner-scoped
  grant… fails closed"). Crabbox's CLI-side provenance lattice, however, is
  a genuinely new idea worth importing — OpenAgents types the *grant*, but
  does not yet type the *trust class of the configuration source* that
  selected a destination.
- **Isolation truth.** Both systems execute with honest, stated,
  non-sandbox postures at the owner-local tier (Crabbox's trusted-single-
  user model. OpenAgents' owner-local `danger-full-access` executor
  invariant). OpenAgents' differentiation — effective-containment as a
  receipt, isolation-policy contracts
  (`agent_computer_isolation_policy.v1`) — remains unclaimed by Crabbox.

## 15. Adapt or consume directly?

OpenAgents already owns remote execution surfaces, so the Executor-style
decision matrix applies:

| Option | Value | Cost/risk | Decision |
| --- | --- | --- | --- |
| Replace `oa-codex-control`/GCE placement with Crabbox | 77 providers, mature lease lifecycle immediately | Coordinator holds raw provider creds (violates broker-only invariant). No settlement/receipt authority. Go/TS stack outside contracts. Evidence not refs-only | **Reject** |
| Run a Crabbox coordinator as OpenAgents cloud capacity backend | Fast multi-cloud capacity | Second lease authority beside placement contracts. Double spend-cap/quota logic. Receipts split across systems | **Reject** |
| Consume Crabbox as one *capacity provider* behind the placement seam (a `crabbox` lane like `Gcp`/`Shc`) | Optional burst capacity across its provider set with its cleanup discipline | Health/version negotiation. Still owner-supplied coordinator credentials. Evidence must be re-receipted | **Possible later, as an external provider lane only** |
| Use the CLI directly as an owner-local dev tool | Immediate utility for maintainers | None beyond normal tool trust | **Fine. No product coupling** |
| Port the load-bearing ideas into owned contracts | Fits invariants, receipts, Effect Schema, settlement | Implementation work | **Primary recommendation** |

The load-bearing ideas to port are §16's list: lease-lifecycle vocabulary,
evidence verbs, the provenance lattice, ownership-proof cleanup, and the
release-gate discipline. The shortest accurate statement:

> Crabbox proves the lease/evidence seam is a real product and got its
> lifecycle honesty right. OpenAgents should adapt that discipline into its
> receipted, broker-only, settlement-bearing placement system — not adopt a
> second control plane that holds raw provider keys.

## 16. What OpenAgents should adapt

### Adapt directly

1. **Evidence verbs as first-class CLI surface.** `attach`, `events`,
   `logs`, `results`, `history` over durable early-handle run records answer
   exactly the gaps the Khala→Pylon runbook already records (silent
   `assignment run-no-spend`, no live progress, D1 queries instead of a
   proof command). Give assignments a `run_`-equivalent early ref and
   phase-tagged durable events readable by one typed command.
2. **Lease-lifecycle vocabulary.** Reserved-versus-estimated cost as
   distinct concepts, `expiresAt = min(ttl, idle)` with heartbeat-touch and
   hard rejection after expiry, and cleanup-retry state that never marks a
   resource gone while it may exist — fold into the GCE capacity-class
   contracts and `compute_quota_routing.v1` caps.
3. **The credential-destination provenance lattice.** Type the trust class
   of every configuration source (trusted user config / repository /
   environment / flag) and make destination selection for credentials a
   checked function of that class, with repo-sourced destinations unable to
   attract higher-trust credentials. This belongs in Pylon config loading
   and Desktop settings ingestion, and it composes with (not replaces) the
   broker-only grant invariant.
4. **Ownership-proof destructive operations.** "Labels, names, and IDs alone
   are not ownership proof". Adoption never silently retargets a bound
   claim. Sweeps touch only exact retained resources. Lifecycle fails closed
   on inventory failure. The oa-codex-control provision/cleanup receipts
   already point this way — make the ownership-proof rule an explicit
   invariant with tests.
5. **Failure capsules.** A portable, replayable bundle capturing a failing
   run (checkout ref, command, environment claim, output) is a cheap,
   high-leverage evidence artifact for the fleet: a failed assignment should
   be reproducible from its capsule, not from prose.
6. **Docs as a tested surface.** Command docs mechanically checked against
   `--help`, a source map from behavior to implementing file, and CI link/
   build checks — directly transferable to the OpenAgents docs sweep.
7. **Release-gate separation.** Verification from protected-default code,
   credential-free candidate builds, byte-equal release notes, separately
   authorized publication, and the willingness to block a release over a
   trust defect (v0.37.0) — corroborates and extends the DMG-1
   notarize/staple/fail-closed lane.

### Adapt with stronger boundaries

- **Signed run receipts** — adopt the closed field set, schema versioning,
  canonical-bytes signing, and duplicate-key-rejecting verifier. Replace the
  trust model. OpenAgents receipts must be countersigned by the authority
  that observed the run (Worker/placement), bind the server-side log digest
  and containment class, and carry settlement fields — `trust=self-signed`
  is the floor, not the product.
- **The provider-adapter seam** — capability hooks and feature-set dispatch
  are the right shape (and match the existing provider-lane enums), but
  OpenAgents capacity classes must stay typed contracts with conformance
  fixtures, not string feature lists.
- **Spend caps at admission** — 429-before-provision with per-owner/org
  monthly reserved budgets is correct and matches the Worker's admission
  authority. OpenAgents' version must reconcile against exact usage truth
  rather than static rate tables.
- **Org ready pools** — atomic adopt-and-replenish warm reserves are a real
  latency product. OpenAgents' equivalent must preserve per-owner execution
  scope (no cross-owner adoption outside the org contract) and receipt the
  adoption.

### Do not copy

1. **Raw provider credentials in the control plane.** The broker-only,
   grant-redemption invariant stays. If Crabbox-style managed clouds are
   ever offered, credentials live behind the capability broker with scoped,
   revocable, auditable attachments — never as coordinator env vars.
2. **Evidence without truth classes.** Run records that carry command
   strings and owner emails are fine for a trusted team. OpenAgents
   receipts stay refs-only, redaction-classed, and settlement-graded.
3. **Containment as a provider attribute.** "Isolation depends on the
   selected provider" is honest but insufficient: execution profiles,
   fail-closed negotiation, and effective-containment receipts remain the
   OpenAgents seam.
4. **Self-signed receipts as the endpoint.** Ship countersigned or do not
   call it verification.
5. **Trusted-team multi-tenancy as a growth path.** Shared tokens with
   header-conveyed identity and cooperative-operator assumptions cannot be
   hardened incrementally into a market. OpenAgents' marketplace path keeps
   adversarial tenancy in the design from the start.

## 17. Final assessment

Crabbox answers a question none of the prior teardown subjects asked: *what
does the execution infrastructure under the agent era look like as a
standalone product?* Its answer — one lease object with honest lifecycle
semantics, one provider-neutral capability seam scaled to 77 backends, a
data plane that never traverses the control plane, evidence as a durable
recorded surface with a signed (if self-attested) receipt, and release
engineering that blocks its own releases — is the strongest shipping
validation yet of the seam OpenAgents builds in `oa-codex-control`, the
capacity-lease contracts, and the Pylon assignment/closeout loop.

What it declines to answer is, again, the half OpenAgents considers the
product: whose credentials execution runs under (broker-redeemed grants
versus a credential-holding coordinator), what isolation class the evidence
was produced in, whether the receipt binds a counterparty, and how work
settles. Crabbox states those boundaries honestly and stops. Its
marketplace-credits skeleton shows it may not stop forever.

The correct OpenAgents response is to treat Crabbox as a well-built adjacent
layer and a discipline reference: import its lifecycle honesty, evidence
verbs, provenance lattice, and release gates into the owned receipted
placement system. Interoperate with it at most as one optional external
capacity lane. And hold the differentiation line at countersigned receipts,
broker-only custody, containment truth, and settlement.

## 18. Primary source map

All paths relative to the pinned Crabbox repository.

| Concern | Primary evidence |
| --- | --- |
| Positioning and trust model | `README.md`. `SECURITY.md`. `docs/security.md` |
| Architecture and lease flow | `docs/architecture.md`. `docs/how-it-works.md`. `docs/orchestrator.md` |
| Provider-neutral core law | `VISION.md`. `docs/vision.md`. `AGENTS.md` (symlinked `CLAUDE.md`) |
| CLI command tree | `internal/cli/cli_kong.go`. `internal/cli/app.go`. `docs/cli.md` |
| Backend modes and adapter contract | `internal/cli/provider_backend.go`. `docs/provider-backends.md`. `docs/features/delegated-runner-contract.md` |
| Broker client and coordinator API | `internal/cli/coordinator.go`. `internal/cli/provider_coordinator.go` |
| Run, sync, lease execution | `internal/cli/run.go`. `internal/cli/sync_plan.go`. `internal/cli/lease.go` |
| Signed run receipts | `internal/cli/attest.go`. `internal/cli/attest_test.go` |
| Credential provenance lattice | `internal/cli/credential_provenance.go`. `docs/security.md` §Credential destinations |
| Coordinator core | `worker/src/fleet.ts`. `worker/src/types.ts`. `worker/src/coordinator-entry.ts`. `worker/src/auth.ts`. `worker/src/usage.ts` |
| Runtime split | `worker/src/coordinator-runtime.ts`. `worker/node/node-runtime.ts`. `worker/node/postgres-storage.ts` |
| Apple VZ trust path | `vmd/Sources/vmd/main.swift`. `internal/applevmhelper/`. `docs/RELEASING.md` |
| TDX attestation | `internal/providers/phala/attestation.go` |
| Station / agent bridge (gated) | `internal/station/`. `docs/features/station-profiles.md`. `docs/features/agent-runtime-bridge.md` |
| Marketplace [vision] | `docs/features/marketplace-credits.md`. `worker/src/marketplace.ts` |
| Evidence features | `docs/features/history-logs.md`. `docs/features/test-results.md`. `docs/features/artifacts.md`. `docs/features/capsules.md`. `docs/features/hermetic-agent-evidence.md`. `docs/features/telemetry.md` |
| Release engineering | `docs/RELEASING.md`. `docs/security.md` §Release Integrity. `.github/release-allowed-signers`. `release/records/`. `scripts/verify-release.sh` |
| OpenAgents comparison | `docs/cloud/README.md`. `docs/cloud/INVARIANTS.md`. `crates/openagents-cloud-contract/src/lib.rs`. `crates/oa-codex-control/`. The Khala→Pylon delegation runbook in `CLAUDE.md` (both in the OpenAgents monorepo) |
