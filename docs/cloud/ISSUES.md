# Cloud Bootstrap And Pylon-To-Node Issue List

Status: initial backlog

This is the private implementation issue list for converting the useful parts
of Pylon into a managed OpenAgents Cloud node layer while keeping contributor
Pylon open source.

Workspace routing note: when this repo is checked out under the umbrella
workspace, `docs/cloud/repo-routing.md` and `docs/cloud/issues.md` in the root
workspace define cross-repo issue placement. This file remains the private
`cloud` implementation backlog.

Ownership rule:

- `cloud` rows are implemented in this repo.
- rows that include `cloud` plus another repo are tracked here first, then split
  into linked companion issues when code must change in the other repo.
- `openagents`-only rows protect public contributor Pylon and should not be
  implemented in this private repo.
- Psionic, Probe, Forge, Autopilot, Treasury, and Nexus authority stays in those
  repos; this repo integrates through typed adapters.

## Issue List

### CND-001 Define `openagents.cloud_node.v1`

Repo: `openagents` and `cloud`

Outcome: one typed schema for node identity, host facts, lifecycle,
capabilities, policy, runtime endpoints, and evidence.

Acceptance:

- Public Pylon can publish the schema without private fleet fields.
- Private `oa-node` can implement the schema without contributor wallet UX.
- Schema fixtures cover contributor, managed, and degraded nodes.

### CND-002 Define `openagents.workroom.v1`

Repo: `cloud`

Outcome: one typed schema for workroom identity, runtime profile, capabilities,
local gateways, ingress, artifacts, and receipt events.

Acceptance:

- Workroom starts private by default.
- Capability attachment and preview exposure are explicit receipt events.
- No wallet authority appears in the default workroom schema.

### CND-003 Extract Public `pylon-core` Boundaries

Repo: `openagents`

Outcome: public contributor-side shared code is separated from full Pylon app
concerns where it reduces risk.

Acceptance:

- Identity, local admin state, availability, inventory, lifecycle, heartbeat,
  and receipt helpers can be tested without launching the TUI.
- Contributor wallet UX remains in public Pylon.
- No private cloud topology or internal settlement policy enters public Pylon.

### CND-004 Keep Full Contributor Pylon Open Source

Repo: `openagents`

Outcome: the installable contributor app, TUI, wallet UX, public provider
inventory truth, and auditable receipt behavior remain public.

Acceptance:

- Public docs say contributor Pylon remains open source.
- Contributor payout behavior remains auditable without private repo access.
- Public Pylon does not depend on private `cloud` crates.

### CND-005 Bootstrap Private `cloud` Repo

Repo: `cloud`

Outcome: private repo has scaffold, AGENTS, invariants, contracts, issue list,
and placeholder binaries.

Acceptance:

- `cargo check` passes.
- `oa-node --help` and `oa-workroomd --help` run.
- Repo docs state the public/private boundary.

### CND-006 Implement `oa-node` Config And Identity

Repo: `cloud`

Outcome: managed node has config, node identity, org binding, local state path,
service name, and signing-key reference.

Acceptance:

- `oa-node init --org <id>` creates local state without secrets in stdout.
- `oa-node status --json` reports identity and contract version.
- Re-running init is idempotent.

### CND-007 Implement Managed Node Admin Store

Repo: `cloud`

Outcome: local SQLite or embedded store persists desired mode, observed status,
health events, inventory, updates, quarantine state, and receipt cursors.

Acceptance:

- Desired mode survives restart.
- Health events are append-only.
- Corrupt or missing store degrades safely.

### CND-008 Implement Capability Detection Adapter

Repo: `cloud`

Outcome: `oa-node` detects host CPU, memory, disk, OS, accelerator inventory,
Psionic availability, sandbox engines, ingress support, and artifact support.

Acceptance:

- Detection separates present hardware from sellable capability.
- Backend failure marks capability degraded, not eligible.
- Output conforms to `openagents.cloud_node.v1`.

### CND-009 Implement Nexus Node Registry Adapter

Repo: `cloud`

