# 2026-02-25 Workspace Crate Usage Audit

Status: complete
Date: 2026-02-25
Owner: repo audit (Codex)

## Scope

Requested scope:

1. Audit all Rust workspace crates.
2. Determine how each crate is used now.
3. Identify likely unused/legacy crates for deletion or consolidation.

This audit is workspace-graph first (Cargo metadata + direct dependency edges), then command/docs invocation signals for standalone binaries and suspicious leaf libraries.

## Mandatory Preflight Authority Check

Reviewed before analysis and edits:

1. `docs/adr/INDEX.md`
2. `docs/plans/rust-migration-invariant-gates.md`

Constraints applied:

1. Keep Rust-only architecture baseline and active ADR authority alignment (`ADR-0001`..`ADR-0007`).
2. Treat code/dependency graph as source of truth when docs drift.
3. Do not propose cleanup that violates active invariants (`INV-01`, `INV-03`, `INV-04`..`INV-07`).

## Method

Commands used:

```bash
cargo metadata --format-version 1 > /tmp/openagents-cargo-metadata.json
```

Then:

1. Extracted all workspace members (`52`) and workspace-only dependency edges.
2. Built reverse-dependency map (`dependents`) per crate.
3. Classified crates by usage shape:
   - `entrypoint-bin`: has binary target (root app/tool package)
   - `shared-core-lib`: library with >=5 direct workspace dependents
   - `internal-lib`: library with 1-4 direct workspace dependents
   - `macro-support`: proc-macro support crate
   - `candidate-unused-lib`: library with 0 direct workspace dependents
4. Ran invocation scans for zero-dependent packages to separate true dead weight from normal root binaries.

## Snapshot Metrics

1. Workspace crates audited: `52`
2. Workspace internal dependency edges: `117`
3. Classification counts:
   - `entrypoint-bin`: `14`
   - `shared-core-lib`: `7`
   - `internal-lib`: `25`
   - `macro-support`: `1`
   - `candidate-unused-lib`: `5`

Most depended-on crates:

1. `nostr` (`14` direct dependents)
2. `nostr-client` (`10`)
3. `openagents-spark` (`10`)
4. `dsrs` (`9`)
5. `wgpui` (`6`)

## Findings

## Critical

1. Five workspace library crates have zero direct workspace dependents and zero external invocation signals (outside their own crate docs/tests):
   - `ai-server`
   - `codex-mcp`
   - `openagents-app-state`
   - `testing`
   - `voice`

Interpretation: these are high-probability dead/parked crates and should be triaged for removal or explicit retention ownership.

## High

2. One standalone bin crate appears dormant in operational runbooks/scripts:
   - `ws-test` (`crates/ws-test/Cargo.toml`)
   - direct dependents: `0`
   - external cargo invocation references: `0`

Interpretation: likely legacy local integration utility; should be either archived/removed or moved to an explicit non-default tools lane.

3. Several root binary packages have `0` dependents by design but remain actively referenced via docs/scripts and should not be treated as dead by graph-only heuristics:
   - `autopilot-desktop`, `openagents-control-service`, `openagents-runtime-service`, `lightning-ops`, `lightning-wallet-executor`, `autopilot`, `openagents-cli`, `openagents-registry`, `arrow`

## Medium

4. Many `internal-lib` crates are single-dependent (`10` crates), indicating consolidation opportunities (possible inlining or tighter module boundaries), e.g.:
   - `agent` -> `pylon`
   - `openagents-runtime-client` -> `openagents-control-service`
   - `openagents-citrea` -> `openagents-cli`
   - `openagents-relay` -> `pylon`

## Recommended Cleanup Sequence

## Phase 0 (immediate)

1. Open deletion/retention decision issues for the five `candidate-unused-lib` crates.
2. For each crate, require one of:
   - delete now,
   - assign owner + near-term integration issue,
   - move to archive/backroom with manifest update.

## Phase 1

3. Resolve `ws-test` status:
   - if still needed, add explicit runbook owner + usage lane;
   - if not needed, remove from workspace.
4. Audit single-dependent libs for inlining opportunities to shrink workspace member count and compile graph complexity.

## Phase 2

5. After deletions/consolidations, rerun this audit and compare:
   - workspace crate count
   - internal edge count
   - dead/zero-dependent library count

## Candidate-Unused Crates (Action List)

