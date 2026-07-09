# INVARIANTS

This repo contains private managed-cloud infrastructure. Treat these invariants
as policy until an explicit change updates this file and the corresponding
tests, models, or smoke checks.

## Public / Private Boundary

- Contributor Pylon remains in `apps/pylon` (public monorepo).
- Managed Cloud implementation lives **in this monorepo** under `crates/oa-*`
  and `crates/openagents-cloud-contract` (migrated from private
  `OpenAgentsInc/cloud`; see `docs/cloud/MIGRATION.md`).
- Public schemas and redacted receipts are shared outward. Live fleet topology,
  Secret Manager material, placement credentials, and internal settlement
  secrets stay in runtime env / Secret Manager — never in tracked fixtures,
  logs, or docs.
- The Worker (`apps/openagents.com`) owns admission, billing, public projection,
  and credit authority. Cloud daemons execute and emit receipts; they do not
  own user credit ledgers or wallet/payout authority.

## Wallet And Settlement

- Contributor-wallet mode is not the default for managed cloud nodes.
- Workrooms do not receive wallet authority by default.
- Workrooms may receive settlement metadata through scoped local gateways, but
  must not receive wallet seeds, node entropy, private keys, preimages, bearer
  tokens, or raw accounting credentials.

## Capability And Secret Handling

- Workrooms consume capabilities through brokers or local gateways, not raw
  provider secrets on disk.
- Every capability attachment is scoped, revocable, auditable, and tied to a
  workroom, program, org, or template policy.
- Secret access must produce redacted evidence that can be audited without
  leaking secret material.
- GitHub write tokens may enter a bounded Codex workroom only through a
  short-lived run-scoped grant (resolved from `github_write_grant_ref`) or a
  statically-configured operator fallback (`OA_CODEX_GITHUB_TOKEN`), supplied to
  the run only through process environment variables. They must not be embedded
  in commits, git config, git remote URLs, artifacts, callbacks, traces, tracked
  files, D1, or normal logs.
