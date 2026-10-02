# Cloud parallel execution audit: what we had, what runs, what to build

- Date: 2026-10-02
- Status: audit and proposal. Built since: the daily `oa-coder-host` image
  (§6, [#10224](https://github.com/OpenAgentsInc/openagents/issues/10224)).
  The rest is not implemented on `main`.
- Question from the owner: today about 16 agents and Coder (`openagents chat
  work --issues`, Codex) run in parallel on two machines (the owner's Mac and
  `coderos-4080`), and they keep filling the Mac's disk. How do we fan work out
  onto Google Cloud quickly, and what can we reuse from the cloud plans that
  existed before the repository reset?
- Method: read-only. `git` history of this repository and of the private
  `OpenAgentsInc/coder` repository, the Factory and Amp reference material, and
  read-only `gcloud` listings of project `openagentsgemini` using the automation
  service account. No builds, no cloud changes.

Link conventions. `OA@8f84d0` means
`https://github.com/OpenAgentsInc/openagents/blob/8f84d05896ef14edee491621bf977ee5315cc8ed/<path>`
(the last commit before the reset). `CODER@c8821c` means
`https://github.com/OpenAgentsInc/coder/blob/c8821c72eb/<path>` (private
repository, last commit 2026-09-29). Commit links are
`https://github.com/OpenAgentsInc/<repo>/commit/<sha>`.

## Short answer

1. **The reset.** Commit
   [`dabc08102f`](https://github.com/OpenAgentsInc/openagents/commit/dabc08102fddd72118d710d644a69c5c4eab95a2)
   "Nuke" (2026-09-18) deleted 3,536 files and 1,343,304 lines, including all
   of `docs/cloud/` (90 files), the four Cloud crates (`oa-codex-control`,
   `oa-node`, `oa-workroomd`, `oa-cloud-run-bridge`), `scripts/cloud/` and
   `infra/modules/`. The README became one word, "Rebooting". The commit gives
   no reason. The commits that follow restart the repository around Coder, Jev
   and the relay. The Agent Computer guest image scripts had already gone on
   2026-08-28 in
   [`d613b8ea22`](https://github.com/OpenAgentsInc/openagents/commit/d613b8ea22)
   (deletion of the TypeScript product roots).
2. **"Droids" and "orbs" are two vendors' words.** Factory calls its remote
   machines **Droid Computers**. Amp (Sourcegraph) calls its per-thread
   machines **orbs**. We studied both in July 2026 and decided to adapt orbs'
   repo setup contract and snapshot economics onto our own Google Cloud
   substrate. Neither vendor's documentation, as we read it, says it runs on
   Google. The Google part was always ours (section 1.4).
3. **We ran three generations of Google Cloud execution.**
   - Openagents Cloud, June to July: a per-session GCE VM provisioner, a
     Firecracker microVM provisioner on a nested-virtualization host (a microVM
     booted in about 1 second), workrooms with scoped Codex logins, and
     managed sandboxes that passed staging acceptance.
   - The Coder run pool, September, in the `coder` repository: a GCE managed
     instance group with one warm on-demand host, a spot burst group, a
     scaler, and runs claimed over a websocket with leases.
   - Our own server for the Box API, September: a QEMU VM per box, ready in
     about 27 seconds warm. Box is the hosted sandbox product from Ascii, now
     renamed **Boat** (boat.dev).

   All three are gone from `main` today. Parts of the cloud estate are still
   running and billing.
4. **Recommendation.**
   - Revive the Coder run pool shape: a managed instance group of identical
     hosts, a few slots per host, a worktree and a target directory per run,
     spot capacity, and hosts that drain and delete themselves when idle.
   - Do not revive its web-tier scaler or claim service. Pair the pool to the
     owner as a **granted computer** and dispatch runs over the host and
     computer protocols already on `main`.
   - Bake a daily image that already has the repository, the toolchains and a
     warm `origin/main` build. That is Amp's 24-hour snapshot, done with GCE
     images.
   - Use the pre-reset per-account `CODEX_HOME` rules for engine logins.
   - Keep Firecracker per-run isolation for later, for partner or untrusted
     work.

   The phases and the issues to open are in sections 4 and 5.
5. **Boat (formerly Ascii Box) as a second backend.**
   - A `large` Boat sandbox (8 vCPU, 16 GB) costs $0.072 an hour, about the
     same as one GCE spot slot. It deploys from a named template in seconds,
     cannot be preempted, and costs nothing when stopped.
   - We already wrote a Rust SDK for its earlier API, in the `coder`
     repository.
   - The plan is in the
     [Boat SDK plan](2026-10-02-boat-sdk-plan.md): port that SDK as
     `crates/boat` and add Boat as a placement backend. Use it first for
     bursty fan-out, and the GCE pool for work that must stay in our project.

## 1. What existed before the reset

### 1.1 OpenAgents Cloud in this repository (June to September 2026)

The private `OpenAgentsInc/cloud` repository was moved into this monorepo on
2026-07-09 under issue
[#8591](https://github.com/OpenAgentsInc/openagents/issues/8591) (closed). The
receipt is `OA@8f84d0 docs/cloud/MIGRATION.md`, and the index is
`OA@8f84d0 docs/cloud/README.md`. Components, from
`OA@8f84d0 docs/cloud/ARCHITECTURE.md`:

| Component | Role | State at reset |
| --- | --- | --- |
| `crates/oa-codex-control` | HTTP control plane: placement (`POST /v1/placement`), Codex runs, GCE capacity, Cloud-VM sessions, managed-sandbox runtime | Implemented. Fake provisioners by default, live lanes behind flags |
| `crates/oa-workroomd` | Sidecar inside each workroom. Turns Codex login grants into a per-workroom `CODEX_HOME`, runs `codex exec`, captures artifacts, closes out, cleans up | Implemented |
| `crates/oa-node` | Managed node daemon: registration, capabilities, health, signed updates, quarantine, receipts | Implemented |
| `crates/oa-cloud-run-bridge` | Narrow Cloud Run edge with a bearer gate | Implemented |
| `docs/cloud/contracts/*` | 24 contracts, among them `cloud_computer.v1`, `cloud_computer_capacity.v1`, `cloud_computer_checkpoint.v1`, `gce_capacity_class.v1`, `cloud_vm_provisioner.v1`, `codex_auth_grant.v1`, `compute_quota_routing.v1`, `resource_usage_receipt.v1`, `managed_sandbox.v1` | Contracts with Rust validators and fixtures |

The live lanes on Google:

- **A GCE VM per session.** `LiveGceProvisioner` in
  `crates/oa-codex-control/src/gce_capacity.rs` calls `gcloud compute
  instances create` with `e2-small`, no public IP and a firewall rule that
  admits only IAP SSH. Teardown is guaranteed and ends with a leak check that
  must find no session VMs. The live smoke on 2026-06-14 provisioned and tore
  down cleanly, but the assignment leg over IAP SSH never confirmed on a first
  boot (`OA@8f84d0 docs/cloud/bootstrap/CND-054-gce-live-per-session-provisioner-smoke.md`).
  Epic [#4996](https://github.com/OpenAgentsInc/openagents/issues/4996),
  "Code on the go… on OpenAgents Cloud (Google-first)", closed 2026-06-14.
- **Firecracker microVMs on a nested-virtualization GCE host ("Agent
  Computer").**
  - `LiveFirecrackerProvisioner` in `crates/oa-codex-control/src/cloud_vm.rs`
    runs each microVM under Firecracker's jailer
    (`OA@8f84d0 docs/cloud/bootstrap/CND-056-cloud-vm-firecracker-provisioner.md`).
  - The host is `agent-computer-gce-1`: `n2-standard-4`, nested virtualization
    on. A microVM booted in about 1 second with its own guest kernel and 2 vCPUs
    ([Agent Computer README before 2026-08-28](https://github.com/OpenAgentsInc/openagents/blob/066aab43c1d956636ddd564e9c0327bd8a3055a3/apps/pylon/deploy/agent-computer/README.md),
    "Firecracker-on-GCE substrate is proven end-to-end").
  - The guest root filesystem carried seven pinned agent harnesses: Codex,
    Claude Code, OpenCode, Pi, Goose, Cursor and Grok. It was rebaked at least
    monthly, with a boot smoke before each promotion
    (`OA@8f84d0 docs/deploy/2026-07-24-agent-computer-image-update-cadence-runbook.md`,
    [#9193](https://github.com/OpenAgentsInc/openagents/issues/9193)).
  - Production acceptance on 2026-07-12 ran one real turn end to end: the
    grant was consumed once, exit code 0, no TAP device left
    (`OA@8f84d0 docs/deploy/agent-computer-production.md`).
- **Managed agent sandboxes.** Epic
  [#9023](https://github.com/OpenAgentsInc/openagents/issues/9023), closed
  2026-07-22.
  - Each sandbox was an owner-scoped `SandboxResource` on a sealed GCE
    `e2-small` image with Secure Boot, a vTPM and an auto-delete disk.
  - It had a Box v1 compatible facade, Codex and Claude turns, guest I/O, and
    stop and resume.
  - The staging live matrix passed. Measured runs were 124 s and 208 s of
    running time, costing 690 and 1,155 micro-USD
    (`OA@8f84d0 docs/sol/2026-07-19-managed-agent-sandboxes-accepted-plan.md`,
    `OA@8f84d0 docs/sol/evidence/2026-07-20-sbx09-live-acceptance.json`,
    `OA@8f84d0 docs/cloud/bootstrap/SBX-02-managed-sandbox-runtime.md`).
  - Production enablement waited on an independent verifier and never
    happened.
- **Engine logins on cloud machines.**
  - `OA@8f84d0 docs/cloud/control/CODEX_MULTI_ACCOUNT_AUTH.md` keeps one
    `CODEX_HOME` per connected account
    (`~/.openagents-codex-accounts/<account>/auth.json`). Logging in
    happens directly in that home (`codex login --device-auth`), and two
    accounts never share `~/.codex`.
  - `OA@8f84d0 docs/cloud/contracts/openagents.codex_auth_grant.v1.md` hands
    a workroom only a reference to the login, never its contents, and limits
    each grant to 2 hours.
  - Agent Computer images carried no provider key or OAuth token. Credentials
    were redeemed per turn at run time.

### 1.2 Amp orbs, as we read them on 2026-07-02

Sources: `OA@8f84d0 docs/research/amp/2026-07-02-amp-orbs-summary.md` and
`OA@8f84d0 docs/research/amp/2026-07-02-amp-orbs-adaptation-audit.md`.

- **What an orb is.** A fresh remote machine for each agent thread, with a
  fixed shape: 16 CPU, 32 GB, Debian 12. It costs $1.66 an hour, billed by the
  minute. It pauses automatically when idle, and a paused orb costs nothing.
- **The repository owns its setup.** `.agents/setup` runs once on a fresh orb
  and `.agents/resume` runs on wake, with a 10-second limit. The state after
  setup is snapshotted and reused for 24 hours, so you pay for setup once a
  day, not once a thread.
- **Our adaptation decision.** The machine is commodity. The leverage is in
  the setup contract, the snapshots, pausing, and conventions that make a repo
  friendly to agents. We already had the substrate on GCE and Firecracker.
- **What we decided not to copy.** Ambient GitHub credentials in public-tier
  machines, and subscription-account seats. "API-key inference only in cloud
  lanes" was a stated rule at the time.

### 1.3 Factory Droid Computers, as we read them on 2026-07-16

Sources: `OA@8f84d0 docs/teardowns/2026-07-16-factory-desktop-cli-teardown.md`
and Factory's public docs at
[`droid-computers.mdx`](https://github.com/Factory-AI/factory/blob/80d2d21a56b89f2bd814a1c801184af0970630bc/docs/cli/features/droid-computers.mdx)
(local reference clone `projects/factoryai/repos/factory`).

- **What a Droid Computer is.** A persistent, long-lived machine that Droid
  sessions connect to. Unlike Amp's orbs, it keeps its state between sessions.
- **There are two kinds.**
  - *Bring your own machine* (`droid computer register`), reached through
    Factory's relay with no public port.
  - *Managed*: Factory provisions it (4 CPU, 8 GB RAM, 6 GB swap). It installs
    the `droid` daemon, pauses when idle and resumes when a session targets it.
- **How it is used.** `droid computer ssh` tunnels over the daemon, and it
  works as a VS Code `ProxyCommand`. Missions can pick a computer, and the
  daemon updates itself remotely.
- **Factory's earlier "cloud templates"** were ephemeral environments built
  from a setup script. Droid Computers superseded them.
- **The lesson the teardown recorded** is the shape: one engine and many
  clients, with work placed on "the local host or an explicitly selected
  computer". The teardown also flagged that the engine runs without a sandbox
  by default, and that session sync to the cloud is on by default.

### 1.4 "Droid orbs on Google infrastructure"

No document in either repository describes Factory's or Amp's machines as
running on Google. The Google designs are our own:

- the per-session GCE VM and the Firecracker Agent Computer (section 1.1)
- the Coder run pool and Box hosts (section 1.5)

The July adaptation audit maps orbs onto exactly that substrate.

What we take from each vendor:

| From | What to take |
| --- | --- |
| Droid Computers | A persistent computer you grant, which pauses and resumes, as the placement target |
| Orbs | A repository-owned setup step and a daily snapshot, so a new machine starts warm |

### 1.5 The Coder run pool and Box hosts (private `coder` repository, September 2026)

This is the closest thing to what the owner asked for. It lived in
`OpenAgentsInc/coder`, not in this repository, and was never deleted there.

**Run pool.** Main sources: `CODER@c8821c docs/workers.md`, `ops/pool.sh`,
`crates/coder-runner/src/pool.rs` and `claimant.rs`, and
`bins/coder-serve/src/scaler.rs`.

- **Hosts.** Two GCE managed instance groups of `c3-standard-8` hosts with
  nested virtualization, Debian 12 and 200 GB balanced disks, in
  us-central1-b.
  - `coder-pool` is the on-demand floor of one warm host.
  - `coder-pool-spot` is the burst group, on spot capacity, scaling from zero.
- **Slots.** Each host had 2 slots of 4 vCPU and 12 GiB. That was cut from
  4 × 6 GiB after `rustc` ran out of memory
  ([`657041c7fe`](https://github.com/OpenAgentsInc/coder/commit/657041c7fe)).
  The disk was raised to 200 GB after one host filled at 100 GB, because a
  single build cache is 37 to 43 GB
  ([`f2ebacf6fc`](https://github.com/OpenAgentsInc/coder/commit/f2ebacf6fc)).
- **Claiming runs.**
  - The whole pool was paired once as one computer. Each host ran
    `coder-runner serve --claim --lane cloud --slots N` and held a 60-second
    lease per slot, renewed every 15 seconds with a ping.
  - The ping was added after Cloud NAT silently dropped an idle socket for
    two hours ([`81ed01e6ac`](https://github.com/OpenAgentsInc/coder/commit/81ed01e6ac)).
  - On SIGTERM a host hands its runs back
    ([`a721fb64a8`](https://github.com/OpenAgentsInc/coder/commit/a721fb64a8)).
- **Scaling.**
  - Every 15 seconds the scaler computed waiting plus working runs divided by
    slots, then resized the spot group upward only.
  - A burst host idle for 10 minutes drained and deleted itself. Only a host
    knows it is idle, so only a host shrinks the group.
  - After a free account's 39 queued runs booted 9 hosts, the scaler was
    switched off pending sign-off
    ([`ffd76367fd`](https://github.com/OpenAgentsInc/coder/commit/ffd76367fd)).
    Nothing shows it was switched back on.
- **Isolation.** gVisor (`runsc`) per run. It measured 1.60 times slower than
  plain containers on the systrap platform and 2.36 times slower on the KVM
  platform, for a gate test step of 198 s with plain containers.
- **Results.** After each turn that changed files, a run force-pushed a
  checkpoint branch `coder/wip/<run>`. On delivery it squashed onto trunk's
  tip and pushed a branch or trunk.
- **Caches.** The image prebuilt the locked dependencies at
  `/opt/coder/target`. Each slot kept its own build cache next to its checkout
  (`crates/coder-runner/src/gate_cache.rs`), with a 20 GB free-disk floor.
  sccache was not used. The per-repository workspace image (coder #157) was
  never built.
- **Credentials.**
  - The git token and the pool's pairing credential came from Secret Manager
    ([`4b540ec94c`](https://github.com/OpenAgentsInc/coder/commit/4b540ec94c),
    [`276cd9a97e`](https://github.com/OpenAgentsInc/coder/commit/276cd9a97e)).
  - Hosts had no Codex or Claude login, so they ran the built-in loop through
    the model gateway.
  - First boots failed twice: the private bucket returned 403, and an empty
    credential was silently accepted. The boot script also printed bearer
    tokens to the serial console
    ([`3180c41352`](https://github.com/OpenAgentsInc/coder/commit/3180c41352),
    [`03b79e960f`](https://github.com/OpenAgentsInc/coder/commit/03b79e960f)).
- **Cost** (`docs/workers.md`).
  - An on-demand host was about $0.42 an hour. A spot host was about
    $0.14 an hour, about $0.07 per run-hour with 2 slots.
  - The fixed floor (web tier, one host and the database) was about $646 a
    month.
  - The pool replaced a Cloud Run worker tier of 5 × 8 vCPU that cost about
    $3,575 a month for 5 concurrent runs.
- **Targets that were set:** 20 to 60 concurrent runs, under 10 seconds to
  claim when warm, under 90 seconds when a host must boot. Warm build times on
  a pool host were never measured.

**Box hosts.** These are our own servers for the Box API (Box is now Boat;
see §1.6). Sources: `CODER@c8821c docs/box/phase-1-proof.md`,
`phase-2-proof.md`, `2026-09-05-box-compatible-infrastructure-audit.md`.

- **What a box is.** A KVM VM from a qcow2 overlay, one per box.
  `box-guest-0.4.0.qcow2` is 652 MiB on disk. Each `coder-box-pool` host
  offered 6 vCPU and 26 GB.
- **Ready time.** 116.6 s cold and about 27.5 s warm.
- **Snapshots, resume and fork** were proven on the Docker backend only.
- **`coderos-4080`** was assessed and recommended against as a standing host:
  uplink of 3 MB/s, a privileged Docker backend, reboots, and machine-check
  errors (`docs/box/coderos-4080-host-evaluation.md`,
  [`fc13690e0f`](https://github.com/OpenAgentsInc/coder/commit/fc13690e0f)).

**Orbs in that repository.** `docs/re/amp/orbs.md`
([`b7cfd1dbd9`](https://github.com/OpenAgentsInc/coder/commit/b7cfd1dbd9)) and
`docs/re/amp/coder-gap-analysis.md`
([`f0f85f0235`](https://github.com/OpenAgentsInc/coder/commit/f0f85f0235))
proposed a target choice on `delegate`, a `/machines` view, a larger host
class, and running the agent and the task as separate users.

### 1.6 Box, now Boat

- **The vendor.** Box was Ascii's hosted Linux sandbox for agents. It is now
  **Boat**:
  - site `https://boat.dev`, API `https://boat.dev/api/v1`
  - keys that begin `boat_`
  - SDKs `@boatdev/sdk` and `boat-sdk`
- **What we built against it.**
  - July, in this repository: a teardown, and a default-off Box v1
    compatibility facade over our GCE sandboxes (SBX-03). Both were deleted
    in the reset.
  - September, in the private `coder` repository: a Rust SDK for all 59
    operations (`crates/coder-box`,
    [`a45fe45cc1`](https://github.com/OpenAgentsInc/coder/commit/a45fe45cc1)).
    Gym ran on the vendor's API, forking a named snapshot for each attempt.
    Then came our own Box-compatible server and the hosts in §1.5. That work
    is still in the repository; it was paused on 2026-09-06, not deleted.
- **Today.** The live API has 69 operations, including streaming exec,
  per-sandbox usage, and scoped, expiring keys.

The full history, the API surface, a comparison with our needs, and the Rust
SDK plan are in the [Boat SDK plan](2026-10-02-boat-sdk-plan.md).

## 2. What exists now

### 2.1 On `main` (this repository)

Nothing on `main` runs a Coder task in the cloud.

- **Where tasks run.** On the machine that runs `openagents`:
  - `chat work --parallel` is capped at 4 (`MAX_PARALLEL` in
    `crates/openagents-cli/src/chat_work.rs`).
  - Auto-start is capped at 8 (`crates/coder/src/task/autostart.rs`).
  - There are 4 Cargo target slots per project, at
    `~/.openagents/targets/<project>-<sha12>-slot-<n>`, trimmed to 64 GiB
    (`crates/coder/src/task/targets.rs`,
    [#10148](https://github.com/OpenAgentsInc/openagents/issues/10148), closed
    today).
  - Task worktrees are under `~/.openagents/worktrees`, with one prewarmed
    spare per project, because a fresh worktree of this repository takes
    7.6 s (`crates/coder/src/task/spare.rs`).
- **The issue flow** (`crates/coder/src/task/issue_run.rs`):
  1. claim the issue
  2. make a worktree of `origin/main`
  3. run checks, with fix rounds
  4. commit and rebase
  5. land on main or open a PR
  6. comment and close

  Landing is serialized by an in-process lock plus a file lock, so it is
  serialized per machine only.
- **Remote computers.**
  - A computer is a resident `coder host serve` (`crates/coder-host`), paired
    over iroh with a QR code (`crates/openagents-connect`) or over SSH
    (`openagents connect --ssh`, `crates/openagents-cli/src/connect/ssh.rs`).
  - `openagents computer task HOST PROMPT --workspace LABEL` submits a task to
    a granted host (`crates/openagents-cli/src/computer.rs`). It runs only if
    that host has `coder host autostart` on for that workspace.
  - `crates/coder-reach/src/placement.rs` scores computers but never moves
    work.
- **The router plan**
  ([`docs/api/2026-10-02-agentic-execution-router.md`](../api/2026-10-02-agentic-execution-router.md)).
  It admits "one explicitly granted computer and repository" (lines 37–38).
  It lists "execution on OpenAgents' computers or the owner's computers by
  default" as a non-goal (line 48). It makes fan-out a bounded, typed plan
  controlled by the host (lines 49–52 and 407, §13.2). Phase 3, "granted
  remote computer", must prove "no implicit host substitution" (line 370).
  It contains no VM plan.
- **Fan-out.**
  [#10183](https://github.com/OpenAgentsInc/openagents/issues/10183) (open)
  splits one chat request into several read-only Coder runs.
- **Disk.**
  [`docs/background/2026-10-02-background-processes.md`](../background/2026-10-02-background-processes.md)
  records that the Mac filled twice. The causes were per-task targets of 9 to
  38 GB each, `coder-one/target` at 85 GB, `openagents-target-agentN` at 17 to
  72 GB each, worktrees at 35 GB, the gate at 55 GB, and leftover Pylon data at
  47 GB. The `disk` background rule now trims these. While this audit was
  being written the Mac had 0.8 GB free, and a full worktree checkout failed
  with "No space left on device".
- **Hosted computers.**
  - `crates/coder/src/cloud.rs` is a *model* fallback (Gemini through
    `coder-worker`), not compute.
  - `knowledge/openagents/openagents.computer-requirements.md` says "there is
    no hosted computer yet".
  - Pylon is historical (`docs/psionic-and-pylon.md`).
- **Engine logins.** Codex reads `$CODEX_HOME/auth.json`, falling back to
  `~/.codex`. Claude reads `$CLAUDE_CONFIG_DIR/.claude.json`. Rate-limit
  capacity is tracked per signed-in account
  (`crates/microcoder-loop/src/capacity.rs`,
  [#10105](https://github.com/OpenAgentsInc/openagents/issues/10105)). BYOK is
  a design only ([#10176](https://github.com/OpenAgentsInc/openagents/issues/10176)).

### 2.2 In Google Cloud (project `openagentsgemini`, read 2026-10-02)

Compute that matters here. All are in us-central1 and running unless marked.
Hourly prices are approximate on-demand list prices.

| Instance or group | Shape | Since | What it is | Note |
| --- | --- | --- | --- | --- |
| `coder-pool` MIG → `coder-pool-w2v4` | c3-standard-8, 200 GB, on-demand | 09-04 | Floor host of the September run pool (template `coder-pool-0-5-0-…`, runner 0.5.0, 1 slot of 8 vCPU and 24 GB; this setting is not in the repository) | **Orphaned.** Its serial log shows it reopening an empty claim every hour. The `coder` Cloud Run service it claims from now serves `openagents-web`. About $0.42 an hour |
| `coder-pool-spot` MIG | c3-standard-8, spot | 09-04 | Burst group | Size 0 |
| `coder-box-pool` MIG → `coder-box-pool-8b1t` | c3-standard-8, 200 GB | 09-06 | Box host (qcow2 guest 0.4.0) | Phase 1 left the group at size 0. It is at 1 again. Likely orphaned |
| `agent-computer-gce-1` | n2-standard-4, nested virtualization, 200 GB | 07-06 | Firecracker Agent Computer host, and the "agent-computer host" named in the workspace `AGENTS.md` | Its control code was deleted in the reset |
| `oa-codex-control-1` | e2-small | 07-08 | `oa-codex-control` service | Its code was deleted in the reset |
| `oa-managed-sandbox-control-{1,staging-1,sbx09-canary-1}` plus 3 Cloud Run `oa-managed-sandbox-bridge*` | e2-small | 07-19 | Managed-sandbox control and bridges | Their code was deleted in the reset |
| `oa-issue7-final-p-{a,b}` | n2-standard-16, nested virtualization, 200 GB | 08-22 | Image family `openagents-computer-host`. The owner is not in this repository; the name points to another product lane ("One") | About $0.78 an hour each |
| `one-eval-cli-builder` | n2-standard-32, 250 GB | 08-12 | Another product lane | About $1.55 an hour |
| `oa-coder-worker-1` | e2-standard-4 | 09-20 | Hosted chat, executor and decision workers. It has no toolchain and no sandbox (`docs/deployment/eval-runner.md`) | In use |
| `oa-pay-1` | e2-small | 10-02 | Pay host (`docs/deployment/pay-host.md`) | In use |
| `oa-iroh-relay-1` | e2-small | 09-29 | iroh relay | In use |
| `oa-rel-worker-linux-x64` | e2-standard-8 | 07-16 | Desktop release worker (epic 8913) | Check whether it is still needed |

Other resources:

- **Images.** 14 `oa-managed-sandbox-guest-v1` images, `one-stg-managed-computer`
  images, and one `openagents-computer-host` image. There was no image of
  this repository; the `oa-coder-host` family (§6) now is one.
- **Buckets.**
  - `openagentsgemini-autopilot-rust-sccache` already exists, so an sccache
    bucket is in place.
  - `openagentsgemini-oa-artifacts` is a natural home for run artifacts.
  - `openagentsgemini_cloudbuild` holds the old `coder-runner` and Box
    binaries.
- **GKE.** `oa-livekit-prod`, `one-prod` and `one-stg` serve other products.
  None is a coding pool.

Taken together, the hosts marked as orphaned or with deleted code
(`coder-pool`, `coder-box-pool`, `agent-computer-gce-1`, `oa-codex-control-1`,
the three sandbox control VMs) cost roughly $1.20 an hour, about $850 a month,
before disks. The `oa-issue7` hosts and `one-eval-cli-builder` add about
$3.10 an hour if they are idle. This audit changed nothing. The owner decides
what to stop.

## 3. Gap analysis against the goal

Goal: start N Coder runs, and subagent-style fan-out, on cloud machines in
seconds to a minute. Results come back through the issue flow, and the Mac
stops being the build farm.

| Need | Before the reset | On `main` now | Gap |
| --- | --- | --- | --- |
| **Fast start** | Firecracker microVM in about 1 s (host already up). Warm pool claim in seconds, and a new host in about 90 s as a target. Box ready in 27.5 s warm and 117 s cold. GCE `e2-small` from a stock image took minutes, and SSH on first boot was unreliable | A spare worktree; a fresh worktree takes 7.6 s | No cloud host and no baked image. A cold `cargo` build of this workspace dominates start time, not VM boot |
| **Per-run isolation** | A microVM per run (Firecracker), a VM per sandbox, or gVisor per run (1.6 to 2.4 times slower builds) | A worktree and a target slot per run, on one machine | For the owner's own repositories, a VM per *host* with a worktree per run is enough. The VM keeps runs off the Mac. Per-run microVMs matter only for partner or untrusted work |
| **Engine logins** | One `CODEX_HOME` per account, grant references with a 2-hour limit, nothing baked into images. The pool ran with no engine login | `~/.codex` or `$CODEX_HOME`, per-account capacity | No way to put the owner's Codex or Claude login on a cloud host safely. Copying one ChatGPT `auth.json` to many hosts risks refresh-token rotation logging the other copies out (not measured; avoid it). Subscription use for the owner's own work is a different question from resale, which the old rules forbade |
| **Results back** | Checkpoint branch per run, squash and push. Artifacts closed out by content address | The issue flow lands on main, with landing serialized per machine | Landing across machines needs a fetch, rebase and retry loop on a rejected push (git already refuses a stale push), not the local lock. Logs and evidence need a bucket and a link in the issue comment |
| **Cost and scale to zero** | A floor plus spot burst and a self-draining host (about $0.07 per spot run-hour). Orbs and Droid Computers pause when idle | Not applicable | No group and no scaler. Pick zero by default plus a stopped standby, or one warm host |
| **Disk and cache** | 200 GB per host. A cache per slot of 37 to 43 GB. Locked dependencies prebuilt in the image. A 20 GB free floor. No sccache | 4 slots and 64 GiB on the Mac | Bake a warm `origin/main` target into the image. Optionally point sccache at the existing bucket for dependency crates. Delete the worktree and target after each run |
| **Router placement** | `POST /v1/placement` with trust tiers and quotas (`compute_quota_routing.v1`: 4 active per owner) | Granted computers, a scoring rule that never moves work, and a router plan with "explicitly granted computer" | The pool must appear as a computer the owner granted, never as an implicit substitute. Dispatch must name it |
| **`chat work --parallel N`** | Not applicable | Capped at 4, local only | Needs an `--on <computer>` target. The limit becomes the target's free slots, not a constant |
| **Fan-out (#10183)** | Not applicable | Read-only plan, local | The same placement choice per run, with the host keeping control of fan-out (router §13.2) |

## 4. Recommended design

### 4.1 Shape: a cloud pool you grant like any other computer

```text
owner's Mac / phone ── openagents chat work --issues --parallel N --on cloud
                                      │  (granted-computer dispatch, NIP-HOST)
                                      ▼
                 "cloud" = one paired computer identity
                                      │
      GCE managed instance group  coder-cloud   (c3-standard-8 or larger, spot)
         host 1          host 2   …   host K       K = ceil(N / slots), 0 when idle
         ├─ slot 1: worktree + target slot + engine run
         └─ slot 2: worktree + target slot + engine run
                                      │
     push to origin/main (fetch, rebase, retry)  ·  issue comment  ·  logs → GCS
```

- **One identity, many hosts.** Pair the pool once and give each host the
  pool's computer credential from Secret Manager, as the September pool did.
  To the router it is one granted computer, "cloud". Its capacity is the sum
  of its free slots. Dispatch names it explicitly, which meets router line 37
  and the phase 3 no-substitution rule.
- **Let the client size the group.** `openagents chat work --on cloud
  --parallel N` resizes the group to `ceil(N / slots)`, waits for hosts to
  register, and then submits tasks. Hosts drain and delete themselves after 10
  idle minutes, as in the September pool. There is no web-tier scaler and no
  separate claim service. Hosts run `openagents host serve` with auto-start on
  for an allow-listed workspace. Spot is the default; on-demand is a flag.
- **The image is the snapshot.** A daily Cloud Build job bakes the image
  family `oa-coder-host`. It contains:
  - Debian 12, the Rust toolchain from `rust-toolchain`, git, gh and ripgrep
  - the pinned Codex and Claude Code CLIs
  - the `openagents` binary
  - a clone of this repository at that day's `origin/main`
  - **a warm `cargo` target for it in each slot directory**

  A new host then only fetches and builds the delta. This is Amp's "setup once
  a day", done with GCE images. Add `.agents/setup` and `.agents/resume` in
  the repository as the bake and wake hooks, following orbs.
- **Isolation.** Owner work gets a worktree and target slot per run inside a
  VM that holds only the pool's credentials and the owner's grants. That VM is
  the boundary between the runs and the Mac. Untrusted or partner work, the
  later router public task class, gets a Firecracker microVM per run. Revive
  `cloud_vm.rs`, the jailer recipe and the rootfs manifest from `OA@8f84d0`
  and the README at `066aab43c1`.
- **Engine logins.**
  - Floor and standby hosts persist: give them one account home each, logged
    in once with `codex login --device-auth`, following
    `CODEX_MULTI_ACCOUNT_AUTH.md`. Never copy `auth.json` between hosts.
  - Burst hosts use API keys from Secret Manager (`OPENAI_API_KEY`,
    `ANTHROPIC_API_KEY`), or Claude Code's long-lived token from `claude
    setup-token` (verify the current Claude Code flag before relying on it), or
    the built-in loop through the gateway.
  - The existing per-account capacity file decides which engine has headroom.
  - Nothing secret goes into the image. The boot script never echoes a
    secret: keep `set +x` around secrets and `pipefail` on credential reads.
- **Results.** The issue flow already pushes to `main`. Cross-host landing
  becomes fetch, rebase, re-run the touched checks, then push, retrying on a
  rejected push. Each turn also pushes a `coder/wip/<run>` checkpoint so a
  preempted spot host loses nothing. Logs and evidence go to
  `gs://openagentsgemini-oa-artifacts/coder/<run>/`, with the link in the
  issue comment.
- **Disk.** 200 GB pd-balanced, or local SSD for scratch, per host; 2 slots
  per `c3-standard-8`, or 4 per `c3-standard-22`, measured before choosing.
  Clean the worktree and target after each run with the same `disk` background
  rule. A preempted or drained host takes its disk with it. sccache on the
  existing bucket is optional and only for dependency crates. Measure it
  against the baked warm target first.
- **Cost.** At about $0.14 an hour per spot `c3-standard-8` with 2 slots, 16
  parallel runs for one hour cost about $1.10 in compute. Tokens dominate. Idle
  cost is zero with no floor. A stopped standby host costs only its disk
  (about $20 a month for 200 GB balanced), and if MIG standby pools are
  available in the region it can start without a cold boot. Verify
  availability before relying on it.

### 4.2 Phases

0. **Now, no code.**
   - The owner decides what to stop among the orphaned hosts in §2.2.
   - Keep cloud operations on the automation service account. Its roles
     already cover `compute.admin`, `storage.admin` and `secretmanager.admin`.
1. **One cloud computer.**
   - Bake `oa-coder-host` by hand.
   - Create one spot VM.
   - `openagents connect --ssh` it.
   - Run `computer task … --workspace openagents` with auto-start and log in
     Codex on it.
   - Measure: boot to registered, first build delta, a run's wall time, disk
     after 10 runs, and cost.
2. **The pool.**
   - Managed instance group plus template.
   - Pool identity from Secret Manager, and host self-drain.
   - `--on cloud` for `chat work` and `computer task`, with a slot-aware limit.
   - Cross-host landing retry, checkpoint branches, and artifacts to GCS.
3. **The daily image.** Built 2026-10-02; see §6.
   - Cloud Build job, `.agents/setup` and `.agents/resume`, warm targets in
     the image.
   - Optional sccache bucket. Image digest recorded per run.
4. **Router integration.** The pool as a granted computer in the router's
   placement (phase 3 of the router plan), and fan-out (#10183) placing runs on
   it under host-owned dispatch plans.
5. **Isolated runs, later.** Firecracker per run for partner or untrusted
   work, revived from `oa-codex-control` and `oa-workroomd` at `OA@8f84d0`,
   with grant references for engine logins.
6. **Boat as a second placement backend.** This can run alongside phases 1
   to 3. Port the Rust SDK and build a daily `oa-coder-main` template from the
   same `.agents/setup`, then `--on boat`. Both backends are granted
   computers; the caller picks, and neither silently stands in for the other.
   See the [Boat SDK plan](2026-10-02-boat-sdk-plan.md) §5.4.

### 4.3 What to revive and what to build

| Revive | From |
| --- | --- |
| Floor-plus-spot group, `ops/pool.sh`, template naming by boot-script digest, host self-drain, lease and heartbeat lessons (Cloud NAT idle drop), SIGTERM hand-back, first-boot hardening | `coder` repository, `docs/workers.md`, `ops/pool.sh`, `crates/coder-runner` |
| Per-slot build caches and the free-disk floor | `coder` `crates/coder-runner/src/gate_cache.rs` |
| Checkpoint-branch delivery | `coder` `docs/workers.md` |
| Per-account `CODEX_HOME`, grant references with a 2-hour limit, no secrets in images | `OA@8f84d0 docs/cloud/control/CODEX_MULTI_ACCOUNT_AUTH.md`, `docs/cloud/contracts/openagents.codex_auth_grant.v1.md` |
| Guaranteed teardown plus a final leak listing | `OA@8f84d0 crates/oa-codex-control/src/gce_capacity.rs` |
| Image rebake cadence with a boot smoke before promotion | `OA@8f84d0 docs/deploy/2026-07-24-agent-computer-image-update-cadence-runbook.md` |
| Per-owner active-run quotas | `OA@8f84d0 docs/cloud/contracts/openagents.compute_quota_routing.v1.md` |
| Firecracker per-run microVMs (phase 5) | `OA@8f84d0 crates/oa-codex-control/src/cloud_vm.rs`, `crates/oa-workroomd` |

| Build new | Why |
| --- | --- |
| `--on <computer>` placement for `chat work` and `computer task`, with slot-aware limits | Lifts `MAX_PARALLEL = 4` without implicit substitution |
| A pool computer identity in `coder-host` and `coder-computers` | One granted computer backed by K hosts |
| Client-side group sizing (`openagents cloud up/down/status`) | Replaces the old web-tier scaler |
| Cross-host landing retry in `issue_run.rs` | The landing lock is per machine |
| `oa-coder-host` image bake for this monorepo | No image of this repository exists |

Do not revive: the `coder-serve` claim websocket and scaler (that web tier is
gone), gVisor per run for owner work (it cost 1.6 to 2.4 times the build
time), our own Box-compatible server and hosts (on hold since 09-06; use
Boat itself instead, see §1.6), or the per-session `e2-small` VMs
(too small for this workspace).

## 5. Issues to open

1. **Cloud: decide and stop orphaned GCE hosts (`coder-pool`, `coder-box-pool`, `oa-codex-control-1`, sandbox control VMs, `agent-computer-gce-1`)**: list each host's owner and last use, then stop or delete with the owner's sign-off; record what was kept.
2. **Done ([#10224](https://github.com/OpenAgentsInc/openagents/issues/10224), §6). Cloud: bake an `oa-coder-host` GCE image with the repository, toolchains, engines and a warm target**: a Cloud Build job, an image family, a boot smoke and the digest in a manifest; no secrets in the image.
3. **Cloud: one spot cloud computer paired over SSH runs a Coder issue end to end**: phase 1, with measured boot, delta build, run time, disk and cost recorded in `docs/cloud/`.
4. **Coder: `--on <computer>` placement for `chat work` and `computer task`**: names a granted computer; the parallel limit is its free slots; never substitutes another host.
5. **Coder: a pool computer identity (one grant, K hosts)**: hosts read the pool credential from Secret Manager and report slots; the router and `/computers` see one computer.
6. **Cloud: `openagents cloud up/down/status` sizes a GCE managed instance group to the requested parallelism**: spot by default, hosts self-drain after 10 idle minutes, a final leak listing.
7. **Coder: cross-host landing for the issue flow**: fetch, rebase and retry on a rejected push in place of the per-machine lock; a `coder/wip/<run>` checkpoint each turn.
8. **Coder: cloud run artifacts to GCS, linked from the issue comment**: logs, evidence and diff stats under `gs://openagentsgemini-oa-artifacts/coder/<run>/`. Shipped (#10227): with `OA_ARTIFACT_BUCKET=openagentsgemini-coder-artifacts` (90-day delete lifecycle, public access prevented) every issue flow uploads `turn-N.atif.jsonl`, `change.diff`, `checks.txt` and `route.json` under `coder/<owner>-<repo>/<issue>/<task>-<outcome>/` and links them from its landing, pull request or failure comment (`crates/coder/src/task/run_artifacts.rs`). Links are authenticated console links; `OA_ARTIFACT_LINKS=signed` with `OA_ARTIFACT_SIGN_KEY` or `OA_ARTIFACT_SIGN_AS` signs them for seven days.
9. **Cloud: engine logins on cloud hosts**: one `CODEX_HOME` per account per persistent host by device login, API keys from Secret Manager for burst hosts, never a copied `auth.json`; document the Claude path.
10. **Repo: `.agents/setup` and `.agents/resume` hooks**: idempotent setup used by the image bake and by host wake, with a 10-second wake limit.
11. **Cloud: measure sccache on `openagentsgemini-autopilot-rust-sccache` against a baked warm target**: keep it only if it cuts the delta build. Measured with issue 2 (§6): it does not help the warm-slot build, and cuts a cold one from 250 s to 165 s; it stays on.
12. **Chat: place #10183 fan-out runs on a chosen computer**: dispatch plans name the computer per run; the host keeps control of fan-out.
13. **Cloud (later): Firecracker per-run isolation for partner work**: revive `cloud_vm.rs` and `oa-workroomd` from `8f84d05896` behind the router's public task class.

The Boat issues, B1 to B9 (the Rust SDK, fixtures and live test, key hygiene,
the daily template, `--on boat`, engine logins, measurements, and retiring
the Box names), are listed in the [Boat SDK plan](2026-10-02-boat-sdk-plan.md)
§6.

## 6. Built: the daily `oa-coder-host` image (2026-10-02)

Issue [#10224](https://github.com/OpenAgentsInc/openagents/issues/10224).
Runbook: [`docs/deployment/coder-host-image.md`](../deployment/coder-host-image.md).

- **One setup script for both backends.**
  [`scripts/cloud/coder-host-setup.sh`](../../scripts/cloud/coder-host-setup.sh)
  installs the build tools, git, gh, Rust 1.97.1 through rustup, sccache,
  Node 24 and the Codex, Claude Code and Grok Build CLIs (none logged in; it
  refuses to finish if a login file exists), clones the repository, and with
  `--warm` builds `openagents-cli`, `microcoder` and `coder` and their test
  targets into the first Coder target slot. Boat's daily template (#10219,
  B5) runs the same script. `cargo-zigbuild` was not needed: the hosts build
  natively for x86_64 Linux.
- **The bake.**
  [`scripts/cloud/build-coder-host-image.sh`](../../scripts/cloud/build-coder-host-image.sh)
  boots a spot `c3-standard-22` builder (no external address, service
  account `oa-coder-host` with access to the sccache bucket only), follows
  its serial console, images its disk as `oa-coder-host-YYYYMMDD` in family
  `oa-coder-host`, boot-smokes the image, deletes every VM it made and keeps
  the newest 3 images.
- **The schedule.** Cloud Scheduler job `oa-coder-host-image-daily` posts an
  inline Cloud Build build at 07:00 UTC
  ([`scripts/cloud/coder-host-image-schedule.sh`](../../scripts/cloud/coder-host-image-schedule.sh)).
  It was chosen over a Cloud Run job because the build only waits on the
  builder and Cloud Build's free minutes cover that wait. The build and the
  job run as the automation account; it cannot grant project roles, so a
  narrower bake account would need the owner and is not required.
- **The first image** is `oa-coder-host-20261002`: commit `bf30328c27`,
  9.5 GB archived, 35 GB used, a 23 GiB warm target.

Measured on a fresh spot `c3-standard-8` from the image:

| | Seconds |
| --- | --- |
| Bake, builder created to image smoke-tested | 1,343 |
| Boot to ready (create call to the ready marker, fetch included) | 53 (65 to 107 on `e2-standard-4`) |
| `cargo build -p openagents-cli` in a fresh worktree, warm slot | 94 |
| The same after one edit | 15 |
| The same into an empty target, sccache warm / off | 165 / 250 |

What changed in the design while building it:

- Cargo rebuilds every workspace crate in a new worktree, because the path
  differs, so only dependency artifacts carry over. The bake therefore prunes
  the workspace's own test executables, binaries and incremental caches
  (57 of 77 GiB) with no loss: 94 s against 98 s on the unpruned image. The
  94 s left is the workspace's own crates; trimming it is a Cargo-level
  question (for example `CARGO_INCREMENTAL=0` so that sccache can cache
  workspace crates too, as `scripts/boat-run.sh` already sets), not an image
  question.
- Warm one package per Cargo invocation. A combined build unified features
  across packages and missed both the target and sccache on a single-package
  build.
- Spot `c3-standard-8` ran out in us-central1-a twice in one afternoon, so the
  pool (phase 2) must try several zones, as the bake now does.
- `coder`'s `interop_processes` test does not compile on `main` (it names a
  `coder-worker` binary no package defines). The bake builds around it and
  records it in the image manifest.
- Monthly cost of the daily image: about $8.

## Sources not repeated above

- The workspace root's `docs/cloud/` (June 2026, historical): `codex-vm-workroom.md`
  and `references/exe-integration-plan.md` (exe.dev studied as a prototype
  substrate).
- Episode 255 transcript (`OA@8f84d0 docs/transcripts/255.md`): "your machine
  as a target, and orbs, their machine as a product."
