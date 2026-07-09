# Cloud repo open-source consolidation plan

Date: 2026-07-08
Status: implementation landed; Phase 6 control-plane cutover executed 2026-07-09 (see receipts/)
Tracking: #8591
Source commit: f87a60c3a7600ae377ec392052f8d85dcc9af421
Scope: move the non-deprecated OpenAgents Cloud infra that still lives in the
private `OpenAgentsInc/cloud` repo into the public `openagents` monorepo, then
retire the private repo as an implementation dependency.

## Executive decision

OpenAgents Cloud should no longer be a separate closed-source codebase. The
private repo currently owns real product-critical infrastructure for Khala Code
Agent Computers, cloud QA, placement, resource receipts, capability brokering,
and managed workroom execution. Those are now core OpenAgents product surfaces,
not private side experiments.

The migration should move code, contracts, fixtures, tests, Dockerfiles, and
operator runbooks into `openagents/`. Secrets, raw topology, service-account
keys, live tokens, private customer data, raw prompts, private repo contents,
wallet material, and host-local paths do not move. Those remain in Secret
Manager, local `.secrets/`, runtime environment, or public-safe refs.

## Current openagents dependency map

`openagents` already treats the private Cloud repo as the missing production
implementation behind public seams:

- `docs/khala-code/2026-07-06-agent-computers-strategy.md` names the private
  `cloud/` repo as the owner of `oa-node`, `oa-codex-control`, the Firecracker
  microVM provisioner, placement API, lifecycle events, and cleanup receipts.
- `apps/openagents.com/workers/api/src/cloud/cloud-coding-session-routes.ts`
  already exposes the flag-gated Agent Computer launch surface. It validates
  lane, repo trust tier, admission, work-context echo, lifecycle refs, resource
  usage refs, and cleanup refs, but it fails closed until a real control-plane
  adapter is armed.
- `apps/pylon/deploy/agent-computer/` contains the public host bootstrap and
  image-manifest lane for nested virtualization, `/dev/kvm`, image digests, and
  the proven in-microVM deterministic turn. It still points remaining control
  plane work back to `cloud/`.
- `apps/qa-runner/src/backend.ts` defines the `CloudVmProvisionerV2` seam and
  explicitly says the real Firecracker implementation lives in `cloud`.
- `scripts/qa-async-gce-trigger.ts` posts
  `openagents.codex_placement_assignment.v1` assignments to the Cloud control
  plane for owned async QA.
- `apps/pylon/src/cloud-control-client.ts` and adjacent tests already model the
  control-plane client side for placement, event streaming, cancellation, and
  GCE lease lifecycle events.
- `packages/oa-infra`, `packages/khala-sync`, `packages/khala-sync-server`,
  `apps/openagents.com/workers/api/src/cloud/*`, and Aiur now hold the public
  storage, sync, metering, and operator-console side of the same system.

That means the private repo is no longer only internal operations glue. It is
the other half of product code that is already documented, tested, billed, and
projected from the public monorepo.

## Current cloud repo inventory

Active implementation that should move:

- `crates/openagents-cloud-contract`: Rust contract constants and validators for
  cloud node, workroom, Codex auth grant, Codex workroom assignment/events,
  placement assignment, quota routing, GCE capacity class, resource usage
  receipts, and lane cost model.
- `crates/oa-codex-control`: HTTP control plane for placement, Codex runs,
  queue dispatch, grant resolution, event ingest, GitHub writeback grants, GCE
  capacity leases, and Cloud-VM provisioning.
- `crates/oa-codex-control/src/gce_capacity.rs`: GCE per-session capacity-class
  lifecycle, including fake/live provisioners, lease refs, cleanup refs, and
  no-secret receipt discipline.
- `crates/oa-codex-control/src/cloud_vm.rs`: owner-gated Firecracker
  `openagents.cloud_vm_provisioner.v1` implementation for provision, exec,
  copy-out, and teardown.
- `crates/oa-node`: managed node daemon for readiness, capability detection,
  assignment intake, service management, quarantine, update status, sandbox
  profile enforcement, and receipt submission.
- `crates/oa-workroomd`: VM/workroom sidecar for metadata, link-local gateways,
  Codex auth materialization, Codex run execution, artifact closeout, ingress,
  lifecycle, required-artifact closeout, token-safe Git writeback, cleanup, and
  resource receipts.
- `crates/oa-cloud-run-bridge`: Cloud Run bridge code where it remains useful
  for current owned services or deploy ergonomics.
- `docs/contracts/*`: contract docs for cloud nodes, workrooms, placement,
  Codex grants, GCE capacity, resource usage receipts, tenant isolation,
  credential brokering, inference gateway, settlement bridge, and worker
  attachments.
