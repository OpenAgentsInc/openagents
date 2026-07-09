# `openagents.tenant_isolation_policy.v1`

Status: design contract for multi-tenant isolation and abuse controls required
before external launch of managed remote execution

This contract specifies the isolation model, spend and session caps, egress
policy, acceptable-use policy, abuse detection, kill-switch, audit
requirements, and retained-projection rules that govern all external tenants
running managed remote execution on OpenAgents Cloud. No session for an external
tenant may be provisioned until each requirement in this contract is met.

This is private managed-cloud infrastructure. Public contributor Pylon should
only see refs, digests, and public-safe tenant shape; it must not learn raw
tenant identifiers, internal user ids, email addresses, IP addresses,
WorkOS-internal cursor tokens, raw org ids, credentials, fleet topology, or
placement details.

## Purpose

`openagents.tenant_isolation_policy.v1` gives Cloud a complete pre-launch
checklist and ongoing enforcement contract for:

- authenticating external tenants via WorkOS and mapping them to stable,
  redacted refs;
- isolating each session in a dedicated ephemeral VM (v1) or microVM (v2+)
  with no shared kernel or shared storage across tenants;
- capping per-tenant spend, active sessions, and lease slots before allocation;
- restricting outbound network access to a declared allowlist per session;
- prohibiting classes of use that violate platform policy;
- detecting abuse patterns and providing a synchronous kill switch;
- producing redacted, auditable evidence for every enforcement decision;
- retaining only refs-only projections that contain no raw tenant identifiers,
  IPs, or credentials.

## Invariant Binding

This contract is a direct application of the `INVARIANTS.md` capability and
secret handling rules and the `danger_full_access` externally-isolated-VM rule.
Both must be satisfied before any external session is admitted.

### Capability and Secret Handling

From `INVARIANTS.md`:

- Workrooms consume capabilities through brokers or local gateways, not raw
  provider secrets on disk. Every tenant-scoped capability attachment is
  scoped, revocable, auditable, and tied to a workroom, program, org, or
  template policy.
- Secret access must produce redacted evidence that can be audited without
  leaking secret material.
- GitHub write tokens, model API keys, and any other tenant-origin secrets
  must not be persisted to git remotes, artifacts, callbacks, traces, tracked
  files, D1, or normal logs.
- Durable event persistence must run the forbidden-secret marker checks used by
  Codex and gateway paths before any non-secret event is retained.

This contract extends those rules to cover tenant identity material: WorkOS
tokens, raw org ids, raw user ids, internal email addresses, and IP addresses
are treated as secret material for all retained-projection and audit-evidence
purposes.

### Externally Isolated VM Rule

From `INVARIANTS.md`:

> `danger_full_access` is allowed only as an explicit externally isolated
> VM/container workroom profile with no wallet authority, no broad host/cloud
> credentials, session-scoped provider auth, and cleanup receipts.

This contract enforces that rule for every external session. Sessions running
under `danger_full_access` are only admitted when:

- the session VM is externally isolated (no shared kernel or storage with any
  other session or tenant);
- the workroom carries no wallet authority;
- the workroom carries no broad host or cloud credentials;
- provider auth is session-scoped and expires with the VM lease;
- a cleanup receipt is required before the session is declared closed.

## Tenant Identity

### WorkOS as the Identity Anchor

All external tenants are authenticated via WorkOS. The Cloud control plane
accepts WorkOS-issued session tokens, verifies them against the WorkOS JWKS
endpoint, and maps the verified claims to an internal tenant ref. No
WorkOS-internal cursor tokens, raw org ids, raw user ids, or raw session token
material are retained in Cloud state or propagated to fleet components.

The control plane stores only:

| Field | Purpose |
| --- | --- |
| `tenant_ref` | Stable, opaque, non-guessable ref derived from the verified WorkOS org claim. Not the raw WorkOS org id. |
| `identity_session_ref` | Non-secret rendezvous ref for this authentication session. Not derived from the WorkOS token. |
| `org_tier` | Bounded enum: `free`, `team`, `enterprise`, or `internal`. Drives cap selection. |
| `plan_policy_ref` | Ref to the active plan and cap policy for this tenant. |
| `auth_evidence_digest` | Digest of the redacted authentication evidence record. |
| `auth_issued_at_ms` / `auth_expires_at_ms` | Authentication window. Must not outlive the WorkOS session. |