| Crate | Manifest | Targets | Direct Dependents | External Cargo Invocation Signals | Recommendation |
| --- | --- | --- | ---: | ---: | --- |
| `ai-server` | `crates/ai-server/Cargo.toml` | `lib` | 0 | 0 | Delete or wire into active surface with owner/date |
| `codex-mcp` | `crates/codex-mcp/Cargo.toml` | `lib` | 0 | 0 (outside self README) | Delete or assign active MCP integration owner |
| `openagents-app-state` | `crates/openagents-app-state/Cargo.toml` | `lib,test` | 0 | 0 | Delete or make canonical shared state crate with adopters |
| `testing` | `crates/testing/Cargo.toml` | `lib,test` | 0 | 0 | Delete or explicitly wire into local-ci/test lanes |
| `voice` | `crates/voice/Cargo.toml` | `lib` | 0 | 0 | Delete unless voice roadmap issue exists and owner assigned |

`ws-test` note:

- `crates/ws-test/Cargo.toml` has `bin` target with no dependents and no external invocation signals; treat as dormant tool candidate.

## Full Workspace Crate Usage Inventory

| Crate | Manifest | Targets | Direct Workspace Dependents | Dependents | Classification |
| --- | --- | --- | ---: | --- | --- |
| `adjutant` | `crates/adjutant/Cargo.toml` | `bin,lib,test` | 1 | autopilot | `entrypoint-bin` |
| `agent` | `crates/agent/Cargo.toml` | `lib` | 1 | pylon | `internal-lib` |
| `ai-server` | `crates/ai-server/Cargo.toml` | `lib` | 0 | - | `candidate-unused-lib` |
| `arrow` | `crates/arrow/Cargo.toml` | `bin` | 0 | - | `entrypoint-bin` |
| `autopilot` | `crates/autopilot/Cargo.toml` | `bin,lib` | 0 | - | `entrypoint-bin` |
| `autopilot-core` | `crates/autopilot-core/Cargo.toml` | `cdylib,example,rlib` | 3 | adjutant, autopilot, autopilot-desktop | `internal-lib` |
| `autopilot-desktop` | `apps/autopilot-desktop/Cargo.toml` | `bin` | 0 | - | `entrypoint-bin` |
| `autopilot-inbox-domain` | `crates/autopilot-inbox-domain/Cargo.toml` | `lib` | 2 | autopilot-desktop, openagents-control-service | `internal-lib` |
| `autopilot-spacetime` | `crates/autopilot-spacetime/Cargo.toml` | `lib` | 2 | autopilot-desktop, openagents-runtime-service | `internal-lib` |
| `autopilot_app` | `crates/autopilot_app/Cargo.toml` | `lib,test` | 3 | autopilot-desktop, autopilot_ui, openagents-control-service | `internal-lib` |
| `autopilot_ui` | `crates/autopilot_ui/Cargo.toml` | `lib,test` | 1 | autopilot-desktop | `internal-lib` |
| `codex-client` | `crates/codex-client/Cargo.toml` | `lib` | 5 | adjutant, autopilot, autopilot-desktop, dsrs, pylon | `shared-core-lib` |
| `codex-mcp` | `crates/codex-mcp/Cargo.toml` | `lib` | 0 | - | `candidate-unused-lib` |
| `compute` | `crates/compute/Cargo.toml` | `lib,test` | 2 | pylon, runtime | `internal-lib` |
| `dsrs` | `crates/dsrs/Cargo.toml` | `example,lib,test` | 9 | adjutant, arrow, autopilot, autopilot-core, autopilot-desktop, dsrs-macros, frlm, gateway, runtime | `shared-core-lib` |
| `dsrs-macros` | `crates/dsrs-macros/Cargo.toml` | `proc-macro` | 2 | adjutant, dsrs | `macro-support` |
| `editor` | `crates/editor/Cargo.toml` | `lib` | 1 | autopilot_ui | `internal-lib` |
| `frlm` | `crates/frlm/Cargo.toml` | `bench,lib` | 2 | compute, pylon | `internal-lib` |
| `gateway` | `crates/gateway/Cargo.toml` | `lib` | 3 | adjutant, autopilot, pylon | `internal-lib` |
| `gpt-oss` | `crates/gpt-oss/Cargo.toml` | `lib,test` | 5 | adjutant, arrow, autopilot, dsrs, local-inference | `shared-core-lib` |
| `issues` | `crates/issues/Cargo.toml` | `bench,example,lib,test` | 2 | adjutant, autopilot | `internal-lib` |
| `lightning-ops` | `apps/lightning-ops/Cargo.toml` | `bin` | 0 | - | `entrypoint-bin` |
| `lightning-wallet-executor` | `apps/lightning-wallet-executor/Cargo.toml` | `bin,lib,test` | 0 | - | `entrypoint-bin` |
| `lm-router` | `crates/lm-router/Cargo.toml` | `lib` | 3 | autopilot, dsrs, rlm | `internal-lib` |
| `local-inference` | `crates/local-inference/Cargo.toml` | `bench,lib,test` | 1 | gpt-oss | `internal-lib` |
| `neobank` | `crates/neobank/Cargo.toml` | `lib` | 1 | openagents-runtime-service | `internal-lib` |
| `nostr` | `crates/nostr/core/Cargo.toml` | `bench,example,lib,test` | 14 | agent, autopilot, autopilot-desktop, autopilot_ui, compute, dsrs, frlm, nostr-client, openagents-citrea, openagents-cli, openagents-registry, openagents-runtime-service, pylon, runtime | `shared-core-lib` |
| `nostr-client` | `crates/nostr/client/Cargo.toml` | `bench,example,lib,test` | 10 | agent, autopilot, autopilot-core, autopilot-desktop, compute, dsrs, openagents-registry, openagents-runtime-service, pylon, runtime | `shared-core-lib` |
| `openagents-app-state` | `crates/openagents-app-state/Cargo.toml` | `lib,test` | 0 | - | `candidate-unused-lib` |
| `openagents-citrea` | `crates/citrea/Cargo.toml` | `lib,test` | 1 | openagents-cli | `internal-lib` |
| `openagents-cli` | `crates/openagents-cli/Cargo.toml` | `bin,lib` | 0 | - | `entrypoint-bin` |
| `openagents-client-core` | `crates/openagents-client-core/Cargo.toml` | `cdylib,rlib,staticlib` | 2 | autopilot-desktop, openagents-control-service | `internal-lib` |
| `openagents-codex-control` | `crates/openagents-codex-control/Cargo.toml` | `lib` | 2 | autopilot-desktop, openagents-client-core | `internal-lib` |
| `openagents-control-service` | `apps/openagents.com/Cargo.toml` | `bin,lib` | 0 | - | `entrypoint-bin` |
| `openagents-l402` | `crates/openagents-l402/Cargo.toml` | `lib` | 3 | neobank, openagents-control-service, openagents-runtime-service | `internal-lib` |
| `openagents-proto` | `crates/openagents-proto/Cargo.toml` | `custom-build,lib,test` | 2 | neobank, openagents-runtime-service | `internal-lib` |
| `openagents-registry` | `crates/openagents-registry/Cargo.toml` | `bin` | 0 | - | `entrypoint-bin` |
| `openagents-relay` | `crates/relay/Cargo.toml` | `lib` | 1 | pylon | `internal-lib` |
| `openagents-runtime-client` | `crates/openagents-runtime-client/Cargo.toml` | `lib` | 1 | openagents-control-service | `internal-lib` |
| `openagents-runtime-service` | `apps/runtime/Cargo.toml` | `bin,lib` | 0 | - | `entrypoint-bin` |
| `openagents-spark` | `crates/spark/Cargo.toml` | `lib,test` | 10 | agent, autopilot, autopilot-desktop, autopilot_ui, compute, lightning-wallet-executor, nostr-client, openagents-cli, pylon, runtime | `shared-core-lib` |
| `openagents-ui-core` | `crates/openagents-ui-core/Cargo.toml` | `lib` | 1 | autopilot_ui | `internal-lib` |
| `openagents-utils` | `crates/openagents-utils/Cargo.toml` | `lib` | 2 | issues, nostr-client | `internal-lib` |
| `protocol` | `crates/protocol/Cargo.toml` | `lib` | 4 | adjutant, dsrs, neobank, openagents-runtime-service | `internal-lib` |
| `pylon` | `crates/pylon/Cargo.toml` | `bin,lib` | 2 | autopilot-core, autopilot-desktop | `entrypoint-bin` |
| `rlm` | `crates/rlm/Cargo.toml` | `bin,example,lib,test` | 2 | adjutant, frlm | `entrypoint-bin` |
| `runtime` | `crates/runtime/Cargo.toml` | `bin,lib` | 4 | agent, autopilot-core, autopilot-desktop, pylon | `entrypoint-bin` |
| `testing` | `crates/testing/Cargo.toml` | `lib,test` | 0 | - | `candidate-unused-lib` |
| `vim` | `crates/vim/Cargo.toml` | `lib` | 1 | wgpui | `internal-lib` |
| `voice` | `crates/voice/Cargo.toml` | `lib` | 0 | - | `candidate-unused-lib` |
| `wgpui` | `crates/wgpui/Cargo.toml` | `bench,cdylib,example,rlib` | 6 | autopilot, autopilot-desktop, autopilot_ui, editor, openagents-client-core, openagents-ui-core | `shared-core-lib` |
| `ws-test` | `crates/ws-test/Cargo.toml` | `bin` | 0 | - | `entrypoint-bin` |
