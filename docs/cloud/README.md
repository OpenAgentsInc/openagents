# OpenAgents Cloud (in-repo)

OpenAgents Cloud infrastructure lives **in this monorepo**. The private
`OpenAgentsInc/cloud` repo is historical source only.

## Crates

| Crate | Role |
| --- | --- |
| `crates/openagents-cloud-contract` | Rust contract validators + fixture conformance |
| `crates/oa-codex-control` | HTTP control plane (placement, GCE capacity, Cloud-VM) |
| `crates/oa-node` | Managed node daemon |
| `crates/oa-workroomd` | Workroom sidecar |
| `crates/oa-cloud-run-bridge` | Historical Cloud Run bridge — not new production paths |

## Related public surfaces

- Worker admission / metering: `apps/openagents.com/workers/api/src/cloud/`
- Agent Computer host bootstrap: `apps/pylon/deploy/agent-computer/`
- QA Cloud-VM seam: `apps/qa-runner` (`CloudVmProvisionerV2`)
- Placement trigger: `scripts/qa-async-gce-trigger.ts`

## Docs

- [Migration receipt](./MIGRATION.md)
- [Consolidation plan](./2026-07-08-cloud-repo-open-source-consolidation-plan.md)
- [Invariants](./INVARIANTS.md)
- [Architecture](./ARCHITECTURE.md)
- Contracts under `docs/cloud/contracts/`
- [Managed agent sandboxes accepted plan](../sol/2026-07-19-managed-agent-sandboxes-accepted-plan.md)
  and [ProductSpec](../../specs/openagents/managed-agent-sandboxes.product-spec.md)
  — active #9023 program for one owner-scoped GCP `SandboxResource`, a
  development-only Box SDK conformance facade, IDE/Sarah consumers, and live
  isolation/cleanup proof; the
  [`openagents.managed_sandbox.v1`](./contracts/openagents.managed_sandbox.v1.md)
  contract and Box-v1 corpus are admitted by SBX-00, while runtime and
  production availability remain gated by SBX-01 through SBX-10
- [Remote-first portable session pathway](../sol/2026-07-11-remote-first-portable-coding-sessions-pathway.md)
  — planned product/roadmap additions for owner-managed and managed-provider
  targets, cross-host checkpoints, general brokered capabilities, and mobile
  control; not current Cloud implementation status

## Local development

```bash
# From monorepo root
cargo test -p openagents-cloud-contract
cargo run -p oa-codex-control -- --help
cargo run -p oa-node -- --help
cargo run -p oa-workroomd -- --help
```

Fake GCE and fake Cloud-VM provisioners are the default. Live lanes are
explicit env-gated (`OA_CLOUD_VM_PROVISIONER=live`, GCE live flags).

## Local smokes

```bash
# Fake control plane on loopback (placement, events, cancel, fake GCE, fake Cloud-VM)
scripts/cloud/fake-control-plane-loopback-smoke.sh

# Stage oa-workroomd for Agent Computer guest image bake
apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh
```