Raw WorkOS org ids, user ids, email addresses, IP addresses, raw session tokens,
refresh tokens, and authorization headers must not be retained in Cloud state,
fleet components, receipts, or projections.

### Tenant Ref Derivation

`tenant_ref` is a non-secret, stable, opaque string computed by the control
plane using a keyed HMAC or similar deterministic mapping from the verified
WorkOS org claim. The key is a Cloud-managed secret that never leaves the
control plane. The same WorkOS org maps to the same `tenant_ref` across
sessions.

`tenant_ref` may appear in fleet state, receipts, projections, and Vortex
ingest. Raw WorkOS org ids may not.

### Identity Verification Receipt

Every successful tenant authentication emits a redacted verification receipt:

| Field | Purpose |
| --- | --- |
| `receipt_kind` | `identity_verified` |
| `tenant_ref` | Stable tenant ref, not a raw org id. |
| `identity_session_ref` | Non-secret session ref for this auth event. |
| `org_tier` | Bounded tier enum. |
| `plan_policy_ref` | Active plan policy ref. |
| `decision` | `accepted` or `denied`. |
| `reason` | Redacted bounded reason string. |
| `evidence_digest` | Digest over non-secret receipt evidence only. |
| `emitted_at_ms` | Receipt emission time. |
| `receipt_digest` | Local `sha256:` digest over the redacted receipt material. |

Forbidden receipt fields: raw WorkOS org ids, user ids, email addresses, IP
addresses, raw session tokens, authorization headers, and any field that
contains or is derived from WorkOS-internal secret material.

## Per-Tenant Session Isolation

### V1: One Ephemeral Full VM Per Session

For external launch (v1), each session is assigned exactly one dedicated,
ephemeral Compute Engine VM provisioned via `openagents.gce_capacity_class.v1`.
No two sessions — whether from the same tenant or different tenants — share a
VM, a kernel, a disk, a filesystem, or a network interface.

Isolation guarantees in v1:

- Each VM is created fresh for the session and destroyed after release.
  Persistent or recycled VMs are not permitted for external tenant sessions.
- VMs for different tenants are separated by GCP project-level or VPC-level
  network policy such that no tenant VM can initiate or receive connections
  from another tenant VM.
- The managed firewall rule applied via `openagents.gce_capacity_class.v1` is
  session-scoped, tenant-scoped, and removed on release.
- Boot images are Cloud-managed and verified against a declared image digest
  before the session is admitted to `ready` state.
- No host-level volumes, bind mounts, or shared devices are exposed to the
  workroom process.
- The session VM carries no credentials for other tenants and no Cloud-wide
  credentials that would permit it to provision or access resources outside its
  own session boundary.

### V2+: MicroVM (Firecracker)

A later revision of this contract will permit Firecracker microVM isolation as
a lower-latency alternative to full Compute Engine VMs, provided:

- each microVM runs on a dedicated bare-metal or physical-host-isolated worker
  node that is not shared with other tenants at the hypervisor level;
- the microVM lifecycle, firewall, and cleanup receipt requirements in
  `openagents.gce_capacity_class.v1` (or a successor microVM capacity class)
  are fully satisfied;
- the same no-shared-kernel, no-shared-storage, and no-cross-tenant-network
  guarantees apply.

Until a microVM capacity class contract is ratified and the corresponding
tests and smoke checks pass, all external sessions must use full VM isolation.

### Session-to-VM Attachment

A session is attached to exactly one VM via a declared session ref. The
attachment is receipt-bearing.

| Field | Purpose |
| --- | --- |
| `session_ref` | Stable non-secret session ref. |
| `tenant_ref` | Stable tenant ref. |
| `lease_ref` | Capacity lease ref from `openagents.gce_capacity_class.v1`. |
| `workroom_ref` | Redacted workroom ref for the attached session. |
| `isolation_profile_ref` | Ref to the isolation profile that was verified before attach. |
| `attach_receipt_ref` | Digest of the session attach receipt. |

A session may not be attached to a VM that already has an attached session,
regardless of tenant. A VM that reaches `in_use` state under one session
must not be reused or re-offered to any other session before a cleanup receipt
is issued.

## Per-Tenant Spend Caps

### Spend Cap Enforcement

Spend caps are evaluated synchronously before session allocation. A session
request that would cause a tenant to exceed any active cap must be denied with
a `cap_exceeded` decision and a redacted denial receipt. The session is not
allocated, and no VM is provisioned.

