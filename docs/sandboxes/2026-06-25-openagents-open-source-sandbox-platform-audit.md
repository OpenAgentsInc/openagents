# OpenAgents Sandboxes — Audit + Design for an Open-Source Agent-Sandbox Platform (clean-room vs Daytona)

- **Date:** 2026-06-25
- **Author:** Agent audit (Raynor lane)
- **Status / honest scope:** This is an **audit + design direction document**, not a product
  promise or a launched feature. It describes (a) what OpenAgents already has that constitutes
  a sandbox/isolated-compute platform today (**NOW**, with file evidence), and (b) a proposed
  architecture and phased roadmap (**FUTURE**) for an open-source agent-sandbox offering. Where
  something does not exist yet, it is called out explicitly as not-built. Nothing here claims
  production readiness, an SLA, pricing, or a ship date. This is design/direction only — do **not**
  read it as a commitment.
- **Clean-room / IP note (load-bearing — read before any implementation):** Daytona
  (`daytonaio/daytona`) closed-sourced its core in June 2026, and its open-source code was published
  under **AGPL-3.0** (strong copyleft + a network-use source-disclosure clause). **We are NOT
  switching to AGPL.** OpenAgents Sandboxes is a permissive (Apache/MIT) + economy-billed offering,
  so **we will use NONE of Daytona's code** — not copied, not adapted, not "read-then-rewritten."
  Incorporating any AGPL-3.0 code (or a derivative of it) would force-license our entire product
  AGPL-3.0 and trigger source-disclosure obligations to every user of the hosted service. That is
  disqualifying, independent of the moral/IP point that they have since withdrawn the code.
- **Correction to an earlier draft of this note:** the *working tree* at
  `~/work/projects/repos/daytona/` shows only `README.md` + `assets/` (the close-source commit
  removed the code), **but the full AGPL-3.0 source is still recoverable from that clone's local git
  history** (~4,143 commits, the complete Go/TS tree in the pack). **That history is OFF-LIMITS.**
  We do not read it, extract from it, or consult it for the build.
- **Clean-room discipline:** this audit and design were written **without reading the Daytona
  source or its history** — purely from the conceptual / public-feature / API-contract level (the
  capability surface a competitor must match). Whoever implements OpenAgents Sandboxes **must also
  not read the Daytona source/history**; they build only from this conceptual spec. No Daytona
  source, file contents, schemas, or copyrighted text appear in this doc or in any OpenAgents code.
  All architecture below is grounded in OpenAgents' own existing assets.

---

## 1. Executive summary

OpenAgents should ship **"OpenAgents Sandboxes"**: an **open-source** platform for ephemeral,
isolated, programmable compute environments for agents and developers — the open competitor to
Daytona's now-private "run AI-generated code in secure, elastic sandboxes" offering.

The strategic finding: **we already have most of the spine.** The private `cloud/` repo
(`oa-node` + `oa-workroomd`) implements a bounded, receipt-bearing, profile-enforced sandbox
execution model with an 8-state workroom lifecycle, managed preview ingress, capability gateways,
content-addressed artifact closeout, fleet quarantine/signed-update controls, **and a real
Firecracker microVM provisioner** behind a typed `cloud_vm_provisioner.v1` contract. On the public
side, Pylon + Probe already run untrusted coding-agent work against per-task git worktrees with an
assignment-lease lifecycle and admission gates, and the `openagents.com` Worker already has a
typed multi-backend runner gateway (`shc_vm | gcloud_vm | cloudflare_container`) with D1 ledgers
and R2 artifacts. We even built and archived a Rust Daytona-SDK exploration
(`backroom/archive/daytona-mvp/`), so we already understand the contract shape we must match.

What's missing is **packaging and positioning**: there is no single OSS-published SDK/CLI/API named
"OpenAgents Sandboxes," no unified sandbox lifecycle API across the substrates, no snapshot/volume
story, and no self-host story for outside developers. The work is largely **consolidation and an
SDK/API surface over assets we already own**, plus filling specific gaps (snapshots, persistent
volumes, a public preview proxy, a published OSS package).

**Differentiation vs Daytona (and vs e2b/Modal/Runloop):**

1. **Genuinely open-source** while Daytona went private — we capture the OSS sandbox mindshare
   Daytona just abandoned, license-clean (Apache/MIT), self-hostable end to end.
2. **OpenAgents economy spine** — sandboxes are first-class citizens of our metering/settlement
   system (`resource_usage_receipt.v1`, no-wallet/internal-accounting modes), billable in
   **credits or Bitcoin/Lightning** (Spark primary), not just a credit-card SaaS.
3. **Agent-native ergonomics** — designed from the assignment-lease/approval/transcript model Pylon
   and Probe already use, not a generic devbox retrofitted for agents.
