# Cloud open-source consolidation — migration receipt

Date: 2026-07-09
Tracking issue: [#8591](https://github.com/OpenAgentsInc/openagents/issues/8591)
Plan: [2026-07-08-cloud-repo-open-source-consolidation-plan.md](./2026-07-08-cloud-repo-open-source-consolidation-plan.md)

## Source freeze

| Field | Value |
| --- | --- |
| Private source repo | `OpenAgentsInc/cloud` |
| Source commit | `f87a60c3a7600ae377ec392052f8d85dcc9af421` |
| Destination | public `OpenAgentsInc/openagents` monorepo |
| License for moved crates | MIT (openagents workspace) |

## What moved

| Private path | Public path |
| --- | --- |
| `crates/*` | `crates/*` (same crate names) |
| `fixtures/*` | `fixtures/cloud/*` |
| `docs/contracts/*` | `docs/cloud/contracts/*` |
| `docs/oa-node/*` | `docs/cloud/oa-node/*` |
| `docs/oa-workroomd/*` | `docs/cloud/oa-workroomd/*` |
| `docs/control/*` | `docs/cloud/control/*` |
| `docs/bootstrap/*` | `docs/cloud/bootstrap/*` |
| `docs/benchmarks/*` | `docs/cloud/benchmarks/*` |
| `INVARIANTS.md` | `docs/cloud/INVARIANTS.md` (adapted) |
| `docker/*` | `docker/cloud/*` |
| `scripts/*` | `scripts/cloud/*` |
| `config/*` | `config/cloud/*` |
| `runners/*` | `runners/*` |

## Explicit non-moves

Secrets, raw topology, service-account keys, live tokens, private customer data,
raw prompts, private repo contents, wallet material, and host-local absolute
paths do not move. Deploy scripts take project ids and tokens from CLI flags /
runtime env only.

## Phase status

| Phase | Status |
| --- | --- |
| 0 freeze + scrub + tracking issue | done (#8591, this receipt) |
| 1 contracts + fixtures | done (Rust crate + fixtures/cloud) |
| 2 control plane fake mode | done (crate + tests + `scripts/cloud/fake-control-plane-loopback-smoke.sh`) |
| 3 Agent Computer VM path | code moved; live lane remains env-gated |
| 4 workroomd | done (crate + tests + Agent Computer image staging script) |
| 5 oa-node | done (crate + tests + authority docs rewrite) |
| 6 production cutover | **control plane cut over** to openagents-built image (see `docs/cloud/receipts/2026-07-09-phase6-openagents-cutover.md`); full mobile Firecracker DoD not re-run |

## Build / test

```bash
cargo test --workspace
cargo test -p openagents-cloud-contract
cargo test -p oa-codex-control
cargo test -p oa-node
cargo test -p oa-workroomd
```

## Private repo posture after merge

`OpenAgentsInc/cloud` is read-only historical mirror. New Cloud implementation
work lands in this monorepo. See private cloud README pointer update.

## 2026-07-09 residuals (#8591 non-owner-gated)

- Authority docs rewritten / historical-bannered (ARCHITECTURE, ISSUES, BENCHMARK_CLOUD, bootstrap, NEXUS_REGISTRY, SETTLEMENT_MODES).
- Agent Computer image bake references in-repo `oa-workroomd` via `apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh`.
- Named loopback smoke: `scripts/cloud/fake-control-plane-loopback-smoke.sh` (placement → events → cancel → fake GCE → fake Cloud-VM).
- Phase 6 production cutover remains owner-gated.

## Phase 6 cutover receipt

See `docs/cloud/receipts/2026-07-09-phase6-openagents-cutover.md`.