- `docs/oa-node/*`, `docs/oa-workroomd/*`, `docs/control/*`,
  `docs/bootstrap/*`, and `docs/benchmarks/*`: operator and architecture docs
  that describe active Cloud behavior, as long as they are redacted and updated
  away from deprecated authority names.
- `fixtures/*`: public-safe contract fixtures and regression examples after a
  redaction scan.
- `docker/*`, `scripts/*`, `config/*.env.example`, and the Python benchmark
  runner where they are active deployment/test paths.

Material to drop, rewrite, or quarantine during migration:

- Proprietary repo metadata and license declarations. New files must inherit the
  openagents licensing posture.
- Deprecated authority naming that says Vortex, Nexus, Autopilot, or Treasury
  own current control, registry, product UX, settlement, or durable state. Where
  still semantically needed, rewrite to current OpenAgents surfaces:
  `openagents.com` Worker, Khala Sync, Pylon, Aiur, Forge, and MDK/Nexus bridge
  only for currently active payout boundaries.
- Any route, doc, fixture, or test that embeds raw GCP project ids, instance
  names, guest IPs, KVM socket paths, SSH keys, provider secrets, bearer tokens,
  wallet material, private topology, raw prompts, raw logs, or private repo
  content.
- Historical benchmark/bootstrap notes that are only source material and not
  current operational runbooks. Keep them only if they can be clearly marked as
  historical and public-safe.

## Target openagents layout

Use one public monorepo source of truth:

- `crates/openagents-cloud-contract/`
  Rust contract crate copied from `cloud/crates/openagents-cloud-contract`.
  Keep this Rust crate as the low-level validator for the Rust daemons, and add
  generated or manually mirrored Effect Schema contracts under
  `packages/cloud-contract/` only where TypeScript callers need them.
- `crates/oa-codex-control/`
  Control-plane binary and tests. Keep fake provisioners as default. Live GCE
  and Firecracker remain explicit env-gated modes.
- `crates/oa-node/`
  Managed node daemon and tests.
- `crates/oa-workroomd/`
  Workroom sidecar and tests.
- `crates/oa-cloud-run-bridge/`
  Only if still active after the Google/Cloudflare consolidation work. If it is
  purely transitional, mark it as historical and do not wire it into new
  production paths.
- `docs/cloud/contracts/`
  Move active contract docs here, with public-safe examples and links back to
  TypeScript/Rust schema owners.
- `docs/cloud/oa-node/`, `docs/cloud/oa-workroomd/`, `docs/cloud/control/`,
  `docs/cloud/bootstrap/`, `docs/cloud/benchmarks/`
  Move active operator docs here. Keep deployment facts as refs and commands,
  not private topology dumps.
- `fixtures/cloud/`
  Move public-safe Cloud fixtures here, grouped by contract version.
- `apps/pylon/deploy/agent-computer/`
  Remains the public Agent Computer host/image lane. Replace references to the
  private repo with relative links to `crates/oa-codex-control`,
  `crates/oa-node`, and `crates/oa-workroomd`.
- `apps/openagents.com/workers/api/src/cloud/`
  Remains the public Worker admission, metering, and projection surface. It
  should import shared TypeScript contract helpers rather than re-declaring
  shapes when practical.
- `packages/oa-infra/`
  Remains shared storage/queue/stream primitives. Cloud daemons should use it
  only through explicit service APIs or generated config, not by assuming Worker
  internals.

Do not bury Cloud under Pylon. Pylon is contributor/local runtime software and
the software inside an Agent Computer image. The control plane, node daemon,
workroom sidecar, capacity class, and receipts are OpenAgents Cloud
infrastructure and deserve first-class homes.

## Migration phases

### Phase 0: freeze and scrub

1. Mark the private `cloud` repo read-only for feature work except emergency
   fixes.
2. Record the exact source commit used for migration in this doc or a sibling
   receipt.
3. Run a redaction audit over candidate files for secrets, raw topology,
   private paths, private repo contents, raw prompts, and wallet material.
4. Classify every doc as `active`, `historical-source`, or `drop`.
5. Open a tracking issue that lists the migration units below and blocks new
   private Cloud-only implementation work.

Exit gate: no candidate file contains forbidden material, and the migration set
is explicit.

### Phase 1: move contracts and fixtures first

1. Add `crates/openagents-cloud-contract/` to the openagents workspace without
   changing runtime behavior.
2. Move active `docs/contracts/*` into `docs/cloud/contracts/`.
3. Move public-safe fixtures into `fixtures/cloud/`.
4. Add an openagents check that validates all moved fixtures with the Rust
   contract crate.