Spend caps are enforced in layers:

| Cap Layer | Scope | Enforcement Point |
| --- | --- | --- |
| Per-session resource budget | Single session VM | Before VM acquire; re-checked at heartbeat |
| Per-tenant daily compute budget | Tenant × calendar day (UTC) | Before VM acquire |
| Per-tenant monthly compute budget | Tenant × calendar month (UTC) | Before VM acquire |
| Org-tier hard ceiling | All sessions under `org_tier` | Before VM acquire |

### Cap Fields

| Field | Purpose |
| --- | --- |
| `tenant_ref` | Stable tenant ref. |
| `plan_policy_ref` | Ref to the active plan and cap policy. |
| `session_budget_ref` | Ref to the declared per-session resource budget. |
| `daily_budget_limit_usd_micros` | Maximum per-tenant daily spend in microusd. |
| `monthly_budget_limit_usd_micros` | Maximum per-tenant monthly spend in microusd. |
| `current_daily_spend_usd_micros` | Observed spend for the current UTC day. |
| `current_monthly_spend_usd_micros` | Observed spend for the current UTC month. |
| `cap_check_receipt_ref` | Digest of the cap evaluation receipt for this request. |

Cap check receipts must not contain raw spend amounts for other tenants, raw
VM cost breakdowns, raw GCP billing line items, or tenant identity material
beyond `tenant_ref`.

### Spend Cap Denial Receipt

| Field | Purpose |
| --- | --- |
| `receipt_kind` | `cap_check` |
| `tenant_ref` | Stable tenant ref. |
| `session_ref` | Non-secret session ref for the denied request. |
| `cap_layer` | Which cap layer triggered denial: `session`, `daily`, `monthly`, or `tier_ceiling`. |
| `plan_policy_ref` | Active plan policy ref. |
| `decision` | `accepted` or `denied`. |
| `reason` | Bounded redacted reason string. |
| `evidence_digest` | Digest over non-secret cap evaluation evidence. |
| `emitted_at_ms` | Receipt emission time. |
| `receipt_digest` | Local `sha256:` digest over the redacted receipt. |

## Per-Tenant Active-Session and Lease Caps

### Session Concurrency Cap

Each tenant is subject to a maximum number of simultaneously active sessions.
A session is active from the moment its VM lease enters `in_use` until the VM
lease cleanup receipt is issued.

| Cap | Default (free) | Default (team) | Default (enterprise) |
| --- | --- | --- | --- |
| Max concurrent active sessions | 1 | 5 | configurable, ≥ 10 |
| Max pending session requests in queue | 2 | 10 | configurable |
| Max total leases held (active + ready) | 2 | 12 | configurable |

Requests that exceed the concurrency cap are denied synchronously. They are not
queued unless the tenant tier permits queuing and the pending queue has
capacity.

### Lease Slot Cap

The total number of VM leases in any non-released state (`acquire`, `ready`,
or `in_use`) held by a single tenant must not exceed the per-tenant lease slot
cap. Leaked or stale leases count against the cap until their cleanup receipt
is issued. The Cloud reconciler must treat expired leases as released and issue
cleanup receipts promptly to free slot capacity.

### Session and Lease Cap Receipt

Every concurrency or lease-slot denial produces a receipt following the same
shape as the spend cap denial receipt, with `receipt_kind` `session_cap_check`
and `cap_layer` values `concurrency` or `lease_slots`.

## Egress Policy

### Outbound Allowlist

Session VMs must not make unrestricted outbound internet connections. Egress is
restricted by a session-scoped outbound allowlist enforced by the managed
firewall rule and, where available, by `oa-workroomd` gateway policy.

The default allowlist for an external tenant session permits:

| Destination Class | Permitted Endpoints |
| --- | --- |
| Managed DNS resolver | Cloud-controlled recursive resolver only; no direct port-53 to arbitrary hosts |
| Model provider APIs | Declared provider domains from `allowed_model_endpoints` in the plan policy, e.g. `api.anthropic.com`, `api.openai.com` |
| OpenAgents local gateways | Link-local gateway paths under `/openagents/` on the node sidecar |
| Cloud artifact store | Declared artifact sink ref from the workroom policy |
| Package mirrors (optional) | Declared, content-addressed package mirror refs in the session policy; not enabled by default |
| GitHub (Codex workrooms only) | `github.com` and `api.github.com` only when a scoped Codex auth grant is active |

