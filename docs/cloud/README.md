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
  contract and Box-v1 corpus are admitted by SBX-00, the durable store is
  implemented by SBX-01, and the default-off GCE runtime component is
  implemented by SBX-02. The default-off `/v1` compatibility facade and exact
  SDK local/loopback HTTP conformance are implemented by SBX-03. The
  [SBX-02 runtime runbook](./bootstrap/SBX-02-managed-sandbox-runtime.md)
  defines its exact profile and live component harness. The
  [SBX-03 facade runbook](../../apps/openagents.com/docs/2026-07-19-managed-sandbox-box-v1-facade.md)
  defines authentication, configuration, tests, and current typed-unavailable
- [SBX-04 turn runtime](./bootstrap/SBX-04-managed-sandbox-turn-runtime.md) —
  implements the native Codex/Claude turn authority,
  ordered reconnect, exact-turn interrupt, and the private default-off guest
  driver adapter. The runbook records the component proof and live boundary.
  I/O safeguards, consumers, independent live rollout, and production
  availability remain gated by SBX-05 through SBX-10
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

Fake GCE and fake Cloud-VM provisioners are the default for their legacy test
lanes. They cannot report managed-sandbox readiness. The managed-sandbox GCE
provider is default-off and requires the exact `live_gce` profile plus the
control VM metadata identity.

## Local smokes

```bash
# Fake control plane on loopback (placement, events, cancel, fake GCE, fake Cloud-VM)
scripts/cloud/fake-control-plane-loopback-smoke.sh

# Stage oa-workroomd for Agent Computer guest image bake
apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh
```