5. Add TypeScript mirror contracts for Worker/Pylon shapes that are currently
   manually duplicated, starting with:
   `openagents.codex_placement_assignment.v1`,
   `openagents.cloud_vm_provisioner.v1`,
   `openagents.gce_capacity_class.v1`,
   `openagents.resource_usage_receipt.v1`,
   `openagents.agent_computer_isolation_policy.v1`.

Exit gate: openagents can validate Cloud contracts and fixtures without reading
the private repo.

### Phase 2: move the control plane in fake mode

1. Add `crates/oa-codex-control/` with fake GCE and fake Cloud-VM provisioners
   as the default.
2. Port its tests and keep no-cloud environments fake by default.
3. Wire `scripts/qa-async-gce-trigger.ts` tests to the in-repo contract crate or
   fixture set.
4. Add a local smoke that starts `oa-codex-control` on loopback and exercises:
   placement start, event read, cancellation, fake GCE acquire/release, fake
   Cloud-VM provision/exec/copy-out/teardown.
5. Update `apps/pylon/src/cloud-control-client.ts` docs/tests to point at the
   in-repo control plane.

Exit gate: the openagents repo can run a complete fake control-plane lifecycle
from source.

### Phase 3: move Agent Computer VM support

1. Port `cloud_vm.rs` live Firecracker code behind the existing opt-in env.
2. Replace the remaining `cloud/` references in
   `apps/pylon/deploy/agent-computer/README.md` with in-repo paths.
3. Implement the already-proven vsock guest protocol in the in-repo
   `oa-codex-control` Cloud-VM path.
4. Connect `POST /v1/placement` to the Firecracker Agent Computer path for
   work-context-bound runs.
5. Ensure every lifecycle event echoes the Worker-required work-context ref and
   cleanup receipts: scratch wipe and microVM destruction.

Exit gate: one owner-gated staging run boots a Firecracker microVM from the
public image contract, runs the deterministic in-guest turn, streams public-safe
events, copies out a result, and tears down with cleanup receipts.

### Phase 4: move workroom execution

1. Add `crates/oa-workroomd/` and port tests for metadata, gateways, ingress,
   lifecycle, artifact closeout, Codex auth, Codex run, writeback, and resource
   receipts.
2. Replace deprecated Vortex grant/event wording with current generic grant
   resolver, event ingest, Khala Sync, and Worker terminology.
3. Keep auth materialization strictly grant-ref based. No raw auth JSON in
   state, receipts, logs, fixtures, or docs.
4. Keep writeback token handling process-env/askpass only. Receipts carry branch
   refs and commit shas, never tokens.
5. Teach the Agent Computer image build to include the in-repo workroomd binary.

Exit gate: `oa-workroomd` can execute the deterministic turn and a mocked Codex
turn in an isolated workroom with redacted receipts and full cleanup.

### Phase 5: move managed node operations

1. Add `crates/oa-node/` and port tests for admin store, capability detection,
   assignment intake, service manager, settlement modes, sandbox profiles,
   update manager, quarantine, Probe/Psionic worker attachments, and registry
   projection.
2. Rewrite node docs so current authority is OpenAgents/Khala surfaces, not
   deprecated Nexus/Vortex/Treasury.
3. Keep public Pylon boundaries intact: contributor wallet UX and local Pylon
   operations stay in `apps/pylon`; managed node internals stay in Cloud crates.
4. Move Dockerfiles and public env templates.
5. Add deploy scripts that read secrets only from runtime env or Secret Manager.

Exit gate: a managed node can report readiness, refuse/degrade honestly, accept
fake assignments, and emit redacted receipts from the openagents repo.

### Phase 6: production cutover

1. Build images from `openagents` source only.
2. Deploy staging control plane, node daemon, and workroom sidecar from
   openagents images.
3. Point `OA_CLOUD_CONTROL_URL` at the staging openagents-built control plane.
4. Run the #8503 DoD path: one real mobile-dispatched Khala Code turn inside a
   Firecracker Agent Computer, with lifecycle, compute, token, writeback, and
   cleanup receipts.
5. Promote production after staging receipts are public-safe and owner-approved.
6. Archive the private `cloud` repo or leave it as a read-only historical mirror
   with a README that points to `openagents`.

Exit gate: production Agent Computer infrastructure is built from public
openagents source, and the private repo is no longer in the deploy path.

## Invariant updates required during implementation

This planning doc does not itself change runtime behavior, but the migration
will require invariant updates when code lands:

- Root `INVARIANTS.md`: remove the claim that managed cloud-node
  implementation lives in the private `cloud` repo.
- `openagents/INVARIANTS.md`: add OpenAgents Cloud as an in-repo authority area
  with explicit Agent Computer, workroom, capability, receipt, quota, and
  no-secret projection boundaries.