All other outbound destinations are denied by default. Requests to denied
destinations are dropped and logged as a redacted egress-denied event. They do
not produce an error that reveals the destination to the workroom process beyond
a standard connection-refused response.

### Cross-Tenant Egress Prohibition

A session VM must not be able to initiate or accept connections to or from any
other tenant's session VM. The managed firewall rule must enforce this at the
network level, not solely at the application level.

### Egress Policy Ref

| Field | Purpose |
| --- | --- |
| `egress_policy_ref` | Ref to the declared egress allowlist for this session. |
| `allowed_model_endpoints_ref` | Ref to the declared model provider endpoint set. |
| `artifact_sink_ref` | Ref to the permitted artifact destination. |
| `dns_resolver_ref` | Ref to the managed resolver policy. |
| `egress_policy_receipt_ref` | Digest of the egress policy apply receipt. |

Egress policy receipts must not contain raw IP addresses, CIDR ranges, raw
domain names beyond the declared bounded allowlist, or private topology
markers.

### Egress Denied Events

Egress-denied events are appended to `egress-denied.jsonl` on the session VM
and forwarded to the tenant's redacted audit trail. Each event records:

| Field | Purpose |
| --- | --- |
| `event_kind` | `egress_denied` |
| `session_ref` | Non-secret session ref. |
| `tenant_ref` | Stable tenant ref. |
| `destination_class` | Coarse destination class: `ip`, `domain`, `port`, or `unknown`. Not the raw destination. |
| `protocol` | `tcp`, `udp`, or `other`. |
| `decision` | `denied`. |
| `reason` | `not_in_allowlist`, `cross_tenant`, `dns_restricted`, or `firewall_drop`. |
| `emitted_at_ms` | Event time. |
| `event_digest` | Local `sha256:` digest over the redacted event. |

Raw destination IPs, raw domain names, raw port numbers, raw packet contents,
and raw firewall rule names must not appear in egress-denied events.

## Acceptable-Use Policy

### Permitted Workloads

Managed remote execution is permitted only for the following workload classes:

- agentic software development tasks (code generation, testing, review,
  debugging, refactoring);
- benchmark evaluation and capability assessment runs with declared inputs,
  policies, and required outputs;
- data processing and analysis tasks that operate on tenant-supplied or
  tenant-authorized data;
- model inference and orchestration tasks using declared model provider
  endpoints;
- artifact production tasks with declared artifact policies and closeout
  requirements.

### Prohibited Activities

The following activities are prohibited and constitute grounds for immediate
session termination and tenant suspension:

- cryptocurrency mining or proof-of-work computation;
- network scanning, port scanning, or reconnaissance of infrastructure not
  owned or authorized by the tenant;
- exploitation of vulnerabilities in systems not owned or authorized by the
  tenant;
- distributed denial-of-service (DDoS) or volumetric attack activity;
- exfiltration of data from other tenants, Cloud infrastructure, or
  OpenAgents systems;
- attempts to escape the session VM boundary or access host-level resources
  not granted by policy;
- generation or distribution of content that violates applicable law or
  OpenAgents terms of service;
- use of the session VM as a proxy or relay for prohibited traffic;
- any action designed to circumvent spend caps, session caps, egress policy,
  or abuse detection.

### Acceptable-Use Attestation

Before a tenant's first external session is provisioned, the tenant must
provide a signed acceptable-use attestation. The attestation is recorded as a
receipt and referenced by `aup_attestation_ref` in all subsequent session
records for that tenant.

| Field | Purpose |
| --- | --- |
| `aup_attestation_ref` | Ref to the signed acceptable-use attestation. |
| `aup_policy_version` | Version string of the AUP policy attested to. |
| `attested_at_ms` | Attestation timestamp. |
| `tenant_ref` | Stable tenant ref. |

Sessions for a tenant with no valid AUP attestation must be denied.

## Abuse Detection and Kill Switch

### Abuse Detection Signals

The Cloud abuse detection subsystem monitors the following signals per tenant:

