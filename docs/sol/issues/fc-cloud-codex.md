# FC-CLOUD-1: Codex inside an Agent Computer

Parent capability: #8636 hybrid fleet routing.

## Outcome

A Sarah FleetRun can place a Codex work unit on the owner's connected Codex
account inside an OpenAgents Agent Computer.

## Scope

- Reproducibly build and pin the Agent Computer rootfs with Codex and runtime
  dependencies.
- Populate an owner-scoped `auth_grant_ref` at placement.
- Redeem inside the VM into an isolated scratch `CODEX_HOME` under
  `provider_credential_policy: broker_only`.
- Execute through `codex_app_server` with pinned repository/work context.
- Record exact model usage as owner subscription capacity with
  `tokenChargeMetered: false`; record compute lifecycle separately.
- Destroy scratch and make grant replay impossible on reclaim.
- Feed the normal FleetRun work-unit progress/closeout contract.

## Exit

One Sarah-created work unit completes inside real Firecracker on the nested-
virt host with verification, token truth, compute receipts, scratch wipe, and
reclaim evidence. Fake control-plane VM lifecycle is not sufficient proof.