- `apps/openagents.com/INVARIANTS.md`: keep the Worker as admission, billing,
  projection, and public product authority; the Cloud daemons execute and emit
  receipts but do not own user credit, public claims, or wallet/payout
  authority.
- New `crates/*/INVARIANTS.md` or `docs/cloud/INVARIANTS.md`: preserve the
  moved cloud invariants for node lifecycle, workroom lifecycle, capability and
  secret handling, placement/quota routing, VM cleanup, and compute-versus-labor
  boundaries.

## Verification matrix

Run these before each phase exits:

- Rust unit tests for every moved crate.
- Fixture conformance tests for every moved Cloud contract.
- Secret/topology scanner over moved docs, fixtures, logs, and receipts.
- Fake control-plane lifecycle smoke.
- Fake GCE lease acquire/release smoke.
- Fake Cloud-VM provision/exec/copy-out/teardown smoke.
- Live Firecracker smoke on an owner-gated nested-virt host.
- Worker route tests for admission, work-context echo, cleanup receipt
  validation, metering outcomes, and typed refusals.
- Pylon cloud-control-client tests.
- End-to-end staging run for #8503 with public-safe receipts only.

## Open questions

- Whether to keep the Rust daemons as first-class `crates/*` or place them under
  `apps/cloud/*`. Prefer `crates/*` because they are binaries plus shared
  contract libraries, and it keeps them independent from Pylon.
- Whether `oa-cloud-run-bridge` still has a non-deprecated role after the
  Google/Cloudflare consolidation. If not, document it as historical and do not
  migrate it into the production build graph.
- Whether Cloud contract TypeScript mirrors should be generated from Rust
  schemas or hand-maintained with fixture conformance. Start hand-maintained for
  speed, then generate once the Rust/Effect Schema boundary stabilizes.
- The owner-approved nonzero Agent Computer compute rate. The metering rail is
  ready for exact receipts, but price remains owner-gated.

## Post-landing review (2026-07-08)

The code move landed in `033386dc75` (openagents), with the private repo
retired for new work and the root workspace routing updated in the same wave.
A follow-up review against `docs/fable/MASTER_ROADMAP.md` (rev 6) confirmed
the plan's assumptions and resolved its first open question:

- **Rust vs Effect boundary (confirmed correct, now explicit).** The Effect
  Native full-conversion mandate (§EN, rev 6) is a **UI-substrate** mandate —
  its definition of done is "every UI surface renders from the catalog." The
  Cloud daemons are systems infrastructure (Firecracker/vsock microVMs, GCE
  capacity leases, managed-node lifecycle) with no UI surface, so they stay
  Rust crates and are **not** conversion targets. TypeScript callers use the
  Effect Schema mirrors in `packages/cloud-contract` and the documented HTTP
  contracts; they never link the crates. Worker admission/billing/projection
  authority and Pylon contributor/local runtime stay on Bun/Effect.
- **Cargo-workspace rule amended.** The repo-level "do not reintroduce the
  old Cargo workspace" rule in `AGENTS.md` predated this migration and
  contradicted it; it is now amended to carve out the Cloud crates only
  (Tauri stays banned; no new non-Cloud Rust surfaces without owner
  direction).
- **Layout question resolved:** `crates/*`, as the plan preferred.
  `oa-cloud-run-bridge` migrated as historical (`HISTORICAL.md`), outside new
  production paths.
- **Residual seams closed in review:** `apps/pylon/src/cloud-control-client.ts`
  header repointed from private-repo paths to `crates/oa-codex-control` and
  `docs/cloud/*`; `scripts/qa-async-gce-trigger.ts` now imports its placement
  contract version from `packages/cloud-contract` instead of re-declaring it.
- **Still open after this review:** Phase 6 production cutover (staging
  `OA_CLOUD_CONTROL_URL` + the #8503 DoD run, owner-gated); wider dedup of
  manually declared contract constants in Worker/Pylon onto the mirror
  package (e.g. `CLOUD_PLACEMENT_CONTRACT_VERSION` in the Pylon client, which
  needs a package-dependency edge rather than a root-script relative import);
  and a named fake control-plane loopback smoke script (the fake lifecycle is
  currently proven by the crate test suites rather than one runnable smoke
  entry point).

## Definition of done

The migration is complete when:

- no production build, deploy, smoke, or runbook requires checking out
  `OpenAgentsInc/cloud`;
- Agent Computer, Cloud-VM, GCE capacity, node, workroom, capability, and
  receipt code build from `openagents`;
- all moved docs and fixtures are public-safe;
- `cloud/` is archived or read-only historical source;
- the first production Khala Code Agent Computer turn runs from the public
  monorepo-built control plane and records lifecycle, compute, token, writeback,
  and cleanup receipts without leaking private material.