Outcome: `oa-node` registers with Nexus, sends signed heartbeats, reports
health, and receives desired lifecycle updates.

Acceptance:

- Heartbeats include snapshot digest and observed status.
- Stale or rejected registration moves node to degraded/offline.
- Integration tests cover accepted, rejected, and stale registrations.

### CND-010 Implement Forge Assignment Adapter

Repo: `cloud`

Outcome: managed nodes can receive typed workroom or worker assignments from
Forge without using NIP-90 as the primary cloud scheduler.

Acceptance:

- Assignments carry template, capability, budget, artifact, and receipt policy.
- Rejected assignments produce refusal receipts.
- Open-ended labor assignments route to Probe/Forge, not sandbox compute.

### CND-011 Implement Psionic Worker Attachment

Repo: `cloud` and `psionic`

Outcome: managed node can attach Psionic inference, training, sandbox, and
cluster workers as capabilities without embedding hot runtime logic.

Acceptance:

- `oa-node status --json` reports Psionic capability readiness.
- Worker crash degrades affected products only.
- Execution receipts cite Psionic evidence digests.

### CND-012 Implement Probe Worker Attachment

Repo: `cloud` and `probe`

Outcome: managed workrooms can invoke Probe for coding-agent runtime under
explicit workroom policy.

Acceptance:

- Probe access is scoped to workroom workspace and capabilities.
- Probe cannot read raw secrets by default.
- Probe closeout produces artifacts and receipts.

### CND-013 Implement `oa-workroomd` Local Metadata Endpoint

Repo: `cloud`

Outcome: workrooms expose non-secret local metadata to agents and tools.

Acceptance:

- Metadata includes workroom, program, repo, template, budget, deadline, trust
  tier, and capability names.
- Metadata excludes raw secrets, tokens, wallet material, and private topology.
- Metadata access is logged.

### CND-014 Implement Link-Local Capability Gateways

Repo: `cloud`

Outcome: workrooms access models, artifacts, receipts, memory, email, and
settlement metadata through local gateways.

Acceptance:

- Gateway endpoints enforce scoped capability policy.
- Every access is auditable with secret redaction.
- Revoked capabilities stop working without restarting the workroom.

### CND-015 Implement Managed Preview Ingress

Repo: `cloud`

Outcome: workrooms can expose private previews, named collaborator access,
public preview switches, custom domains, and scoped endpoint tokens.

Acceptance:

- Workrooms start private.
- Every exposure, token, revocation, and custom-domain change emits a receipt.
- Ingress policy is visible to Autopilot and Forge.

Implementation scaffold: `oa-workroomd ingress` stores non-secret ingress state
in `ingress-state.json`, defaults to private, supports named collaborator
grants, public/collaborator visibility, custom domains, endpoint-token digests,
and receipt-backed revocation.

### CND-016 Implement Artifact Closeout

Repo: `cloud`

Outcome: workrooms upload declared artifacts, content digests, logs, and
closeout manifests.

Acceptance:

- Artifact uploads are content-addressed.
- Closeout fails closed when required artifacts are missing.
- Forge can verify artifact digests against receipts.

Implementation scaffold: `oa-workroomd artifacts` stores content-addressed
objects, upload receipts, required artifact policy, and a closeout manifest that
cites artifact and receipt digests for Forge verification.

### CND-017 Implement Workroom Lifecycle

Repo: `cloud`

Outcome: create, start, pause, resume, expose, closeout, archive, and destroy
workroom states are explicit and receipt-bearing.

Acceptance:

- State transitions are validated.
- Destroy cannot run before required closeout policy is satisfied.
- Restart preserves active workroom state.

Implementation scaffold: `oa-workroomd lifecycle` persists explicit workroom
states, validates create/start/pause/resume/expose/closeout/archive/destroy
transitions, emits lifecycle receipts, and gates closeout/destroy on submitted
artifact closeout when required.

### CND-045 Run Codex On GCP VM And Emit Workroom Receipts

Repo: `cloud`