- A Codex coding run may commit and push its workspace changes back to the
  target repo/branch (`repository_clone_url` / `repository_ref`) before
  workspace teardown (cloud#96 git writeback). Writeback is gated on a write
  token being present and the working tree having changes; runs without a token
  perform no writeback. The token reaches `git push` only via the askpass helper
  and process environment. The emitted writeback event and `git-writeback.json`
  receipt are refs-only (commit sha + branch ref) and must never carry the
  token.
- Codex workrooms persist every non-streaming-delta Codex event payload and
  every detected tool call to the Khala Sync / Worker event ingest path by default after
  forbidden-secret marker checks. Streamed text/tool-output deltas are omitted
  from durable persistence unless a future policy explicitly promotes them.

## Node Lifecycle

- Desired mode and observed status are distinct.
- A managed node that cannot prove backend readiness must degrade or refuse; it
  must not advertise healthy capacity.
- Update, rollback, and quarantine decisions must produce receipts or health
  events.

## Workroom Lifecycle

- A workroom is not a provider wallet and not a public provider persona.
- A workroom starts private by default.
- Ingress, token minting, public exposure, custom-domain binding, capability
  attachment, artifact upload, and closeout are receipt-bearing events.

## Cross-OS Cloud-VM Provisioner

- The cross-OS Cloud-VM provisioner (`openagents.cloud_vm_provisioner.v1`,
  `crates/oa-codex-control/src/cloud_vm.rs`, route
  `POST /v1/cloud-vm/sessions`) implements the qa-runner's `CloudVmProvisionerV2`
  seam (`provision -> exec -> copyOut -> teardown`) over firecracker microVMs and
  is owner-gated and default-OFF.
- The live firecracker lane is opt-in via `OA_CLOUD_VM_PROVISIONER=live` and
  additionally requires a Linux host with a reachable `/dev/kvm` plus configured
  kernel/rootfs images. Absent any of those, the lane falls back to the
  deterministic fake provisioner so no-KVM hosts never attempt a real boot.
- When armed but KVM is genuinely unavailable, or when the requested OS tier has
  no host pool, `provision` must refuse honestly. It must never fall back to a
  local browser and never fake a green (mirroring the qa-runner container
  backend's `ContainerEngineUnavailableError` posture).
- The provisioned handle and the provision/cleanup receipts are refs-and-limits
  only: no raw KVM socket paths, tap devices, guest IPs, SSH keys, kernel/rootfs
  absolute paths, credentials, wallet material, bearer tokens, or private
  topology markers.
- A provisioned VM carries no wallet authority. A failed acquire or unhealthy
  boot must tear down any partial jail before refusing, and teardown must be
  idempotent and always run (even on exec/copy_out failure) so a VM is never
  leaked.

## Compute Versus Labor

- Bounded sandbox execution is compute only when the runtime profile, inputs,
  policies, expected outputs, resource limits, and receipt semantics are
  declared before execution.
- For bounded Codex workrooms with declared required artifacts, a stable,
  complete required-artifact set is sufficient for artifact closeout even if
  the Codex process would otherwise continue producing a final message. The
  runner must still emit receipt-bearing artifact evidence and must not treat
  missing artifacts as success.
- `danger_full_access` is allowed only as an explicit externally isolated
  VM/container workroom profile with no wallet authority, no broad host/cloud
  credentials, session-scoped provider auth, and cleanup receipts.
- Open-ended planning, tool choice, and semantic outcome delivery belong in
  labor paths owned by Forge/Probe/Autopilot, not in a hidden cloud-node path.

## Placement And Quota Routing

- Lane-agnostic placement is cost-driven (CND-042). For a non-caller-pinned
  `Auto` assignment with both lanes eligible (GCE available), placement compares
  lanes on the measured cost-plus-10% per-VM-second estimate from the CND-042
  report (`docs/benchmarks/2026-06-14-cnd-042-gce-shc-receipt-comparison.md`)
  and records `cost_driven = true` with a refs-only `cost_basis` on the binding.
  Per owner direction, Google GCE wins ties and near-ties and is the preferred
  lane; SHC (`oa-shc-katy-01`) is chosen only when it is BOTH materially cheaper
  than GCE AND the SHC pilot recommendation is "expand". The report currently
  recommends HOLD, so cost-driven placement resolves to GCE today. Caller pins,
  the GCE-unavailable fallback, and the disabled-cost-driven path
  (`OA_CODEX_PLACEMENT_COST_DRIVEN=false`, policy-driven Google-first) record
  `cost_driven = false` with no `cost_basis`. The cost model rates live in one
  place (`openagents_cloud_contract` `*_RAW_PER_VM_SEC_NANOUSD` constants) and
  carry no raw customer cost.
- A placement assignment must not request wallet authority. A placed run
  inherits the no-wallet VM/workroom boundary; `danger_full_access` is the
  explicit default sandbox inside that boundary, never an implicit escalation
  with broad host/cloud credentials.
- Placement bindings and quota/metering records are refs-and-limits only. They
  must not carry raw owner identity, raw customer cost, GCP project ids, instance
  names, IP addresses, credentials, wallet material, bearer tokens, or private
  topology markers. The one permitted cost figure is the contract's modeled infra
  billing input (`cost_input_microusd`, the cost-plus-10% figure on a
  `compute_usage` sub-record), which is never a customer's billed/settled amount.
- Session/lease/TTL/idle caps come from
  `openagents.compute_quota_routing.v1` defaults unless fleet policy overrides
  them; placement must honor those caps rather than minting unbounded leases.
- A `cloud-gcp` placement must drive the
  `openagents.gce_capacity_class.v1` lease lifecycle
  (acquire -> ready -> in_use -> release): provision a per-session VM, run the
  Codex runner for the assignment, emit a refs-only
  `openagents.resource_usage_receipt.v1`, then idempotently release the lease
  and mint a cleanup receipt. That GCE resource-usage receipt must carry a
  `compute_usage` sub-record whose `vm_seconds` is the genuinely measured lease
  wall-time (`release_at − acquire_at`, whole seconds, saturating at 0,
  `metering_source = node_measured`) and whose `cost_input_microusd` is
  `floor(vm_seconds × cost-plus-10% rate)` using the shared
  `LaneCostModel`/`GCE_RAW_PER_VM_SEC_NANOUSD` markup (not a re-derived 1.10).
  Because that rate is the GCP published list-price catalog rate rather than a
  live GCP Billing export, `cost_input_basis` must be
  `cost_plus_10pct_gcp_catalog`, never `cost_plus_10pct_gcp` (reserved for a live
  metered Billing export) and never a fabricated "metered from billing API"
  basis; `unavailable` requires a `null` cost. Failed acquire/readiness must
  degrade or refuse,
  never advertise a healthy VM. Real GCP calls are gated behind config/ADC; a
  fake/dry-run provisioner backs unit tests and any no-cloud environment. The
  live provisioner drives real Compute Engine calls (via `gcloud`) only when
  Application Default Credentials and a configured raw project id are both
  present; otherwise it must refuse so the caller falls back, and any failed
  acquire or non-healthy readiness probe must tear down all partially created
  resources (VM and firewall) before refusing — it must never leak a running
  instance. Release must be idempotent, tolerate already-missing resources, and
  verify via a label/name-filtered `instances list` that zero session VMs
  remain (degrading loudly otherwise). The live lane is opt-in via env and ADC;
  the default provisioner is fake so no-cloud envs and unit tests never bill.
  Raw GCP project id / zone / instance name may be used only transiently at
  provisioning time and must never be retained. Lease projections, provision
  receipts, and cleanup receipts are refs-and-limits only and must reject raw
  GCP project ids, instance names, self-links, IP addresses, SSH keys,
  credentials, wallet material, bearer tokens, and private topology markers.