4. **Verifiable execution** — content-addressed artifacts, sha256 receipt trails, signed updates,
   and an optional **confidential / attested** tier (TDX, per `sek8s` ideas) that Daytona's
   open-source product never offered.
5. **Tiered isolation by trust** — in-process (V8/WASM) → Cloudflare Containers → Firecracker
   microVMs → confidential TDX, chosen per workload instead of one-size-fits-all.

---

## 2. What we have today (system-by-system, with file evidence)

### 2.1 OpenAgents Cloud — the spine (`~/work/cloud`)

The single closest thing we have to a managed sandbox platform. This is **private** today; the OSS
sandbox offering would extract public-safe pieces and keep fleet policy private.

**Architecture** (`cloud/docs/ARCHITECTURE.md`): split into a **control plane** (Autopilot UX,
Forge intake/verification, Nexus registry/heartbeat, Treasury settlement) and an **execution plane**
(`oa-node` managed daemon, `oa-workroomd` per-workroom sidecar, Psionic runtime, Benchmark Cloud).
The recurring discipline: **compute is bounded, profile-declared, and receipt-producing; labor is
open-ended and routed separately.** That is exactly the right framing for a sandbox product.

**Sandbox profile enforcement** (`cloud/docs/oa-node/SANDBOX_PROFILE_ENFORCEMENT.md`): profiles are
registered *before* work is accepted, via:
```
oa-node sandbox profile register \
  --profile-id sandbox.posix.local --profile-digest sha256:... \
  --execution-class sandbox.posix.exec \
  --network-policy none --filesystem-policy workspace_only \
  --timeout-ms 60000 --max-artifact-bytes 10485760 \
  --secret-policy brokered_no_raw_secrets --json
```
The enforced knobs — `execution_class`, `network_policy`, `filesystem_policy`, `timeout_ms`,
`max_artifact_bytes`, `secret_policy` — are precisely the per-sandbox isolation/quota controls a
Daytona-class product exposes. Assignments are **refused** (auditable
`forge_assignment_receipt.v1`) if the sandbox block is missing, mismatches the registered digest,
or requests a timeout/artifact budget over the profile limit. With ≥1 profile registered, node
status flips `sandbox_policy = profile_enforced`.

**Firecracker microVM provisioner** (`cloud/docs/bootstrap/CND-056-cloud-vm-firecracker-provisioner.md`,
contract `cloud/docs/contracts/openagents.cloud_vm_provisioner.v1.md`). Real code exists:
`cloud/crates/oa-codex-control/src/cloud_vm.rs`, HTTP `POST /v1/cloud-vm/sessions` in
`crates/oa-codex-control/src/main.rs`, contract test `tests/cloud_vm_contract.rs`. The provisioner
trait is the sandbox lifecycle in miniature:
```rust
trait CloudVmProvisioner {
  async fn provision(...) -> Result<ProvisionedVm>;
  async fn exec(&self, vm, cmd) -> Result<ExecOutcome>;
  async fn copy_out(&self, vm) -> Result<ExtractedArtifacts>;
  async fn teardown(&self, vm) -> Result<()>;
}
```
Two lanes: a **fake** provisioner (default, no KVM, deterministic, returns a stable opaque ref) and
a **live Firecracker** lane (`OA_CLOUD_VM_PROVISIONER=live`, needs `/dev/kvm`, a kernel image, and
a rootfs image) that **launches firecracker under jailer** (seccomp/cgroup/chroot), boots the
guest, execs over a **vsock** control channel, copies `/qa/artifacts`, and tears down the jail.
Invariants already hold: **refs-only handles**, no raw KVM socket paths / tap devices / guest IPs /
SSH keys / rootfs paths in receipts (`contains_forbidden_material` check), **degrade-or-refuse**
(`KvmUnavailable` HTTP 500, `OsTierUnavailable` HTTP 400), and **idempotent teardown that always
runs** so VMs never leak. This is a *working microVM sandbox primitive*, not a plan.

**Workroom lifecycle** (`cloud/docs/oa-workroomd/WORKROOM_LIFECYCLE.md`): an explicit 8-state
machine — `not_created → created → running ↔ paused → exposed → closed_out → archived → destroyed`
(terminal) — with narrow legal transitions, persisted to `lifecycle-state.json`, and an append-only
`lifecycle-receipts.jsonl` where every transition records `action / from_state / to_state /
receipt_digest`. `closeout`/`destroy` are **blocked** until required artifacts are submitted. This
is directly reusable as the **sandbox state machine** (create/start/pause/resume/expose/stop/destroy).