Outcome: Cloud can accept one structured Codex workroom assignment for
`oa-gcp-shc-katy-01`, run Codex in a private no-wallet workroom under a
session-scoped ChatGPT/Codex account, capture declared artifacts, emit
normalized events and receipts, then clean up VM-local state.

Acceptance:

- Assignment input validates against `openagents.codex_workroom_assignment.v1`.
- The runner verifies the matching CND-046 Codex auth grant with
  `codex login status` before execution.
- The runner creates a private no-wallet workspace and writes an `AGENTS.md`
  boundary file before starting Codex.
- Codex starts as `codex exec --json --sandbox workspace-write`.
- Events use `openagents.codex_workroom_event.v1` and cover queued, started,
  log/redacted, artifact, receipt, completed, failed/timeout, and cleanup.
- Declared artifacts are captured through the normal artifact/closeout path.
- Workspace and session Codex auth material are scrubbed after success,
  failure, timeout, or cancellation.
- Logs, events, receipts, docs, and artifacts do not include raw Codex tokens,
  `auth.json` contents, OpenAI API keys, GCP credentials, wallet material, or
  VM-global Codex auth.

Implementation scaffold: `oa-workroomd codex run` consumes a structured
assignment, validates the session auth grant, runs Codex, writes event JSONL,
uploads artifacts, submits closeout, removes the temporary workspace, and
scrubs the session `CODEX_HOME`. Tests cover the full fake-Codex lifecycle,
including redaction.

GCP smoke: `oa-gcp-shc-katy-01` is reachable, has `/dev/kvm`, current Rust via
rustup, and `codex-cli 0.135.0`. The fake-Codex `oa-workroomd` runner test
passed on the VM. A real account-backed Codex run still needs the
Vortex-issued provider-account grant and brokered auth material.

### CND-046 Materialize Session-Scoped Codex Auth Grants

Repo: `cloud`

Outcome: Cloud can consume a Vortex ChatGPT/Codex provider-account grant ref,
materialize a per-session `CODEX_HOME`, run `codex login status`, and scrub
VM-local Codex auth material with redacted receipts.

Acceptance:

- Grant inputs are validated against `openagents.codex_auth_grant.v1`.
- `auth.json` is written only inside a session `CODEX_HOME`, with `0600`
  permissions on Unix.
- `codex login status` runs under that session `CODEX_HOME`.
- Cleanup removes VM-local auth material and records a scrub receipt.
- Receipts contain refs, decisions, reasons, and digests only, never raw
  credential contents.

Implementation scaffold: `oa-workroomd codex auth materialize/status/scrub`
implements the local lifecycle. Tests cover materialization, status check,
scrub, expired-grant refusal, permissions, and receipt redaction.

### CND-018 Implement Service Manager Install

Repo: `cloud`

Outcome: `oa-node` can install and report launchd/systemd service status for
managed nodes.

Acceptance:

- Install, start, stop, restart, status, and uninstall are explicit.
- Service logs avoid secret output.
- Service state appears in node health.

Implementation scaffold: `oa-node service` records launchd/systemd install
intent, start/stop/restart/status/uninstall state, redacted service events, and
health events that project service status through `oa-node status`.

### CND-019 Implement Signed Update And Rollback

Repo: `cloud`

Outcome: managed nodes support signed release channels, staged rollout,
rollback, and health-gated promotion.

Acceptance:

- Update receipts include previous version, target version, signer, and result.
- Failed rollout triggers rollback or quarantine.
- Nodes can pin or defer according to fleet policy.

Implementation scaffold: `oa-node update` records release channel policy,
version pins, deferred updates, signed apply/rollback receipts, health events,
rollback-on-failure, and quarantine when no previous version exists.

### CND-020 Implement Quarantine

Repo: `cloud`

Outcome: unhealthy, policy-violating, or suspicious nodes can be quarantined.

Acceptance:

- Quarantined nodes stop receiving new work.
- Existing workrooms are paused, migrated, or closed according to policy.
- Quarantine emits health events and control-plane receipts.

Implementation scaffold: `oa-node quarantine` enters/exits quarantine,
projects quarantined node health, refuses new Forge assignments, records
pause/migrate/close workroom drain policy, and emits health events plus
quarantine receipts.

