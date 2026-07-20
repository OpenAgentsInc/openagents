# INVARIANTS

This public monorepo contains managed-cloud infrastructure. Runtime secrets,
live topology, and private customer data remain outside Git. Treat these
invariants as policy until an explicit change updates this file and the
corresponding tests, models, or smoke checks.

## Public / Private Boundary

- Contributor Pylon remains in `apps/pylon` (public monorepo).
- Managed Cloud implementation lives **in this monorepo** under `crates/oa-*`
  and `crates/openagents-cloud-contract` (migrated from private
  `OpenAgentsInc/cloud`. See `docs/cloud/MIGRATION.md`).
- Public schemas and redacted receipts are shared outward. Live fleet topology,
  Secret Manager material, placement credentials, and internal settlement
  secrets stay in runtime env / Secret Manager — never in tracked fixtures,
  logs, or docs.
- The Worker (`apps/openagents.com`) owns non-money admission and public
  projection. Billing, credit, wallet, payout, payment, and settlement
  authority are retired under VP-1. Cloud daemons must not recreate them.

### 2026-07-11 authority-wording boundary

The Vortex/Convex/private-Cloud/Treasury names removed from current Cloud docs
were stale ownership labels, not executable policy. This reconciliation changes
no schema, route, runtime decision, receipt, or deployment. Existing contract
tests, Cloud smokes, and the production-cutover receipt remain the behavioral
evidence. The superseded Convex bridge evaluation is retained as historical
design context only.

## Wallet And Settlement

- Payments, billing credits, markets, Sites, wallets, payouts, and settlement
  are outside the accepted MVP and retired under the
  [`Node/pnpm/Vite Plus conversion contract`](../sol/2026-07-14-node-pnpm-vite-plus-full-conversion-plan.md).
  Paid/credit-backed admission is disabled and must never be reinterpreted as
  free managed compute. Historical redacted receipts and private recovery
  archives may remain read-only. They carry no execution authority. Any
  revival requires a fresh owner-approved invariant and proof program, while
  the no-wallet workroom boundary below remains permanent.
- Contributor-wallet mode is not the default for managed cloud nodes.
- Workrooms do not receive wallet authority by default.
- Workrooms may receive settlement metadata through scoped local gateways.
  They must not receive wallet seeds, node entropy, private keys, preimages,
  bearer tokens, or raw accounting credentials.

## Capability And Secret Handling

- Workrooms consume capabilities through brokers or local gateways, not raw
  provider secrets on disk.
- Every capability attachment is scoped, revocable, auditable, and tied to a
  workroom, program, org, or template policy.
- Secret access must produce redacted evidence that can be audited without
  leaking secret material.
- Portable-session capabilities use `PortableCapabilityBroker` and the frozen
  `PortableCapabilityLease` scope. Every provider, SCM, MCP/tool, or bounded API
  redemption is bound to one owner/session/attachment-generation/target plus a
  least-privilege permission set and bounded TTL. Owner-local and
  OpenAgents-managed targets enter through explicit class-matched adapters.
  The broker retains refs and outcomes only. Raw material may exist solely in
  the injected vault-to-adapter callback. A destination generation receives a

  freshly authorized source-grant ref only after source revocation and target
  wipe. Replay, expiry, broker outage, target denial, or cleanup failure cannot
  mint or reactivate destination authority.

- The enforcing fault oracle is
  `packages/portable-session-contract/src/capability-broker.test.ts`.
- GitHub write tokens may enter a bounded Codex workroom through a short-lived
  run-scoped grant from `github_write_grant_ref`.
  A statically configured operator fallback may use `OA_CODEX_GITHUB_TOKEN`.
  Both paths supply the token only through process environment variables.
  Tokens must not enter commits, git configuration, remote URLs, artifacts,
  callbacks, traces, tracked files, D1, or normal logs.