**Managed preview ingress** (`cloud/docs/oa-workroomd/MANAGED_PREVIEW_INGRESS.md`): preview-URL
exposure with `private | collaborators | public` visibility, `preview_url` + `custom_domain`,
named-collaborator grants, and **minted endpoint tokens stored only as `sha256:` digests** (raw
token never persisted). Receipt events: `preview_exposed`, `collaborator_granted`,
`endpoint_token_minted`, `ingress_revoked`. This is the **preview-URL / port-forward** feature a
sandbox platform must have — already designed with auth and revocation.

**Capability gateways** (`cloud/docs/oa-workroomd/LINK_LOCAL_GATEWAYS.md`): six default link-local
gateways (`model`, `artifacts`, `receipts`, `memory`, `email`, `settlement`), each with an explicit
capability allow-list, file-backed revocation (no restart needed), and an append-only
`gateway-access.jsonl` audit trail with allow/deny + reason. This is the **brokered network/secret
access** model — a sandbox doesn't get raw secrets, it gets brokered capabilities.

**Metadata endpoint** (`cloud/docs/oa-workroomd/METADATA_ENDPOINT.md`): a workroom-local,
**secret-free** metadata surface (id, program, repo, template, budget `runtime_ms`/`cost_microusd`,
deadline, trust tier, capability names) — the conceptual analog of a cloud metadata service / MMDS,
already redaction-hardened with a `metadata-access.jsonl` audit trail.

**Artifact closeout** (`cloud/docs/oa-workroomd/ARTIFACT_CLOSEOUT.md`): **content-addressed**
storage (`artifacts/sha256/<digest>`), required-artifact policy, append-only
`artifact-receipts.jsonl`, a `closeout-manifest.json` (with `manifest_digest`), and **fail-closed**
closeout if any required artifact is missing. This is the persistence/output-capture contract for
sandbox results, with verifiability built in.

**Fleet control** — quarantine (`cloud/docs/oa-node/QUARANTINE.md`: enter/exit with
`pause|migrate|close` workroom policy, refuses new work while quarantined); signed updates with
rollback (`cloud/docs/oa-node/SIGNED_UPDATES.md`: channel/pin/defer, signature-digest receipts,
auto-rollback or auto-quarantine on failure); capability broker redaction
(`cloud/docs/oa-node/CAPABILITY_BROKER_REDACTION.md`: redacts headers/url/env/config/log/receipt,
strict-by-default, zero raw material in receipts).

**Metering / settlement** (`cloud/docs/oa-node/SETTLEMENT_MODES.md`, contract
`resource_usage_receipt.v1`): `no-wallet` (default) and `internal-accounting`
(`--treasury-ref`/`--nexus-ref`, `--amount-microusd` receipts). This is the **billing spine** the
sandbox product plugs into.

> **Bottom line:** `cloud/` already implements sandbox profiles + enforcement, an 8-state lifecycle,
> preview ingress with tokens, capability gateways, content-addressed artifacts/receipts, a metadata
> endpoint, fleet quarantine/signed-updates, metering, **and a live Firecracker microVM provisioner**.
> This is ~70% of a sandbox control plane — it just isn't packaged or exposed as a public SDK/API.

### 2.2 Pylon + Probe — the "run untrusted agent work" muscle (`~/work/openagents`)

**Pylon** (`apps/pylon/`, README + `src/assignment.ts`, `src/workspace-materializer.ts`,
`src/gepa-capability.ts`): a Bun/Effect node that polls for assignment **leases**
(`openagents.pylon.assignment_lease.v0.3`), runs an admission gate
(`computeAssignmentAdmission`: capability match, backend support, **isolation-profile availability**,
wall-clock + cost budgets, payout readiness), executes (Claude / Codex / Tassadar / runtime-gate),
streams progress, submits content-addressed artifacts, and closes out with a settlement state. Lease
states: `offered → accepted → running → closed | rejected | cancelled | timed-out | stale`.

Isolation today is **workspace-scoped, not VM-scoped**: `workspace-materializer.ts` materializes
**per-task git worktrees** in a Pylon-owned cache (path-traversal blocked, `..`/null-byte/absolute/
`.git` rejected, cache-root boundary enforced), exports **refs not paths**
(`file.pylon.workspace.<sha256>`), and tracks workspace leases with TTL/cleanup receipts. The GEPA
envelope (`gepa-capability.ts`) already names an isolation profile —
`pylon.isolation.local_sandbox.v0.3` — and budgets (default 20-min wall clock, $0 cost). **The
`isolationProfileRef` seam is the hook a real sandbox substrate plugs into.** Missing today:
process/network isolation and runtime cgroup enforcement (admission-gated only).