| Signal | Description |
| --- | --- |
| Egress rate anomaly | Sustained high-volume outbound traffic on denied or allowlisted endpoints beyond declared expected volume |
| Compute burst pattern | Repeated rapid session acquire/release cycles inconsistent with declared workload |
| Spend velocity anomaly | Spend rate that exceeds the tenant tier norm by a configurable multiple |
| Session cap evasion attempt | Repeated requests that arrive below the concurrency cap threshold in close temporal proximity |
| Workroom capability abuse | Repeated gateway access denials, repeated forbidden-secret marker hits, or repeated AUP violations within a session |
| Cross-tenant probe pattern | Egress attempts toward other tenant VM addresses or Cloud-internal management addresses |
| Credential extraction attempt | Workroom process behavior consistent with attempting to read host-level credentials, metadata-server endpoints, or cross-session material |

Detection signals produce abuse-signal events appended to the tenant's
redacted audit trail. Signals do not carry raw packet contents, raw IP
addresses, raw domain names, or raw WorkOS user identifiers.

### Abuse Signal Event

| Field | Purpose |
| --- | --- |
| `event_kind` | `abuse_signal` |
| `tenant_ref` | Stable tenant ref. |
| `session_ref` | Non-secret session ref for the triggering session, if applicable. |
| `signal_class` | Bounded enum from the signal table above. |
| `severity` | `low`, `medium`, `high`, or `critical`. |
| `decision` | `monitor`, `throttle`, `terminate_session`, or `suspend_tenant`. |
| `reason` | Redacted bounded reason string. |
| `evidence_digest` | Digest over non-secret signal evidence. |
| `emitted_at_ms` | Event time. |
| `event_digest` | Local `sha256:` digest over the redacted event. |

### Automated Response Thresholds

| Severity | Default Automated Response |
| --- | --- |
| `low` | Monitor; append abuse-signal event; no session impact |
| `medium` | Throttle session; cap heartbeat refresh rate; alert ops |
| `high` | Terminate the triggering session; deny new sessions until human review |
| `critical` | Invoke kill switch; suspend tenant immediately; alert ops with high-priority page |

### Kill Switch

The kill switch provides a synchronous, authoritative mechanism to terminate
all active and pending sessions for a tenant and block new session requests.

Kill switch activation:

1. The Cloud control plane marks the tenant ref as `suspended` in the
   tenant-state store.
2. All active sessions for the tenant receive a `policy` release signal; their
   VM leases transition to `release` via the `openagents.gce_capacity_class.v1`
   release path.
3. All pending session requests for the tenant are denied with `tenant_suspended`
   as the reason.
4. A kill-switch receipt is issued for each terminated session and for the
   tenant-level suspension event.
5. The reconciler verifies cleanup receipts for all terminated sessions within
   the reconciler GC window and escalates any missing cleanup receipts to ops.

Kill switch invocation is available to:

- Cloud automated abuse detection at `critical` severity;
- Cloud operations staff via an authenticated internal control-plane action;
- WorkOS-triggered account suspension events received through a verified
  webhook.

Kill switch invocation is not available to workroom processes, Forge, Probe,
Autopilot, or Pylon clients.

### Kill Switch Receipt

| Field | Purpose |
| --- | --- |
| `receipt_kind` | `kill_switch_activated` |
| `tenant_ref` | Stable tenant ref. |
| `activated_by` | `abuse_detection`, `ops_action`, or `workos_webhook`. |
| `sessions_terminated` | Count of sessions terminated. Not a list of session refs. |
| `reason` | Redacted bounded reason string. |
| `evidence_digest` | Digest over non-secret kill-switch evidence. |
| `emitted_at_ms` | Receipt emission time. |
| `receipt_digest` | Local `sha256:` digest over the redacted receipt. |

Kill-switch receipts must not contain raw lists of session refs, raw WorkOS
org ids, raw user ids, raw IP addresses, raw abuse signal payloads, or raw
packet evidence.

## Audit and Redacted Evidence Requirements

### Audit Trail Composition

Every external tenant session produces an ordered audit trail composed of the
following receipt and event kinds, in lifecycle order:

```text
identity_verified
aup_check
cap_check (spend)
session_cap_check (concurrency / lease_slots)
egress_policy_applied
session_attached
[egress_denied* (zero or more)]
[abuse_signal* (zero or more)]
[gateway_access* (one or more, from oa-workroomd gateway-access.jsonl)]
[capability_attached* (zero or more)]
session_released
cleanup_receipt (from openagents.gce_capacity_class.v1)
[wipe_receipt* (from openagents.byo_credential_broker.v1, if BYO credentials used)]
```