- A Codex coding run may commit and push its workspace changes back to the
  target repo/branch (`repository_clone_url` / `repository_ref`) before
  workspace teardown (cloud#96 git writeback). Writeback is gated on a write
  token being present and the working tree having changes. Runs without a token
  perform no writeback. The token reaches `git push` only via the askpass helper
  and process environment. The emitted writeback event and `git-writeback.json`
  receipt are refs-only (commit sha + branch ref) and must never carry the
  token.
- Codex workrooms persist every non-streaming-delta Codex event payload by
  default after forbidden-secret marker checks.
  They also persist every detected tool call to the Khala Sync or Worker event
  ingest path.
  Streamed text/tool-output deltas are omitted
  from durable persistence unless a future policy explicitly promotes them.

## Node Lifecycle

- Desired mode and observed status are distinct.
- A managed node that cannot prove backend readiness must degrade or refuse. It
  must not advertise healthy capacity.
- Update, rollback, and quarantine decisions must produce receipts or health
  events.

## Workroom Lifecycle

- A workroom is not a provider wallet and not a public provider persona.
- A workroom starts private by default.
- Ingress, token minting, public exposure, custom-domain binding, capability
  attachment, artifact upload, and closeout are receipt-bearing events.

## Managed Agent Sandbox Contract (SBX-00/01/02/03, #9029/#9034/#9028/#9025)

- `openagents.managed_sandbox.v1` is the sole managed-sandbox domain identity.
  GCE, Firecracker, Box-v1, IDE, mobile, and Sarah records are projections or
  consumers and may not mint, replace, or reinterpret that identity.
- Owner, tenant, program, work unit, attachment generation, and resource
  generation are fixed before a provider effect.
  Target, immutable image digest, profile, lease, budget, capability, and
  idempotency scope are also fixed before that effect. A
  model response, provider object, SDK status, or quiet process is never
  authority or lifecycle truth.
- Lifecycle, lease, guest, filesystem, ingress, runtime, work-admission, and
  cleanup facts remain distinct. Stop requires a durable filesystem
  checkpoint. Delete requires observed cleanup. Uncertainty becomes failed or
  `recovery_required`, never an invented success.
- `PostgresManagedSandboxStore` is the sole durable lifecycle and native event
  authority.
  It records an exact command fingerprint before any provider effect.
  An exact retry returns the stored command state or receipt.
  A receipt retry must match the stored settlement fingerprint.
  Different bytes under the same command or idempotency identity refuse.
- One sandbox may have only one pending command and one accepting resource
  generation.
  Resume fences the old generation before a new generation can accept work.
  Native event sequence remains dense across that generation change.
- Native event cursors and Box compatibility cursors use separate tables and
  versions.
  A compatibility cursor cannot advance beyond or replace native event
  authority.
- Box-v1 is a development/conformance projection that uses exact
  `@asciidev/box-sdk@0.0.24` bytes. Unsupported SDK operations return a typed
  `501 capability_not_implemented`. Projection cursor/omission metadata is
  preserved. The SDK is not a production domain dependency and conveys no OCI,
  Docker, Kubernetes, generic GCP, or generic container-admin claim.
- The Worker `/v1` Box facade is default-off. When enabled, it requires a
  programmatic bearer. The route also resolves the linked owner, derived
  tenant, exact sandbox, and current resource generation before each native
  operation.
- Command retries replay the durable reservation. Changed bytes conflict. Box
  list and event cursors are bounded. Event pages use native order, and a
  cursor from an old generation refuses.
- The compatibility response never publishes provider URLs, IP addresses,
  desktop endpoints, snapshot availability, subdomains, raw topology, or
  credentials. SBX-04 runtime calls use the private control URL and an
  explicitly configured absolute SDK-driver path. Without either dependency,
  prompt/status/events/interrupt fail typed and closed. SBX-05 file, command,
  and artifact calls use the same private URL plus an absolute I/O driver.
- A runtime turn binds the exact scope, command, capability, prompt digest,
  provider, model, harness, and optional reasoning effort before provider
  dispatch. A provider event page must use the same turn and generation with
  a dense per-turn sequence. Replayed bytes are no-ops. Changed bytes and gaps
  conflict.
- Silence is never a runtime event. Only `RuntimeSettled`,
  `RuntimeInterrupted`, or `RuntimeFailed` creates a terminal turn receipt.
  `RuntimeInterruptRequested` remains visible and interrupting until the
  provider emits its terminal event. Service restart may restart the helper.
  No receipt claims provider-private process or session state was snapshotted.
- Guest I/O admits only file read, file write, command, and artifact read. Each
  private request binds the exact owner, tenant, work unit, sandbox generation,
  active capability, retry identity, time, and resource limits.
- Guest paths stay beneath `workspace` through no-follow resolution. Absolute,
  dot, empty, backslash, NUL, parent, or unproven symlink paths refuse.
- The guest command wrapper rebinds the already validated directory at its
  canonical `/workspace` path before execution. A private `/proc/self/fd/*`
  transport path must not become the command's observable working directory.
- A command success requires a closed process tree and zero descendants. It
  also requires clean scratch, closed ingress, and denied egress. CPU,
  duration, output, process count, and network use must remain within bounds.
  The control adapter kills an overdue driver process group.
- Artifact receipts bind content digest, size, source generation, source path
  digest, retention, content type, and evidence refs. Secret or credential
  markers refuse before public projection.
- Sarah's closed managed-sandbox action vocabulary grants no raw `gcloud`,
  shell, database, topology, guest address, service-account, credential,
  filesystem-path, or generic container administration. Runtime mutation must
  continue to refuse until the exact broker and GCP target are deployed,
  healthy, within budget, and receipt-capable.
- Deterministic enforcement lives in
  `packages/managed-sandbox-contract/src/{schemas,lifecycle,box-v1}.test.ts`
  and `packages/khala-sync-server/src/managed-sandbox-store.test.ts`.
  Exact SDK facade, authorization, retry, cursor, unsupported-operation, local
  fake, and loopback HTTP coverage lives in
  `apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts`.
  Authority denial tests live in
  `packages/authority/src/managed-sandbox-authority.test.ts`. Live target
  evidence is separately required by SBX-09 and must not be inferred from
  these contract tests.
- The managed-sandbox provider route is default-off and fail-closed. Only the
  exact `live_gce` profile can report ready. Legacy fake GCE and Cloud-VM
  provisioners cannot satisfy managed-sandbox readiness, and a managed-sandbox
  failure cannot continue on the control host.
- The GCE profile pins immutable image and profile digests.
  It also pins region, machine class, isolation class, provisioner, network
  policy, component identities, zero prewarm, capacity, TTL, and budgets.
  Every pin precedes the provider effect.
  Capacity or quota pressure refuses the request and cannot select a
  substitute.
- A GCE managed-sandbox workload has no external IP, guest service account,
  OAuth scope, ambient provider home, or host path. Network policy is
  default-deny in both directions.
- The only admitted ingress is SSH on port 22 from the reserved control IP
  `/32`. The profile digest binds that control IP. The allow rule pairs the
  exact source with the generation-owned guest target tag. A source service
  account plus target tag is not a substitute because GCP rejects that rule.
- Application egress reaches only the exact private control-broker IP and
  port. Bootstrap may reach only GCE metadata TCP 80 at
  `169.254.169.254/32`. The guest has no service account or OAuth scope.
  Legacy metadata, project SSH keys, and OS Login are disabled. An in-guest
  owner rule denies metadata to the unprivileged workload UID. The generation
  marker cannot emit before that rule exists.
- Higher-priority per-sandbox allow rules sit above explicit deny-all rules.
  All five rules are generation-owned and cleanup-observed. Readiness checks
  each exact destination, port, priority, and target tag.
- The persistent control firewall admits broker traffic from the shared
  managed-guest tag at priority 900. It denies every other source at priority
  1000. The control VM has no external IP. No public or provider endpoint is
  reachable from the guest. Only run-scoped capability refs pass admission.
- The dedicated Cloud Run bridge uses Direct VPC egress and a source tag. Its
  control-port firewall does not admit the enclosing subnet.
- Staging and production have distinct control nodes, private addresses,
  bridge services, tags, firewall rules, control-token secrets, broker signing
  keys, and native database authority. Cross-environment reuse is forbidden.
  Cloud Run IAM does not consume the application authorization exchange. The
  Worker sends the control secret in
  `x-openagents-managed-sandbox-token` only on managed-sandbox runtime paths.
  The bridge accepts that header on no generic route. It compares the secret
  in constant time and privately synthesizes the control `Authorization`
  header.
- Provider credentials remain in the Worker. After native turn admission, the
  Worker mints one HMAC-signed capability. It binds actor, owner, tenant,
  sandbox, generation, turn, and capability ref. It also binds provider,
  requested/effective model, nonce, and expiry.
- The private control proxy
  relays that capability without learning or storing a provider credential.
  Every provider request
  rechecks the native resource, turn, active lease, generation, and capability.
  The Worker then injects its OpenAI key or mints a Vertex access token.
  Revoked, expired, cross-provider, stale-generation, or changed-model tokens
  fail before a provider effect.
- The control component uses a metadata identity and refuses a downloadable
  service-account key. Its HTTP bearer is read from a root-only runtime file
  fetched from Secret Manager. The dedicated deployment never writes the raw
  bearer into instance metadata or tracked configuration.
- Provider ownership and cleanup ownership are durable before effects.
  Readiness requires observed provider, guest-generation, image, address,
  identity, metadata, and network facts. Cleanup requires observed zero
  compute, firewall, disk, ingress, and grant residue. Uncertainty reports
  `recovery_required` and reconciles the same ownership.
- The operational image builder must preserve an empty `/etc/machine-id` file,
  then boot the sealed image once as a private no-identity VM. Image admission
  requires observed DHCP, metadata startup, regenerated SSH host keys, active
  SSH, the workload metadata guard, both pinned agent SDKs, and both guest
  drivers. The smoke must also prove exact metadata v1 networking. A failed
  newly-created image remains unadmitted and is deleted with its exact
  builder/smoke resources.
- Readiness polling must finish within the bridge timeout. A transport timeout
  cannot erase cleanup ownership. The live harness attempts reconciliation.
  If authority is unavailable, it deletes only its deterministic resource
  names and still reports the run as failed.
- Deterministic provider enforcement lives in
  `crates/oa-codex-control/src/managed_sandbox_runtime.rs` and its HTTP
  contract test. The bounded live component harness is
  `scripts/cloud/managed-sandbox-live-acceptance.ts`. SBX-09 must reproduce
  independent cross-surface live evidence before rollout.

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
- The provisioned handle and the provision and cleanup receipts expose only
  references and limits.
  They expose no raw KVM socket paths, tap devices, guest IPs, or SSH keys.
  They also expose no absolute kernel or rootfs paths, credentials, wallet
  material, bearer tokens, or private topology markers.
- A provisioned VM carries no wallet authority.
  A failed acquire or unhealthy boot must tear down any partial jail before
  refusal.
  Teardown must be idempotent and must run after `exec` or `copy_out` failure.
  A VM must never leak.
- PORT-03 managed movement uses the separate authenticated retained route
  `POST /v1/portable-agent-computers/operations`. One exact operation ref is
  byte-idempotent through the durable host journal.
  A staged Firecracker guest remains non-accepting and alive until activation.
  Abort or reclaim destroys the VM and its disposable scratch. Conflicting bytes or attachment
  generations refuse before guest effects.
- The control bearer stays in the
  HTTP Authorization header and may never enter an operation body, journal, or
  receipt. If the daemon is armed for live Firecracker but KVM/images are not
  ready, retained movement refuses and must not substitute the fake lane.
- The live retained guest accepts operations only through the fixed
  `/opt/agent/portable-session-control` binary. Arbitrary caller commands are
  not part of the route. The guest controller must return public-safe receipts.
  host and TypeScript adapters reject raw credentials, paths, process/socket
  details, and topology before persistence or projection.

## Agent Computer In-VM Provider Execution (CX-3, #8547)

- Production readiness is an authenticated runtime fact, not configuration
  inference. `oa-codex-control` reports
  `openagents.agent_computer_readiness.v1` and `ready:true` only when its
  effective provisioner is live. Fake/non-KVM nodes report unavailable. Mobile
  target catalogs and admission gates must consume this proof before exposing
  or launching Agent Computer capacity.
- A mobile `managed_cloud` turn is admitted only from a repository-bound
  thread. Server dispatch issues its owner-scoped Codex grant after the
  durable start claim succeeds, then sends a real `codexTurn` plus broker refs
  to the guest. It never sends both the Codex block and the old hosted
  inference block.
  It never issues grants for a losing queue reader.
  It never silently substitutes another execution target.

- The in-VM Codex turn (`apps/pylon/deploy/agent-computer/turn-runner.ts`,
  work-context `codexTurn` block) executes ONLY under a broker-redeemed,
  owner-scoped provider grant materialized into a per-turn scratch
  `CODEX_HOME` (`provider_credential_policy: broker_only`). A codex turn with
  no materialized grant fails closed (`codex.provider_auth_required`). A
  reclaimed grant is never replayable
  (`canReplayCodexProviderGrantAfterReclaim` returns `false`).
- The in-VM Codex execution fails closed at every stage with typed reasons.
  Missing baked binaries use `codex.binary_missing`, and failed execution uses
  `codex.exec_failed`.
  A failed provider turn uses `codex.turn_failed`.
  Missing exact usage uses `codex.no_exact_usage`, and usage is never fabricated.
  Failed receipt ingest uses `codex.usage_receipt_failed`.
  A turn without an ingested exact usage row is a failed turn.
- The codex child process receives a minimal constructed environment (PATH,
  HOME, plus the materialization's `CODEX_HOME`/auth content), never the
  ambient process environment. Agent bearers and auth material never appear in
  emitted events, result bundles, or logs.
- Owner-subscription-capacity usage receipts cover two exact lane and provider
  pairs.
  The pairs are `codex_app_server` with `pylon-codex-org-capacity` and
  `claude_pylon` with `pylon-claude-org-capacity`.
  These receipts are exact token truth rows.
- Owner-subscription-capacity receipts avoid card or credit metering only after
  server-held grant authority proves a used provider-account grant.
  That grant must match the owner, provider account, and provider kind.
  Caller-supplied lane/provider labels and grant refs alone grant no
  exemption. The Worker then skips the metering hook and answers
  `tokenChargeMetered: false` with
  `tokenChargeSkippedReason: owner_subscription_capacity`
  (`isOwnerSubscriptionCapacityReceipt` in
  `apps/openagents.com/workers/api/src/khala-cloud-runtime-usage-routes.ts`).
- Every other lane/provider combination meters normally.
  This includes a Codex-lane row without the org-capacity provider.
  The skip can never widen into a metering bypass. Missing, unredeemed, revoked, cross-owner,
  wrong-account, or wrong-provider grant evidence is denied before token
  insertion. Authority-store failure is a typed 503.

  Grant/account refs never
  enter the public token event. Compute lifecycle stays separately billed
  through `openagents.resource_usage_receipt.v1`.

- Agent Computer receipt consumers independently require that same exact
  no-charge disposition for owner-subscription capacity.
  A `200` response fails the turn if it reports metering, omits the skip reason,
  or names another reason.
  The failure code is `codex.owner_capacity_charge_disposition_invalid`.
  Recorded token truth alone cannot produce accepted closeout.
- Agent Computer usage retry identity is server-derived from the immutable
  owner/thread/turn/lane/provider/model tuple.
  Executor-supplied `usageRef` is metadata, never idempotency authority.
  A lost-response retry with a fresh client ref returns the same token event.
  It inserts no second row, publishes no second public-counter delta, and reports
  `tokensServedDelta: 0`.
- Subscription capacity is never resold (`subscriptionCapacityResale: false`).
  these lanes serve only the owner the grant is scoped to.

## Portable Session Attachment To Agent Computer (PORT-03, #8748)

- An Agent Computer is a destination runtime beneath the canonical portable
  session.
  It never mints or rewrites session, thread, run, agent, parent-edge,
  transcript, or per-thread cursor identity.
- Movement requires migration `0067`'s durable owner/session execution binding.
  the target must preserve its exact run, repository, and pinned-base refs.
  Legacy unbound rows cannot move, and host paths never supply a fallback.
- The Agent Computer may materialize and verify an exact secret-free
  checkpoint only in staged/non-accepting mode. It must verify checkpoint,
  repository post-image, diff, graph, catalog generation, approvals, artifacts,
  receipts, and per-thread cursors before reporting stage success.
- Every provider, SCM, tool, and API capability is a new PORT-02 lease bound to
  the destination attachment generation. A source auth home, token cache,
  environment, process, socket, or credential-bearing workspace is never a
  checkpoint component and is never copied into the guest.
- PORT-01 Cloud SQL authority advances only after graph-wide source quiescence
  and source grant revoke and wipe.
  It also requires destination redemption, exact target staging, and complete
  source process, scratch, and port cleanup.
  The result is the new sole live attachment.
  The Agent Computer may accept work only after that durable commit.
- A destination rejection or any pre-commit failure leaves no destination
  attachment authoritative, releases any newly issued destination grants,
  retains the source graph fenced, and records `recovery_required`. A lost
  post-commit activation acknowledgement replays the same activation operation.
  it never creates another attachment or accepted parent/child turn.
- The managed target's durable adapter records the exact operation bytes before
  each provisioner effect. The provisioner operation ref is byte-idempotent, so
  a restart may reconcile a pending effect but cannot change its session,
  attachment, generation, checkpoint, resource, or result. Stage remains
  non-accepting. Activation independently reads PORT-01 and refuses until the
  exact destination attachment is the current active generation.
- Failback from a managed target uses the same adapter in source mode.
  Only its exact active generation may quiesce.
  Checkpoint construction requires that durable non-accepting state.
  Reclaim must prove all canonical agents plus process, scratch, and port
  release. Abort is limited to a staged target and
  both abort and reclaim are safe to replay after a process restart.
- Deterministic enforcement lives in
  `packages/khala-sync-server/src/portable-session-move.test.ts` and the
  real-Postgres
  `packages/khala-sync-server/src/portable-managed-agent-computer-target.test.ts`.
  These are not the real-host acceptance receipt.
  #8748 stays open until #8636 is complete.
  A direct owner-local Pylon to accepted Agent Computer to owner-local journey
  must prove live identity, digest, grant, and cleanup behavior.

## Compute Versus Labor

- Bounded sandbox execution is compute only when the runtime profile, inputs,
  policies, expected outputs, resource limits, and receipt semantics are
  declared before execution.
- For bounded Codex workrooms, a stable and complete required-artifact set is
  sufficient for artifact closeout.
  The workroom must declare those required artifacts before execution.
  Closeout may occur even if Codex would otherwise continue with a final message. The
  runner must still emit receipt-bearing artifact evidence and must not treat
  missing artifacts as success.
- `danger_full_access` requires an explicit, externally isolated VM or container
  workroom profile.
  The profile has no wallet authority or broad host and cloud credentials.
  It requires session-scoped provider authorization and cleanup receipts.
- Open-ended planning, tool choice, and semantic outcome delivery belong in
  Forge/Probe and current OpenAgents product paths, not in a hidden cloud-node
  path.

## Placement And Quota Routing

- Lane-agnostic placement is cost-driven (CND-042).
  A non-caller-pinned `Auto` assignment compares eligible lanes when GCE is
  available.
  It uses the measured cost-plus-10% per-VM-second estimate from the CND-042
  report (`docs/benchmarks/2026-06-14-cnd-042-gce-shc-receipt-comparison.md`).
  The binding records `cost_driven = true` with a refs-only `cost_basis`.
- Per owner direction, Google GCE wins ties and near-ties.
  GCE is the preferred lane.
  SHC (`oa-shc-katy-01`) is selected only when it is materially cheaper than GCE.
  The SHC pilot recommendation must also be "expand". The report currently
  recommends HOLD, so cost-driven placement resolves to GCE today.
- Caller pins,
  the GCE-unavailable fallback, and the disabled-cost-driven path
  (`OA_CODEX_PLACEMENT_COST_DRIVEN=false`, policy-driven Google-first) record
  `cost_driven = false` with no `cost_basis`. The cost model rates live in one
  place (`openagents_cloud_contract` `*_RAW_PER_VM_SEC_NANOUSD` constants) and
  carry no raw customer cost.
- A placement assignment must not request wallet authority.
  A placed run inherits the no-wallet VM or workroom boundary.
  `danger_full_access` is the explicit default sandbox inside that boundary.
  It is never an implicit escalation with broad host or cloud credentials.
- Placement bindings and quota/metering records are refs-and-limits only.
  They must not carry raw owner identity, raw customer cost, GCP project IDs,
  instance names, IP addresses, or credentials.
  They must not carry wallet material, bearer tokens, or private topology markers.
  The one permitted cost figure is the contract's modeled infrastructure
  billing input, `cost_input_microusd`.
  It is the cost-plus-10% figure on a `compute_usage` sub-record.
  It is never a customer's billed or settled amount.
- Session/lease/TTL/idle caps come from
  `openagents.compute_quota_routing.v1` defaults unless fleet policy overrides
  them. Placement must honor those caps rather than mint unbounded leases.
- A `cloud-gcp` placement must drive the
  `openagents.gce_capacity_class.v1` lease lifecycle
  (acquire -> ready -> in_use -> release).
  It must provision a per-session VM and run the Codex runner for the assignment.
  It must emit a refs-only `openagents.resource_usage_receipt.v1`.
  It must then release the lease idempotently and mint a cleanup receipt.
- That GCE resource-usage receipt must carry a `compute_usage` sub-record.
  Its `vm_seconds` value must be the measured lease wall-time.
  The measurement is `release_at − acquire_at` in whole seconds and saturates at 0.
  The receipt must contain `metering_source = node_measured`.

  Its `cost_input_microusd` value must be `floor(vm_seconds × cost-plus-10% rate)`.
  Use the shared `LaneCostModel` and `GCE_RAW_PER_VM_SEC_NANOUSD` markup.
  Do not derive a new 1.10 value.

- The rate is the GCP published list-price catalog rate, not a live GCP Billing export.
  Thus, `cost_input_basis` must be `cost_plus_10pct_gcp_catalog`.
  It must never be `cost_plus_10pct_gcp`, which is reserved for a live metered Billing export.
  It must never use a fabricated "metered from billing API" basis.
  `unavailable` requires a `null` cost. Failed acquire/readiness must
  degrade or refuse,
  never advertise a healthy VM.
- Real GCP calls are gated behind config/ADC. A
  fake/dry-run provisioner backs unit tests and any no-cloud environment. The
  live provisioner drives real Compute Engine calls (via `gcloud`) only when
  Application Default Credentials and a configured raw project id are both
  present. Otherwise, it must refuse so the caller falls back.

  A failed acquire or unhealthy probe must remove all partial resources before refusal.
  These resources include the VM and firewall.
  The provisioner must never leak a VM instance.

  Release must be idempotent, tolerate absent resources, and
  verify via a label/name-filtered `instances list` that zero session VMs
  remain. It must report a clear defect if resources remain.

- The live lane is opt-in via env and ADC.
  The default provisioner is fake so no-cloud envs and unit tests never bill.
  Raw GCP project id / zone / instance name may be used only transiently at
  provisioning time and must never be retained.
  Lease projections and provision and cleanup receipts contain only references
  and limits.
  They reject raw GCP project ids, instance names, self-links, IP addresses, and
  SSH keys.
  They also reject credentials, wallet material, bearer tokens, and private
  topology markers.
