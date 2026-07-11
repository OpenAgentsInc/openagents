# P0 TASK: Codex inside a real Agent Computer/workroom

- Issue: #8547
- Parent capability: #8636 hybrid target routing
- Mobile consumer: #8597
- Status: active P0 minimum remote-workroom path; advanced capacity follows R7
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md), Revision 29

## Outcome

An authenticated owner can start a repository-bound Codex unit from OpenAgents
mobile inside a real isolated OpenAgents Agent Computer/workroom. The unit uses
the normal Fleet/Sync progress and closeout contract and can continue on
Desktop without forking thread, workroom, run, account, or receipt truth.

## Current truthful baseline

The rootfs, in-VM Codex execution, exact token receipt path, and owner-
subscription metering exemption are source/fixture proven; the Worker half of
the double-billing correction is deployed. A real brokered owner-account
Firecracker turn with complete token/compute/reclaim/writeback evidence is not
yet accepted and remains the only target that can unlock #8636's live hybrid
routing exit.

## Scope

- Reproducibly build and pin the Agent Computer rootfs/runtime dependencies.
- Create/resume/stop/reclaim an owner-scoped workroom with explicit lifecycle,
  TTL, snapshot identity, and isolation rung.
- Populate bounded provider and Git `auth_grant_ref` values at placement;
  redeem them into isolated scratch account homes under broker-only policy.
- Bind stable owner/repository/thread/workroom/run refs and execute through the
  approved Codex runtime with pinned repository/work context.
- Expose typed bounded file IO, run/spawn/PTY, managed preview ports, network
  policy, artifacts, verification, and safe branch/PR writeback.
- Record exact model usage or `not_measured`; keep owner subscription/model
  usage distinct from compute lifecycle/economics.
- Authorize any owner-subscription metering exemption from server-held
  grant/capacity authority, not a poster-supplied lane/provider label.
- Treat unexpected metering of a proven owner-subscription turn as a typed
  failure/alert rather than silently charging the owner twice.
- Deduplicate usage truth and charging across timeout/retry with a stable
  server-derived usage identity; a fresh client UUID is not the idempotency
  authority.
- Destroy scratch, prevent grant replay, revoke preview access, and prove
  reclaim without leaving an executable orphan.
- Feed normal FleetRun/Khala Sync progress, approvals, command outcomes,
  verification, writeback, and receipt projections to mobile and Desktop.

## Non-goals before R7

- every provider or region;
- elastic capacity optimization or a sophisticated placement planner;
- public arbitrary container hosting;
- treating a control-plane mock or lower-isolation dev container as accepted
  production isolation.

## Exit

From a physical phone, one owner starts a real Codex workroom, observes
authoritative progress, inspects/changes code, reviews the exact diff, runs a
bounded command, opens a managed preview, completes verified branch/PR
writeback, and receives token/compute/reclaim receipts. Desktop resumes the same
refs. Brokered credentials do not reach either client, grant replay and force
writeback fail closed, and scratch/ports/processes are gone after reclaim.
The receipt proves the metering exemption from the redeemed grant, refuses an
unexpected owner-capacity charge, and retains one token-usage fact across a
lost-response retry.
