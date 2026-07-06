# Khala Mobile Agent Computer Executor Runbook

Date: 2026-07-06
Issues: #8473, #8503, #8474-#8477, #8479
Status: #8473 executor spine landed; #8503 arms the Firecracker/GCE Agent
Computer path and is live-proof-gated; #8474 admission landed in the public
Worker; #8475 owns SCM credentials, #8476 owns isolation enforcement, #8477
owns writeback, and #8479 owns charging.

## Purpose

The mobile-only MVP runs coding turns on OpenAgents-owned Agent Computers, not
on a user's desktop Pylon and not on a hosted Pylon pool. An Agent Computer is
an isolated Firecracker microVM on our GCE capacity, assigned to one admitted
work context (`user + thread + repo binding`) and reclaimed after the lifecycle
policy says the context is idle or expired.

The Pylon runtime and coding agents remain implementation details inside the
image. The provisioned, metered, and user-facing unit is the Agent Computer.

The mobile wire contract remains the same:

- runtime events and turns continue to sync as `runtime_event` /
  `runtime_turn` entities;
- one-line tool summaries and assistant updates are still mobile surface data;
- exact token usage receipts are mirrored from runtime `usage.recorded` events;
- compute-time lifecycle receipts are emitted by the Agent Computer provisioner
  as `openagents.resource_usage_receipt.v1` refs.

## Deployment Shape

Public repo responsibilities:

- `apps/openagents.com/workers/api/src/cloud/cloud-coding-session-routes.ts`
  exposes the flag-gated `/v1/cloud-coding-sessions` launch/read seam;
- `apps/pylon/deploy/agent-computer/` documents and tests the public GCE
  nested-virt host shape;
- the Worker remains inert unless `CLOUD_CODING_SESSIONS_ENABLED=true`,
  `OA_CODEX_GCE_PROVISIONER=live`, and `OA_CLOUD_CONTROL_URL` plus the
  Worker-secret control token are configured.

Private `cloud/` responsibilities:

- `oa-codex-control` accepts placement assignments from the Worker;
- `oa-node` provisions Firecracker microVMs on the nested-virt host;
- lifecycle transitions emit refs-only public receipts;
- scratch disks are wiped and failed/quarantined microVMs are reclaimed.

Do not put control tokens, SCM credentials, provider keys, wallet material,
guest IPs, raw GCE instance names, prompts, repo content, or private traces in
public docs, issue comments, tests, logs, or Worker projections.

## Launch Flow

1. Mobile dispatch creates or resumes a thread with a repo binding.
2. Admission (#8474) checks the mobile bearer session, positive Pool B credit
   balance, per-user request/concurrency allowance, and Agent Computer capacity
   before any placement or Agent Computer assignment. Refusals are exactly
   `insufficient_credit`, `rate_limited`, or `org_capacity_unavailable`.
3. The Worker posts a refs-only `openagents.codex_placement_assignment.v1`
   payload to the private control plane.
4. The control plane provisions or reuses the work-context Agent Computer.
5. The runtime inside the microVM consumes the `khala_runtime_control_intent.v1`
   turn and emits normal Khala Sync runtime events.
6. Token usage is mirrored through
   `POST /api/khala/cloud/runtime-turn-usage`.
7. Compute lifecycle receipts are projected from `cloud.gce.*` control-plane
   events and later charged by #8479.
8. Idle or expired Agent Computers are reclaimed; scratch storage is wiped.

## Execution Lanes

`codex_app_server`
: Runs Codex inside the Agent Computer image with an OpenAgents-owned runtime
  credential. The microVM never receives raw user OAuth tokens or wallet
  material.

`claude_pylon`
: Runs Claude Agent SDK inside the same Agent Computer isolation envelope.
  Account credentials are org-owned runtime credentials, not another user's
  Pylon or account home.

`hosted_khala`
: Calls the OpenAgents gateway from inside the same admitted work context.
  Default model choice remains temporary until #8484's merged model-preference
  read contract is consumed by the executor path.

## Scaling

Scale by adding nested-virtualization GCE hosts and letting the private
provisioner place one Firecracker microVM per admitted work context. Do not
scale by sharing one persistent hosted-Pylon OS across users or repos.

Recommended first host:

- one `n2-standard-4` GCE VM in `openagentsgemini`;
- IAP/private egress only, no external IP by default;
- `/dev/kvm` verified before live arming;
- private `oa-node` provisioner configured from Secret Manager;
- no secrets in instance metadata or startup scripts.

Use `apps/pylon/deploy/agent-computer/setup-gce-host.sh --dry-run` to verify
the public command shape before live host creation.

## Draining And Reclaim

1. Remove the host from new-work admission at the control-plane layer.
2. Let active turns reach terminal `turn.finished` or `turn.interrupted`.
3. Reclaim each Agent Computer through the private provisioner, not by manually
   deleting guest files from the host OS.
4. Confirm lifecycle receipts include reclaim and scratch-wipe evidence refs.
5. Stop or delete the host only after the provisioner reports no active guest.

## Receipts

Token receipts:

```text
POST /api/khala/cloud/runtime-turn-usage
schemaVersion: openagents.khala_cloud_runtime_turn_usage.v1
```

Compute receipts:

```text
openagents.resource_usage_receipt.v1
event kinds: cloud.gce.provisioned, cloud.gce.resource_usage_receipt, cloud.gce.cleanup
```

The Worker route:

- requires an `oa_agent_` bearer for token usage mirroring;
- rejects linked user-Pylon agents posting for a different owner;
- requires nonzero exact input/output token counts;
- writes `openagents.token_usage_event.v1` with
  `demandKind=external`, `demandSource=khala_mobile_org_cloud_runtime`,
  `demandClient=khala-code-mobile`, and `usageTruth=exact`;
- projects Agent Computer lifecycle/resource receipt refs from the control
  plane without exposing private host topology.

#8479 consumes exact token receipts and exact compute lifecycle receipts for
credit charging. Do not charge from estimates or client-supplied amounts.

## Still Gated

- #8503: live nested-virt host, signed/digest-pinned image, Worker arming, and
  one real mobile-dispatched Firecracker turn receipt.
- #8474: public Worker admission gate landed; live turn admission still depends
  on #8503 arming real Agent Computer capacity.
- #8475: private GitHub checkout through the SCM auth broker only.
- #8476: isolation enforcement and retention policy from the Agent Computers
  strategy.
- #8477: branch/PR writeback with user GitHub authorization.
- #8479: credit metering and balance gate from exact token + compute receipts.