**Probe** (`packages/probe/`, runtime under `packages/runtime/src/`): the first-party coding-agent
runtime (TS/Effect/Bun reset). Tool registry with **risk classes** and **approval-required** gating
(shell/write/PR), redacted website events (`probe.website_event.v1`), per-run auth materialization
scrubbed on closeout, and contracts for managed runtime/environment
(`probe.managed_runtime.v1`, `probe.managed_environment.v1`). The assignment schema already carries
`repo`, `leaseRef`, and a `sandbox?` field — a placeholder for sandbox dispatch.
(`apps/openagents.com/docs/probe/2026-06-07-first-party-probe-runtime-audit.md` documents this and
notes the deprecated Rust Probe even had `probe managed daytona advertise|run-once` commands — that
surface is **not** carried forward.)

### 2.3 Cloudflare substrate (`apps/openagents.com`)

We use Cloudflare as the **product edge / coordination layer**, not primary heavy compute
(`wrangler.jsonc`): Workers (`openagents-autopilot`), D1 (`agent_runs`, `deployments`, events,
artifacts), R2 (`openagents-autopilot-artifacts`), Durable Objects (`SyncRoomDurableObject`,
`DurableInferenceStreamObject`, MDK sidecar containers), Queues (runner-events, enrichment,
inference batch), Browser Rendering (`BROWSER`, admin smoke + QA backend), KV, cron.

**Cloudflare Containers as a runner**
(`apps/openagents.com/docs/2026-06-04-cloudflare-containers-runner-backup-audit.md`): HTTP/WS via
`container.fetch()`, Docker (linux/amd64 only), one instance per run/deploy ID (DO-backed), cold
start 1–3s, billed in 10ms increments, break-glass SSH only. **Hard limits:** ephemeral disk (no
persistence after sleep), max `standard-4` (4 vCPU / 12 GiB / 20 GB), account caps (1,500 vCPU /
6 TiB / 30 TB), Cloudflare-controlled placement, **no KVM/Firecracker nested-virt proof**. The
runner gateway schema (`packages/sync-schema`, migration
`0010_omni_agent_runs_and_deployments.sql`) supports `shc_vm | gcloud_vm | cloudflare_container`
with a typed adapter interface (dispatch/cancel/health/callbacks); the CF-Container adapter exists
(fake + real-contract) but is **not live** (gated). Verdict: an excellent **light/web/burst** lane,
**not** a substitute for an isolated heavy/untrusted VM.

**SHC = Self-Hosted Cloud** (`apps/openagents.com/docs/2026-06-02-shc-agent-deployment-runbook.md`):
the current primary execution substrate. Node `oa-shc-katy-01` (Ubuntu 24.04, 16 vCPU / 64 GB /
256 GB SSD), control API `http://.../v1/codex-runs`, Codex broker over WS. Flow: Foldkit → Worker
→ D1/DO/R2/Queues → **SHC runner (primary)** or **GCloud (`oa-gcp-shc-katy-01`, n2-standard-16)
fallback**. Important honesty: SHC today runs Codex in `danger_full_access` — only safe **because**
the whole VM is the no-wallet boundary; it is a coarse "the box is the sandbox," not per-task
isolation.

### 2.4 Ephemeral fleet + prior Daytona work

- **`oa-codex-control`** (`cloud/crates/oa-codex-control/`, audit
  `docs/launch/2026-06-20-cloud-agent-fleet-audit.md`): a live Rust control daemon that spawns
  **per-task ephemeral GCE VMs** (`e2-small`), runs agents via `oa-workroomd codex run`, enforces a
  global concurrency cap, and does **idempotent cleanup with reconciliation labels** (zero
  stragglers) and VM-seconds receipts. This *is* a working ephemeral-VM-per-task orchestrator.
- **Pylon worktrees** (`apps/openagents.com/.../oa-node-managed-machine.ts` for the managed-machine
  contract; worktree audit `docs/autopilot-coder/2026-06-11-autopilot-worktree-support-audit.md`):
  branch-per-task isolation over a shared bare-repo cache, with lease records + cleanup receipts.
- **Hydralisk** (`docs/inference/2026-06-23-hydralisk-python-nvidia-inference-stack.md`): the
  Python/NVIDIA GPU fleet for inference + the Harbor benchmark harness (Docker + GPU). It hosts
  benchmark *containers*, but it is a **stateful inference/eval lane, not a general sandbox executor**.
- **Prior Daytona exploration**: `backroom/archive/daytona-mvp/crates/daytona/` — Cargo.toml says
  *"Rust SDK for Daytona sandbox API"* with models for `CreateSandbox`, `SandboxState`,
  `ExecuteRequest/Response`, file ops, git ops, sessions. **Archived, never integrated.** Daytona
  also appears only as *one optional backend* in Crabbox/Harbor backend lists
  (`docs/autopilot-coder/2026-06-13-crabbox-pylon-audit.md`,
  `docs/gym/2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md`,
  `docs/research/tmax/paper.md`). **We never depended on Daytona; the SDK exists only to prove we
  already understand the contract we must match.**