### CND-021 Implement Settlement Modes

Repo: `cloud`, `treasury`, and `nexus`

Outcome: managed nodes support `internal-accounting` and `no-wallet` modes,
while public contributor Pylon keeps contributor-wallet mode.

Acceptance:

- No default managed workroom has wallet authority.
- Internal accounting receipts reconcile with Treasury/Nexus.
- Contributor-wallet mode remains public Pylon behavior.

Implementation scaffold: `oa-node settlement` defaults managed nodes to
`no-wallet`, supports `internal-accounting` receipts with Treasury and Nexus
refs, projects accounting receipt evidence, and rejects contributor-wallet mode
inside private Cloud.

### CND-022 Implement Capability Broker Redaction

Repo: `cloud`

Outcome: capability broker logs and receipts prove access without leaking
tokens, API keys, private keys, wallet material, or customer data.

Acceptance:

- Redaction tests cover headers, URLs, env vars, config files, logs, and
  receipt payloads.
- Secret-looking data in fixtures fails tests unless explicitly marked fake.
- Broker never writes raw secrets to artifacts.

Implementation scaffold: `oa-node broker redact` supports headers, URL, env,
config, log, and receipt payload redaction, rejects unmarked secret-looking
fixtures, writes only redacted artifacts, and emits digest-only broker receipts.

### CND-023 Implement Sandbox Profile Enforcement

Repo: `cloud` and `psionic`

Outcome: sandbox jobs run only under declared profiles with explicit network,
filesystem, timeout, artifact, and secret policy.

Acceptance:

- Undeclared network/filesystem access is rejected or receipted as failure.
- Profile digests appear in execution receipts.
- Sandbox compute does not accept open-ended labor requests.

Implementation scaffold: Psionic receipts now carry profile-policy evidence
through `OpenAgentsInc/psionic#1089`. In Cloud, `oa-node sandbox profile
register` records local profile policy, Forge sandbox worker assignments are
accepted only when their declared sandbox policy matches a registered profile,
and sandbox Psionic receipts require `--profile-digest`.

### CND-024 Implement Autopilot Workroom UX Adapter

Repo: `cloud` and `autopilot`

Outcome: Autopilot can create workrooms, show status, manage preview access,
inspect artifacts, and submit acceptance decisions.

Acceptance:

- Autopilot displays workroom state from typed APIs.
- Approval and exposure changes emit receipts.
- User-visible copy does not expose private node topology.

### CND-025 Implement Forge Verification Adapter

Repo: `cloud` and `forge`

Outcome: Forge verifies workroom artifacts, receipts, execution evidence, and
acceptance gates before delivery or settlement.

Acceptance:

- Failed verification blocks acceptance.
- Verification records cite artifact and receipt digests.
- Retry/migration paths preserve evidence continuity.

### CND-026 Implement Compatibility Test Matrix

Repo: `openagents` and `cloud`

Outcome: public Pylon, public schema fixtures, private `oa-node`, and private
`oa-workroomd` agree on boundaries.

Acceptance:

- Fixture tests cover contributor, managed, workroom, degraded, quarantined,
  and no-wallet cases.
- Public Pylon tests pass without private repo access.
- Private cloud tests fail if private fields enter public fixtures.

### CND-027 Implement Local Proof Fleet

Repo: `cloud`

Outcome: local proof environment can run one Nexus/Forge-shaped node and
workroom lifecycle without touching production.

Acceptance:

- `scripts/local-proof.sh` or equivalent runs a create-to-closeout smoke.
- Proof run writes redacted receipts and artifacts.
- Reset deletes only proof namespace state.

### CND-028 Implement Observability And Operator Runbook

Repo: `cloud`

Outcome: managed cloud nodes have logs, metrics, health events, dashboards, and
operator runbooks.

Acceptance:

- Runbook covers install, update, rollback, quarantine, workroom drain, and
  incident evidence export.
- Metrics distinguish node health, backend health, workroom health, and control
  plane health.
- Logs are redacted by default.

### CND-029 Implement Migration Plan From Current Pylon

