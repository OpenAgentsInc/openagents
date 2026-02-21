# OA-RUST-103 Package Disposition Audit (2026-02-21)

Status: Completed for `OA-RUST-103`
Owner lane: `owner:infra`

## Objective

Retire legacy TypeScript runtime package lanes under `packages/` so production-critical runtime paths no longer depend on Node/TypeScript package execution.

## Disposition Matrix

| Legacy lane | Previous role | Decision | Rust replacement / owner |
| --- | --- | --- | --- |
| `packages/effuse` | Effect runtime orchestration utilities | Archived/removed from active tree | Rust runtime + UI crates (`apps/runtime`, `crates/openagents-app-state`, `crates/openagents-ui-core`) |
| `packages/effuse-panes` | Pane/window primitives | Archived/removed from active tree | WGPUI pane surfaces (`crates/wgpui`, `crates/autopilot_ui`, `apps/autopilot-desktop`) |
| `packages/effuse-test` | TS E2E harness | Archived/removed from active tree | Rust-first checks via local CI + service smoke lanes |
| `packages/dse` | TS DSE/compiler implementation | Archived/removed from active tree | Rust DSE/runtime lanes (`crates/runtime`, `crates/openagents-proto`, runtime service contracts) |
| `packages/lightning-effect` | TS Lightning/L402 contracts + adapters | Archived/removed from active tree | `apps/lightning-ops` (Rust) + `apps/lightning-wallet-executor` (Rust) |
| `packages/lnd-effect` | TS LND integration contracts | Archived/removed from active tree | Rust lightning services + `crates/spark` |
| `packages/khala-sync` | TS Khala sync client | Removed as runtime dependency from active app manifests | Rust/Khala service contracts + app-local legacy fallback where needed |

## Runtime Dependency Graph (Before -> After)

Before:
- `apps/openagents.com` referenced `@openagentsinc/khala-sync` via `file:../../packages/khala-sync`.
- Verification scripts included `packages/*` TS lanes.

After:
- `apps/openagents.com/package.json` no longer depends on `@openagentsinc/khala-sync`.
- Active verification in `scripts/verify.sh` runs Rust service checks (`lightning-ops`, `lightning-wallet-executor`) and no longer enumerates TS package lanes.
- `packages/` is absent from active runtime surfaces.

## Enforcement Artifacts

- `apps/openagents.com/package.json`
- `apps/openagents.com/package-lock.json`
- `apps/openagents.com/resources/js/pages/admin/index.tsx`
- `scripts/verify.sh`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`

## Verification

- `cargo check -p lightning-ops`
- `cargo check -p lightning-wallet-executor`
- `cargo test --manifest-path apps/lightning-wallet-executor/Cargo.toml`