The audit trail is the authoritative evidence chain for enforcement, dispute
resolution, and compliance review. Gaps in the trail (missing receipts for
required events) must be treated as enforcement failures and escalated.

### Redaction Requirements

Every audit event, receipt, projection, and log line produced for an external
tenant session must pass the following redaction filter before persistence:

- Raw WorkOS org ids, user ids, email addresses, cursor tokens, and any other
  WorkOS-internal identifiers are forbidden.
- Raw IP addresses (source or destination, IPv4 or IPv6) are forbidden.
- Raw VM names, GCE instance self-links, and GCE resource identifiers are
  forbidden (see `openagents.gce_capacity_class.v1`).
- Raw model API keys, OAuth tokens, authorization headers, bearer tokens, and
  any other tenant-origin credential material are forbidden (see
  `openagents.byo_credential_broker.v1`).
- Raw wallet seeds, private keys, preimages, and settlement credentials are
  forbidden.
- Raw packet payloads, raw HTTP request/response bodies containing credential
  material, and raw stderr/stdout containing secrets are forbidden.
- Raw GCP project ids, project numbers, service-account emails, and ADC tokens
  are forbidden.

The forbidden-secret marker filter from `INVARIANTS.md` Codex and gateway
paths applies to all durable persistence for external tenant sessions.

### Evidence Without Leaking Secrets

Audit evidence must be sufficient for a Cloud operator or compliance reviewer
to determine:

- which tenant (by `tenant_ref`) requested a session;
- which caps were checked and what the outcome was;
- which session VM was provisioned (by `lease_ref` and `instance_ref`);
- what capabilities were attached and revoked;
- whether any egress denials or abuse signals occurred;
- whether the session was released normally or via a kill switch;
- whether cleanup receipts were issued for all provisioned resources.

This determination must be possible using only refs, digests, bounded enum
values, timestamps, and redacted reason strings. It must not require access to
raw tenant identity material, raw IP addresses, or raw credential material.

### Evidence Retention

Audit evidence is retained in the receipt sink identified by
`receipt_sink_ref` for no less than the period required by platform compliance
policy. Evidence digests are sufficient for cross-referencing; raw secret
material must not be retained beyond the session VM lifetime.

## Refs-Only Retained Projections

### Tenant Policy Projection

Cloud may retain a compact tenant policy projection for cap evaluation and
status:

| Field | Permitted |
| --- | --- |
| `tenant_ref` | Stable opaque ref. Not a raw WorkOS org id. |
| `org_tier` | Bounded enum. |
| `plan_policy_ref` | Ref to active plan policy. |
| `aup_attestation_ref` | Ref to signed AUP attestation. |
| `aup_policy_version` | Bounded version string. |
| `status` | `active`, `suspended`, `pending_review`, or `churned`. |
| `active_session_count` | Integer count of active sessions. |
| `daily_spend_usd_micros` | Current UTC-day spend total. |
| `monthly_spend_usd_micros` | Current UTC-month spend total. |
| `latest_cap_check_ref` | Digest of the most recent cap check receipt. |
| `kill_switch_receipt_ref` | Digest of the kill-switch receipt if activated, otherwise null. |

Forbidden projection fields: raw WorkOS org ids, raw user ids, email addresses,
raw IP addresses, raw session ref lists, raw credential material, raw billing
line items, raw GCP resource identifiers, and raw abuse signal payloads.

### Session Projection

Cloud may retain a compact session projection for status and reconciliation:

| Field | Permitted |
| --- | --- |
| `session_ref` | Stable non-secret session ref. |
| `tenant_ref` | Stable opaque tenant ref. |
| `lease_ref` | Capacity lease ref from `openagents.gce_capacity_class.v1`. |
| `workroom_ref` | Redacted workroom ref. |
| `egress_policy_ref` | Ref to the session egress policy. |
| `isolation_profile_ref` | Ref to the isolation profile. |
| `aup_attestation_ref` | Ref to the AUP attestation for this tenant. |
| `state` | `pending`, `provisioning`, `active`, `releasing`, or `released`. |
| `release_reason` | `normal`, `ttl_expired`, `idle_timeout`, `kill_switch`, `policy`, or `abuse`. |
| `attach_receipt_ref` | Digest of the session attach receipt. |
| `cleanup_receipt_ref` | Digest of the VM cleanup receipt, nullable until released. |
| `session_started_at_ms` / `session_ended_at_ms` | Timestamps. |