Repo: `openagents` and `cloud`

Outcome: current Pylon capabilities are mapped to public Pylon, public
`pylon-core`, private `oa-node`, private `oa-workroomd`, Psionic, Forge, Nexus,
and Treasury.

Acceptance:

- No current Pylon behavior is moved without an owner decision.
- Public contributor behavior stays public.
- Managed cloud behavior has private repo issues and test gates.

### CND-030 Cut First Private Cloud Milestone

Repo: `cloud`

Outcome: first milestone proves one managed node can register, advertise
capability, create one private workroom, run a bounded command, expose a
private preview or artifact, close out, and emit receipts.

Acceptance:

- End-to-end smoke runs locally.
- No wallet authority is present in the workroom.
- Root audit, cloud docs, and issue list agree on repo boundaries.

### CND-031 Add Local Cloud MVP Smoke Harness

Repo: `cloud`

Outcome: one local command proves the first private Cloud MVP path without
touching production services.

Acceptance:

- A single command runs node init, workroom start, bounded command execution,
  artifact closeout, and receipt emission.
- The smoke uses local/mock Nexus and Forge adapters.
- Output is redacted and leaves an inspectable local evidence directory.

### CND-032 Add Container Packaging

Repo: `cloud`

Outcome: `oa-node` and `oa-workroomd` can be packaged into reproducible local
container images for GCP testing.

Acceptance:

- Images include version metadata.
- Images do not bake in secrets or local machine paths.
- Local image run supports `status --json` and `doctor --json`.

### CND-033 Add First GCP Bootstrap Runbook And Scripts

Repo: `cloud`

Outcome: a documented `gcloud` path prepares a test environment for the first
Cloud MVP deployment.

Acceptance:

- Runbook covers APIs, Artifact Registry, service accounts, IAM, network rules,
  logs, and cleanup.
- Scripts are idempotent where possible and fail closed on missing project/env.
- No production project is the default.

### CND-034 Deploy `oa-node` To One Test GCE VM

Repo: `cloud`

Outcome: one test VM runs `oa-node` as a managed daemon.

Acceptance:

- VM starts `oa-node` under systemd.
- `oa-node status --json` can be collected over the intended operator path.
- Logs are redacted.
- Destroy path removes the test VM and related temporary resources.

### CND-035 Run First GCP Workroom Smoke

Repo: `cloud`

Outcome: the GCE node can run one no-wallet test workroom end to end.

Acceptance:

- Workroom starts private.
- Workroom runs a bounded command.
- Artifact closeout and receipt emission complete.
- No wallet authority is present inside the workroom.

### CND-036 Add Cloud MVP CI Gates

Repo: `cloud`

Outcome: CI protects the MVP path before deploy scripts are treated as valid.

Acceptance:

- CI runs `cargo fmt --check`.
- CI runs `cargo check`.
- CI runs unit tests once they exist.
- CI runs local smoke or a dry-run smoke once the harness exists.
- CI verifies image build scripts before GCP deployment docs call them ready.

### CND-037 Add Minimal Local/GCP Control CLI

Repo: `cloud`

Outcome: operators have one command surface for local and first GCP test flows.

Acceptance:

- CLI supports status, doctor, deploy test, smoke test, logs, and destroy.
- Commands emit JSON for automation.
- Output redacts secrets and project-specific sensitive values.

### CND-038 Add Redacted Config And Environment Management

Repo: `cloud`

Outcome: local and GCP config can be templated without committing secrets.

Acceptance:

- Example env/config files contain fake placeholders only.
- Real secrets are loaded from local ignored files or cloud secret managers.
- Redaction tests cover env values, URLs, headers, logs, and config dumps.

### CND-039 Add MVP Observability For Local And GCP Tests

Repo: `cloud`

Outcome: first MVP tests expose enough logs and event IDs to debug failures.

Acceptance:

- Node, workroom, ingress, artifact, and receipt events share stable IDs.
- Local logs and GCP logs expose the same event categories.
- Logs avoid raw secrets, wallet material, tokens, and private customer data.