---

## 3. Daytona feature/contract map (clean-room, concepts only)

The capability surface a competitor must match, taken from Daytona's public README/docs framing
(concepts only — no code/text copied), each mapped to **what we'd build it on** and the tradeoffs.

| Daytona concept (clean-room) | What it means | Our substrate choice | Tradeoffs / limits |
|---|---|---|---|
| **Sandbox CRUD + lifecycle** (create/start/stop/destroy, "<90ms from code to exec") | Isolated full computer with kernel/FS/net + vCPU/RAM/disk | Lifecycle state machine from `oa-workroomd` (8 states); CF Containers for light, Firecracker for heavy | CF Containers cold start 1–3s (not <90ms); Firecracker ~125ms boot — competitive but Linux/KVM-only |
| **Process & code execution / exec** | Run commands, get stdout/stderr/exit | `CloudVmProvisioner::exec` over vsock; CF `container.fetch`; Probe shell tool | vsock works for microVM; CF exec is HTTP-shaped |
| **Filesystem operations** | Read/write/search files in sandbox | Probe FS tools; workroomd content-addressed store; VFS ideas from `agent-os` | In-process VFS (agent-os) is fastest but JS/WASM-only |
| **Snapshots (stateful, persistent)** | Freeze/restore sandbox state across sessions | **GAP** — design needed; Firecracker snapshot/resume + R2-backed rootfs diffs | Firecracker snapshotting is external tooling, not native; real work |
| **Volumes / persistence** | Durable disk attached to sandbox | **GAP** — R2/persistent-disk volumes; GCE persistent disk for VM lane | CF Containers disk is ephemeral — disqualifies it for the persistent lane |
| **Declarative builder / images / snapshots from base images** | Define env via image + packages + tooling (OCI/Docker) | OCI rootfs for Firecracker; Docker images for CF Containers; image registry | Need an image/snapshot build pipeline (don't have one packaged) |
| **Preview URLs + custom preview proxy** | Expose a port over an authenticated URL | `oa-workroomd` managed preview ingress (visibility, tokens, custom domain) + a **public preview proxy** (GAP) | Ingress model exists; the public edge proxy + cert story is new build |
| **SDK (Python/TS) + REST API + CLI** | Programmatic sandbox control | **GAP** — new OSS `@openagents/sandboxes` SDK + REST over Worker + CLI | Highest-leverage missing piece; contract shape known from `daytona-mvp` |
| **Web terminal / SSH / VNC / PTY** | Human interactive access | Probe PTY ideas; CF break-glass SSH; workroomd ingress for terminal-over-WS | VNC/computer-use is a larger lift; PTY first |
| **MCP server / agent tools / git ops / LSP** | Agent-facing programmatic surface | Probe tool registry (already has git, shell, FS); MCP wrapper over the SDK | Strong fit — Probe already is the agent tool layer |
| **Isolation (dedicated kernel, full computer)** | Strong per-sandbox boundary | Tiered: in-process → CF Container → Firecracker → TDX | The core differentiator section §4 |
| **Org / API keys / limits / billing / audit logs / webhooks** | Platform governance | Worker auth + D1; `resource_usage_receipt.v1`; gateway/lifecycle audit JSONL; Queues for webhooks | We have ledgers + audit trails; need API-key + org/limits packaging |
| **Regions** | Geographic placement | CF global edge (containers) + named GCE zones + SHC | CF placement is automatic (less control); GCE gives explicit regions |
| **Network limits / computer use / log streaming / OTel** | Operational controls | `network_policy` profile knob; Queues/SSE for log streaming; OTel exporter | network_policy enforcement at the substrate is partly a gap |
| **Self-host vs managed split** | Run it yourself or use their cloud | **OSS self-host = our differentiator**; managed = OpenAgents Cloud | This is exactly where Daytona retreated |

---

## 4. Proposed architecture — "OpenAgents Sandboxes"

### 4.1 Tiered isolation model (substrate by workload/trust)

Single API, multiple substrates selected by a `tier` / trust + the existing `isolationProfileRef`:

| Tier | Substrate | Boundary | Cold start | Best for | Status |
|---|---|---|---|---|---|
| **0 — in-process** | V8 isolate + WASM (ideas from `projects/repos/agent-os`) | Language runtime (kernel-backed polyfills, deny-by-default fs/net/childProcess) | ~5ms | High-frequency agent tool calls, JS/POSIX-via-WASM, lowest cost/multi-tenant | reference only (not built) |
| **1 — light/web** | Cloudflare Containers | Container + CF placement | 1–3s | Web previews, bursty/backup runs, light builds | adapter exists, **not live** |
| **2 — untrusted/heavy** | **Firecracker microVM** (`cloud/crates/oa-codex-control/src/cloud_vm.rs`) | Hardware KVM + jailer (seccomp/cgroup/chroot) | ~125ms | Untrusted code, full OS, native binaries, long sessions | **live lane exists** (opt-in) |
| **3 — confidential** | Intel TDX + attestation (ideas from `projects/repos/sek8s`) | CPU TEE + LUKS + attestation quote | 30–60s | Regulated/proprietary, attestable provenance, confidential GPU | reference only (not built) |

Isolation design ideas mined (concepts, not code): Firecracker's **jailer + seccomp + cgroups + TAP
+ vsock** model (`projects/repos/firecracker/docs/{jailer,seccomp,design}.md`) — already partly
realized in our live provisioner; `sek8s`'s **TDX attestation + LUKS + RTMR-measured access config**
(`projects/repos/sek8s/docs/tee-gpu-vm.md`) for the confidential tier; `agent-os`'s **central
kernel + VFS + deny-by-default permissions + ACP durable sessions** for the in-process tier.

### 4.2 Lifecycle + API/SDK surface

Reuse the `oa-workroomd` 8-state machine as the canonical **sandbox lifecycle**:
`create → start → (pause/resume) → expose → exec/fs → closeout → archive → destroy`. Surface it
through three OSS clients over one typed REST contract (`openagents.sandbox.v1`, a new public-safe
contract sibling to `cloud_vm_provisioner.v1`):

- **REST API** on the `openagents.com` Worker (auth via API keys in D1, org-scoped, rate-limited),
  dispatching to the runner gateway (`shc_vm | gcloud_vm | cloudflare_container | firecracker`).
- **SDK (TS + Python)** — `@openagents/sandboxes` / `openagents-sandboxes`: `Sandbox.create({tier})`,
  `.exec()`, `.fs.read/write/search()`, `.git.*`, `.preview()`, `.snapshot()`, `.destroy()`. The
  contract shape is already understood from the archived `daytona-mvp` SDK (built clean from our own
  models — not Daytona code).
- **CLI** wrapping the SDK; plus an **MCP server** exposing sandboxes as agent tools (Probe's tool
  registry is the natural backend).

### 4.3 Networking / preview / snapshots / persistence

- **Preview/networking:** extend `oa-workroomd` managed ingress (visibility, hashed endpoint
  tokens, custom domain) with a **public preview proxy** at the CF edge (new build) and a PTY/
  terminal-over-WS path.
- **Snapshots (GAP):** Firecracker pause+snapshot to an R2-backed rootfs+memory diff, keyed by
  content digest; restore = boot-from-snapshot. This is the headline "stateful sandbox" feature and
  is real engineering, not a config flag.
- **Persistence/volumes (GAP):** R2-backed named volumes + GCE persistent disk for the VM lane;
  explicitly **not** Cloudflare Containers (ephemeral disk).

### 4.4 Auth + metering on the OpenAgents economy spine

Every sandbox is a metered resource: VM/CPU/RAM/disk-seconds → `resource_usage_receipt.v1`, in
`no-wallet` or `internal-accounting` mode, billable in **credits or Bitcoin/Lightning (Spark
primary, MDK for checkouts)**. Audit trails come for free from the existing append-only JSONL
receipt logs (lifecycle, gateway, artifact, quarantine, update). API keys + orgs + limits live in
the Worker/D1 (extends the existing runner/agent-run schema).

### 4.5 Open-source story (OSS vs managed)

- **OSS (Apache-2.0/MIT):** the sandbox SDK/CLI/MCP, the REST contract (`openagents.sandbox.v1`),
  the Firecracker provisioner runtime (the `CloudVmProvisioner` trait + jailer/vsock harness,
  generalized from `cloud/`), reference profiles, and a **self-host guide** (bring your own
  KVM host / CF account / GCE project). This is the Daytona-abandoned OSS mindshare grab.
- **Managed (private, in `cloud/`):** fleet placement/capacity, quarantine/signed-update policy,
  capability brokering, settlement policy, the managed preview edge, and SLA. Mirrors the existing
  contributor-Pylon-OSS / managed-cloud-private boundary.

---

## 5. Gaps + what's needed

1. **No published OSS SDK/CLI/API named "OpenAgents Sandboxes."** Highest leverage; contract shape
   already known. (Pure build.)
2. **No unified public sandbox lifecycle API** across substrates — today it's split across
   `oa-workroomd` (private), the runner gateway (CF/SHC/GCE), and `oa-codex-control`. Needs one
   `openagents.sandbox.v1` contract.
3. **Snapshots** — not built. Firecracker snapshot/restore + R2 diff store.
4. **Persistent volumes** — not built. R2/GCE-PD volumes; CF Containers disqualified (ephemeral).
5. **Public preview proxy / cert + PTY terminal** — ingress *model* exists in workroomd; the public
   edge proxy and terminal path are new.
6. **Runtime isolation enforcement on the Pylon path** — process/network/cgroup isolation is
   admission-gated only; the `isolationProfileRef` seam needs a real Firecracker/CF backend wired in.
7. **CF Container runner not live** — adapter exists behind gates; needs arming + a smoke.
8. **Image/snapshot build pipeline** — OCI rootfs for Firecracker, Docker images for CF; no packaged
   declarative builder yet.
9. **Tier 0 (in-process) and Tier 3 (confidential)** — reference-only; not built.

---

## 6. Phased roadmap (epics)

- **Epic A — Sandbox API/SDK MVP (Tier 1, CF Containers).** Define `openagents.sandbox.v1`; ship
  REST over the Worker + TS SDK + CLI; arm the existing CF-Container adapter; lifecycle (create/exec/
  fs/destroy) + content-addressed artifacts + `resource_usage_receipt.v1` metering. Fastest path to
  a demoable open product on a substrate that's already 90% wired.
- **Epic B — Firecracker heavy/untrusted tier (Tier 2).** Generalize `cloud/crates/oa-codex-control/
  src/cloud_vm.rs` into an OSS provisioner runtime; wire it as a sandbox backend; expose exec/fs over
  vsock; harden jailer/seccomp/network profiles; self-host KVM guide.
- **Epic C — Preview + persistence.** Public preview proxy + hashed-token auth (extend workroomd
  ingress); R2/GCE-PD volumes; PTY terminal-over-WS.
- **Epic D — Snapshots.** Firecracker pause/snapshot/restore + R2 diff store; SDK `.snapshot()`.
- **Epic E — Agent-native + MCP + economy.** MCP sandbox server over Probe's tool registry;
  approval/transcript integration; Lightning/credits billing surface; org/API-key/limits/audit-log
  packaging.
- **Epic F — Confidential tier (Tier 3) + in-process tier (Tier 0).** TDX attestation lane
  (`sek8s` ideas) for confidential/verifiable execution; optional V8/WASM in-process lane
  (`agent-os` ideas) for ultra-low-latency agent tool calls.

---

## 7. Risks

- **Cloudflare Container limits** — ephemeral disk, 1–3s cold start, max standard-4, automatic
  placement, **no nested-virt proof**. Good for light/burst, wrong for persistent/untrusted-heavy.
  Mitigation: tiering; never market CF as the isolated-VM tier.
- **Firecracker ops burden** — Linux/KVM only, needs bare-metal or nested virt, kernel/rootfs image
  management, jailer/cgroup/network plumbing, snapshot tooling is external. Mitigation: we already
  have a working live lane; invest in image pipeline + runbook.
- **Security/isolation** — a sandbox escape is catastrophic for a "run AI-generated code" product.
  Mitigation: hardware boundary for untrusted (Firecracker/TDX), brokered-capability + redaction
  invariants we already enforce, deny-by-default network, fail-closed closeout.
- **Cost** — idle VMs, snapshot storage, egress. Mitigation: aggressive idle teardown (already
  idempotent in `oa-codex-control`), metering-by-default, snapshot-instead-of-keep-warm.
- **Open-source vs managed leakage** — keep fleet/placement/settlement policy private; ship runtime
  + SDK + contract OSS. Mitigation: mirror the existing Pylon-OSS / cloud-private boundary.
- **Scope creep vs Daytona's full surface** — VNC/computer-use/LSP/regions are large. Mitigation:
  ship the agent-native core (exec/fs/preview/snapshot/SDK/MCP) first; defer human-IDE surfaces.

---

## 8. Open questions

1. Where does the public sandbox control plane live — extend the `openagents.com` Worker, or a new
   OSS control service? (Leaning: Worker for the API edge, OSS provisioner runtime for the data plane.)
2. Default substrate for the OSS self-host story — Firecracker (most honest isolation, Linux-only) or
   Docker/CF (easier, weaker)? Probably Firecracker-default with a Docker dev fallback.
3. Snapshot format/portability — can a snapshot move across hosts/regions, or is it host-pinned?
4. How much of the private `cloud/` lifecycle/gateway/ingress code can be cleanly extracted to OSS
   without leaking fleet policy? (Likely: lifecycle SM, provisioner trait, ingress model = OSS;
   placement/quarantine/settlement = private.)
5. Billing unit + price (VM-seconds vs vCPU-seconds vs request) and the credits↔Lightning mapping.
6. Confidential tier demand — is TDX worth the ops cost now, or a later differentiator?
7. Relationship to Pylon — does a contributor Pylon become a sandbox *host* in a decentralized
   capacity pool (the strongest economy story), and how is that isolation trusted?

---

## 9. Sharpest findings

1. **We already have ~70% of a sandbox control plane** — `cloud/` ships sandbox profiles +
   enforcement, an 8-state lifecycle, preview ingress with hashed tokens, capability gateways,
   content-addressed artifacts/receipts, a metadata endpoint, fleet quarantine/signed-updates,
   metering, **and a live Firecracker microVM provisioner** (`cloud/crates/oa-codex-control/
   src/cloud_vm.rs`). The missing 30% is packaging: a public SDK/CLI/API, snapshots, and volumes.
2. **Best substrate per workload is unambiguous:** Cloudflare Containers for light/web/burst (cheap,
   wired, but ephemeral-disk + no nested-virt), **Firecracker microVMs for untrusted/heavy/persistent**
   (we already run them), and TDX (`sek8s` ideas) for the optional confidential/verifiable tier. CF
   Containers must **never** be sold as the isolated-VM tier.
3. **Strongest differentiator vs Daytona: be the open one, on the economy spine.** Daytona just went
   private — we take the OSS sandbox mindshare with an Apache/MIT self-hostable stack, then
   differentiate further with **metered, Bitcoin/Lightning-billable, verifiable (content-addressed
   + receipt-trailed + optionally attested) agent-native sandboxes** that Daytona's OSS product
   never offered.
4. **The integration seam already exists.** Pylon's `isolationProfileRef`
   (`pylon.isolation.local_sandbox.v0.3`) and Probe's `sandbox?` assignment field are explicit hooks
   waiting for a real substrate; wiring the Firecracker provisioner behind them is the connective
   work, not a rewrite.

---

### Appendix — key file evidence

- `~/work/cloud/docs/ARCHITECTURE.md`, `~/work/cloud/README.md`, `~/work/cloud/INVARIANTS.md`
- `~/work/cloud/docs/oa-node/SANDBOX_PROFILE_ENFORCEMENT.md`
- `~/work/cloud/docs/bootstrap/CND-056-cloud-vm-firecracker-provisioner.md`
- `~/work/cloud/crates/oa-codex-control/src/cloud_vm.rs`, `.../src/main.rs`, `.../tests/cloud_vm_contract.rs`
- `~/work/cloud/docs/contracts/openagents.cloud_vm_provisioner.v1.md`, `openagents.workroom.v1.md`, `openagents.resource_usage_receipt.v1.md`
- `~/work/cloud/docs/oa-workroomd/{WORKROOM_LIFECYCLE,MANAGED_PREVIEW_INGRESS,LINK_LOCAL_GATEWAYS,METADATA_ENDPOINT,ARTIFACT_CLOSEOUT}.md`
- `~/work/cloud/docs/oa-node/{QUARANTINE,SIGNED_UPDATES,CAPABILITY_BROKER_REDACTION,SETTLEMENT_MODES}.md`
- `~/work/openagents/apps/pylon/{README.md,src/assignment.ts,src/workspace-materializer.ts,src/gepa-capability.ts}`
- `~/work/openagents/packages/probe/` (runtime under `packages/runtime/src/`)
- `~/work/openagents/apps/openagents.com/docs/2026-06-04-cloudflare-containers-runner-backup-audit.md`
- `~/work/openagents/apps/openagents.com/docs/2026-06-02-shc-agent-deployment-runbook.md`
- `~/work/openagents/apps/openagents.com/docs/probe/2026-06-07-first-party-probe-runtime-audit.md`
- `~/work/openagents/apps/openagents.com/workers/api/migrations/0010_omni_agent_runs_and_deployments.sql`, `packages/sync-schema/src/index.ts`
- `~/work/openagents/apps/openagents.com/workers/api/src/oa-node-managed-machine.ts`
- `~/work/openagents/docs/launch/2026-06-20-cloud-agent-fleet-audit.md` (oa-codex-control fleet)
- `~/work/openagents/docs/inference/2026-06-23-hydralisk-python-nvidia-inference-stack.md`
- `~/work/openagents/docs/autopilot-coder/{2026-06-13-crabbox-pylon-audit,2026-06-11-autopilot-worktree-support-audit}.md`
- `~/work/openagents/docs/gym/2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md`
- `~/work/backroom/archive/daytona-mvp/crates/daytona/` (archived Rust Daytona-SDK exploration — not integrated)
- Isolation references (concepts only): `~/work/projects/repos/firecracker/docs/{design,jailer,seccomp}.md`, `~/work/projects/repos/sek8s/docs/tee-gpu-vm.md`, `~/work/projects/repos/agent-os/`