Forbidden projection fields: raw WorkOS identifiers, raw IP addresses, raw VM
names, raw GCP resource identifiers, raw credential material, raw egress
destination lists, and raw abuse signal payloads.

### Public-Safe Shape

Projections forwarded to Vortex, Forge, Nexus, Probe, or Pylon-facing status
endpoints must pass the same forbidden-secret and private-topology filter used
by other Cloud contracts before forwarding. Refs, digests, bounded enum values,
and timestamps are the only field classes permitted in forwarded projections.

## Sequencing and Gate Requirements

The following checks must complete successfully, in order, before a session VM
is provisioned for an external tenant. Each step must produce a receipt. Any
failure at any step must deny the session without provisioning a VM.

```text
1. identity_verified      (WorkOS token verified; tenant_ref resolved)
2. aup_check              (valid AUP attestation exists for tenant)
3. cap_check (spend)      (daily and monthly budgets not exceeded)
4. session_cap_check      (concurrency and lease-slot caps not exceeded)
5. egress_policy_resolved (egress allowlist compiled and approved)
6. VM acquire             (openagents.gce_capacity_class.v1 acquire → ready)
7. session_attached       (session ref bound to lease ref)
```

Steps 1–5 are control-plane checks with no cloud resource allocation. Step 6
is the first point at which cloud resources are provisioned. Steps 1–5 must
all succeed before step 6 is attempted.

## Validation Rules

- `tenant_ref` must be a stable, opaque, non-guessable string not derived from
  or equal to the raw WorkOS org id, user id, or email address.
- `identity_session_ref`, `credential_session_ref`, `session_ref`, and other
  non-secret refs must be random, bounded, and not derived from secret material.
- `org_tier` must be a bounded enum. Unrecognized tier values must be treated
  as `free` for cap purposes.
- All spend cap fields must be non-negative integers in microusd. Negative or
  non-integer values must be rejected.
- Concurrency and lease-slot caps must be positive integers bounded by Cloud
  fleet policy.
- `aup_attestation_ref` must reference a valid, signed attestation for the
  current AUP policy version. Expired or unsigned attestations must be treated
  as absent.
- `egress_policy_ref` must reference a non-empty, explicit allowlist. Wildcard
  or empty allowlists are invalid.
- Sessions must not enter `active` state without a successful `session_attached`
  receipt.
- Sessions must not be attached to a VM lease that is already in `in_use` state.
- Kill-switch activation must atomically mark the tenant as `suspended` and
  initiate release for all active sessions. Partial suspension is invalid.
- Every receipt, event, projection, and log line must pass the forbidden-secret
  and private-topology redaction filter before persistence.
- Retained projections must reject raw WorkOS identifiers, raw IP addresses,
  raw GCP resource identifiers, raw credential material, raw wallet material,
  and raw abuse signal payloads.
- Cleanup receipts from `openagents.gce_capacity_class.v1` are required before
  a session may be marked `released` in the tenant projection.
- For sessions using BYO credentials, a wipe receipt from
  `openagents.byo_credential_broker.v1` is additionally required before
  successful credential closeout.
- The reconciler must verify cleanup receipts for all sessions and escalate any
  missing receipts within the reconciler GC window.
- No VM may be provisioned for a tenant whose status is `suspended`.

## Non-Goals

- No shared-kernel or container-only isolation for external tenant sessions in
  v1. Full VM isolation is required.
- No microVM (Firecracker) isolation for external sessions until a ratified
  microVM capacity class contract and corresponding tests exist.
- No wallet authority for any external tenant workroom.
- No broad host or cloud credentials in any external tenant session VM.
- No raw WorkOS org ids, user ids, or email addresses in any retained
  projection, receipt, or fleet state.
- No raw IP addresses in any retained projection, receipt, event, or log line.
- No reusable or persistent session VMs across tenant sessions.
- No unrestricted outbound internet egress from session VMs.
- No tenant-visible egress destination details in egress-denied events.
- No kill-switch authority for workroom processes, Forge, Probe, Autopilot, or
  Pylon clients.
- No generic secret tunnel for arbitrary tenant files or credentials beyond
  the `openagents.byo_credential_broker.v1` path.
- No billing invoice or public cost breakdown in any receipt or projection.