### CND-040 Publish First Cloud MVP Closeout Report

Repo: `cloud`

Outcome: the first local and GCP MVP pass is recorded as a checked-in report.

Acceptance:

- Report includes local smoke command and result.
- Report includes GCP smoke command and result.
- Report includes cleanup status, known gaps, and next implementation tranche.
- Report links back to the audit and issue list.

### CND-041 Bootstrap And Benchmark `oa-shc-katy-01`

Repo: `cloud`

Outcome: the first SHC Katy VPS runs the same managed-node bootstrap path as
the GCP fallback node and produces measured benchmark receipts.

Acceptance:

- SHC SSH access is confirmed before implementation starts.
- `oa-node` installs under the intended service manager.
- Host inventory records CPU, RAM, disk, OS, network, and site metadata.
- Benchmark receipts are labeled `managed_pilot`, measured, and distinct from
  modeled economics.
- Restart, heartbeat expiry, and quarantine/degrade paths are smoked.

Notes:

- The SHC target is `oa-shc-katy-01`.
- The GCP reference target is `oa-gcp-shc-katy-01`.
- Do not treat a successful GCP run as proof of SHC readiness.
- 2026-06-01 measured smoke: SSH works as `ubuntu`, KVM is exposed,
  `scripts/verify-bootstrap.sh` passed, the fake-Codex `oa-workroomd` runner
  test passed, `oa-node` lifecycle/quarantine scaffolds passed in a temporary
  state directory, and a manual Firecracker guest boot returned
  `OA_FIRECRACKER_GUEST_OK`.
- Detailed runbook/result: `docs/bootstrap/CND-041-shc-katy-01-bootstrap.md`.

### CND-042 Compare GCP And SHC Receipts

Repo: `cloud` and `workspace`

Outcome: GCP and SHC runs are compared through receipts, cost notes, and
operator evidence before expanding the SHC pilot.

Acceptance:

- Comparison cites setup receipts, execution receipts, artifact receipts,
  benchmark receipts, and closeout receipts from both nodes.
- Report separates measured GCP cost, measured SHC invoice cost, modeled SHC
  economics, and any unsettled assumptions.
- Report recommends expand, hold, or stop for the SHC pilot.
- Follow-up issues are filed in the correct repos for any missing Nexus,
  Forge, Probe, Psionic, Autopilot, Treasury, or public Pylon integration.

### CND-043 Add Benchmark Cloud Execution Lane For Terminal-Bench 2

GitHub tracking: https://github.com/OpenAgentsInc/cloud/issues/62

Repo: `cloud`, with Vortex companions

Outcome: Cloud has a reusable Benchmark Cloud execution plan with
Terminal-Bench 2 as the first dataset adapter.

Acceptance:

- `docs/BENCHMARK_CLOUD.md` defines the Cloud/Vortex authority split.
- The MVP path targets a Terminal-Bench 2 oracle smoke on Google Cloud Batch.
- The plan keeps Terminal-Bench behind normalized benchmark contracts.
- The plan lists follow-on issues for runner contracts, GCP substrate, Batch,
  Terminal-Bench, OpenAgents/Codex comparison, and SWE/custom adapters.

### CND-044 Add Normalized Benchmark Contracts And Python Runner Skeleton

GitHub tracking: https://github.com/OpenAgentsInc/cloud/issues/63

Repo: `cloud`

Outcome: local benchmark tasks can run through a dataset-neutral Python runner.

Acceptance:

- Fake pass, timeout, and exception tasks write `result.json`,
  `events.jsonl`, and `metadata.json`.
- Result files include run id, task run id, dataset/version, agent/model,
  harness version, status, verifier result, usage placeholders, and artifact
  refs or digests.
- Tests cover required artifact emission and redaction-safe logging.

### CND-045 Provision GCP Benchmark Cloud Substrate

GitHub tracking: https://github.com/OpenAgentsInc/cloud/issues/64

Repo: `cloud`

Outcome: dev GCP setup exists for benchmark runner images, specs, artifacts,
events, service accounts, and narrow IAM.

Acceptance:

- Artifact Registry, Cloud Storage, Pub/Sub, Cloud Build, Batch, Cloud Run,
  and Secret Manager setup is scripted or runbooked.
- Runner service account can read task specs, write artifacts, and publish
  events without production-secret access.
- Cleanup and cost guardrails are documented.

### CND-046 Implement Cloud Batch Benchmark Task Backend

GitHub tracking: https://github.com/OpenAgentsInc/cloud/issues/65

Repo: `cloud`

Outcome: one normalized benchmark task attempt can run on Google Cloud Batch.

Acceptance:

- A fake normalized benchmark task executes through Batch.
- Required artifacts land under the expected Cloud Storage run/task prefix.
- Batch state maps to normalized task states.
- Failed or timed-out started tasks still emit required artifacts.

### CND-047 Run Terminal-Bench 2 Via Harbor Wrapper On Cloud Batch

GitHub tracking: https://github.com/OpenAgentsInc/cloud/issues/66

Repo: `cloud`

Outcome: Terminal-Bench 2 oracle smoke runs as the first dataset adapter.

Acceptance:

- One selected Terminal-Bench 2 oracle task runs on Cloud Batch.
- Harbor/raw artifacts and normalized artifacts are retained.
- Normalized result includes dataset/version, task id, oracle agent identity,
  harness version, verifier result, wall time, and artifact refs.

### CND-048 Add OpenAgents/Codex Benchmark Agent Adapter And Proof Bundle Output

GitHub tracking: https://github.com/OpenAgentsInc/cloud/issues/67

Repo: `cloud`, with Probe/Vortex coordination

Outcome: the same Terminal-Bench task can run through an OpenAgents/Codex-style
agent adapter and produce proof-bundle-ready artifacts.

Acceptance:

- Transcript, commands, verifier output, result JSON, usage, and stop reason
  are captured.
- Proof bundle manifest includes dataset/version, task id, agent/model,
  harness version, retry/timeout policy, artifact digests, and redaction state.
- Raw Codex auth and provider credentials do not appear in artifacts or logs.

### CND-049 Add SWE-Bench And Custom Repo Benchmark Adapter Path

GitHub tracking: https://github.com/OpenAgentsInc/cloud/issues/68

Repo: `cloud`, with Probe/Vortex coordination

Outcome: Benchmark Cloud proves the harness is general by supporting a
SWE-bench-style or custom repo task path after Terminal-Bench smoke works.

Acceptance:

- One custom repo task or SWE-bench instance runs through the normalized runner.
- Patch/diff, transcript, commands, verifier logs, result JSON, and artifact
  manifest are captured.
- Vortex can track the run without dataset-specific state tables.

### CND-056 Cross-OS Cloud-VM Provisioner (qa-runner CloudVm seam)

GitHub tracking: https://github.com/OpenAgentsInc/openagents/issues/6200

Repo: `cloud` (firecracker provisioner), with openagents/qa-runner coordination

Outcome: the qa-runner's typed `CloudVmProvisionerV2` /
`CloudVmHandle` seam (`apps/qa-runner/src/backend.ts`,
provision -> exec -> copyOut -> teardown) has a production cloud-side
implementation backed by firecracker microVMs, exposed over HTTP from
`oa-codex-control` (`POST /v1/cloud-vm/sessions`).

Acceptance:

- Provisioner (`crates/oa-codex-control/src/cloud_vm.rs`) implements
  provision/exec/copy_out/teardown with a deterministic fake lane and a
  KVM-gated live firecracker/jailer lane (owner-gated, default-OFF, honest
  refusal — never falls back to local, never fakes a green).
- HTTP route satisfies the `CloudVmProvisionerV2` wire shape (contract test
  `tests/cloud_vm_contract.rs`); fake-runtime unit tests prove the lifecycle +
  refs-only receipts with no KVM.
- Live boot on a Linux KVM host is the deploy step
  (`docs/bootstrap/CND-056-cloud-vm-firecracker-provisioner.md`); the
  `#[ignore]` live proof drives the on-hardware run. macOS/Windows tiers refuse
  until a host pool exists.
