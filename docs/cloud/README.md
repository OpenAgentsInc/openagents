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
