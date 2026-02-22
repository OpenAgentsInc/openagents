# OpenAgents: Agent Contract (READ THIS FIRST)

This file is a map. Start here, then follow the linked canonical docs.

## Core Orientation

- Progressive disclosure map: `docs/AGENT_MAP.md`
- Documentation index: `docs/README.md`
- Repository map / ownership: `docs/PROJECT_OVERVIEW.md`
- Priorities and sequencing: `docs/ROADMAP.md`
- Vocabulary authority: `docs/GLOSSARY.md`
- Canonical architecture: `docs/ARCHITECTURE-RUST.md`
- Canonical build roadmap: `docs/ARCHITECTURE-RUST-ROADMAP.md`
- Canonical deploy/process matrix: `docs/DEPLOYMENT_RUST_SERVICES.md`
- ADR index: `docs/adr/INDEX.md`

## README Doc Index (Moved)

Canonical architecture and migration sequencing:

- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- `docs/DEPLOYMENT_RUST_SERVICES.md`

Documentation entry points:

- `AGENTS.md`
- `docs/README.md`
- `docs/PROJECT_OVERVIEW.md`
- `docs/ROADMAP.md`

## Contracts (Canonical Specs)

- Execution artifacts + replay: `docs/execution/README.md`
- Protocol contracts: `docs/protocol/README.md`
- DSE/compiler contracts: `docs/dse/README.md`
- Proto authority: `proto/README.md`

## Plans and Strategy

- Plans index: `docs/plans/README.md`
- Active plans: `docs/plans/active/`
- Synthesis strategy: `docs/SYNTHESIS.md`
- Synthesis execution posture: `docs/SYNTHESIS_EXECUTION.md`

## Product and Service Surfaces

- Web control service: `apps/openagents.com/service/`
- Web WASM shell: `apps/openagents.com/web-shell/`
- Runtime service: `apps/runtime/`
- Desktop app: `apps/autopilot-desktop/`
- iOS app: `apps/autopilot-ios/`
- Onyx app: `apps/onyx/`
- Shared Rust crates: `crates/`

## Operations and Verification

- Local CI policy: `docs/LOCAL_CI.md`
- Staging/prod validation matrix: `docs/RUST_STAGING_PROD_VALIDATION.md`
- Legacy infra decommission sequencing: `docs/RUST_LEGACY_INFRA_DECOMMISSION.md`
- Runtime deploy + migrate runbook: `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- Runtime WS incident runbook: `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`

Canonical runtime deploy + migrate command:

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/runtime/runtime:<TAG> \
apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh
```

## Authority (Non-Negotiable)

1. If docs conflict with behavior: code wins.
2. If terminology conflicts: `docs/GLOSSARY.md` wins.
3. If architecture intent conflicts: ADRs win.
4. If status conflicts: active codebase + `docs/PROJECT_OVERVIEW.md` win.
5. If sequencing conflicts: `docs/ROADMAP.md` wins.

## Mandatory Pre-Coding Gate (Non-Negotiable)

You must complete this gate before writing or modifying code.

1. Identify touched surfaces and read the governing ADR(s), invariant gates, and ownership-boundary docs.
2. Record in your first implementation update which docs were checked and which invariants/ADRs constrain the change.
3. If proposed code violates those constraints, stop and redesign before editing files.
4. If constraints still conflict with requested work, stop and ask the user how to proceed.

iOS Codex/WGPUI-specific required reads before coding:
- `docs/plans/active/rust-migration-invariant-gates.md` (minimum: `INV-03`, `INV-07`, `INV-11`)
- `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
- `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`
- `apps/autopilot-ios/docs/wgpui-codex-ownership-boundaries.md`
- `apps/autopilot-ios/docs/codex-wgpui-parity-gates.md`
- `apps/autopilot-ios/docs/rust-client-core-integration.md`

Hard rule for iOS Codex lanes:
- Product UI/state/business logic is Rust/WGPUI-authoritative.
- Swift/SwiftUI is host/bootstrap/OS bridge code only.
- Do not add new Swift-owned Codex product logic.

## Engineering Invariants

- Verify before claiming completion.
- No TODO-only stubs in production paths.
- Boundary contracts are typed and schema-governed.
- Replayability and deterministic hashes are required for execution artifacts.

Canonical references:
- `docs/execution/ARTIFACTS.md`
- `docs/execution/REPLAY.md`
- `docs/protocol/PROTOCOL_SURFACE.md`
- `docs/plans/active/rust-migration-invariant-gates.md`

## Git Hygiene

- Do not create `git worktree` without explicit user approval.
- Do not use `git stash` without explicit user approval.
- If clean-state work is needed, present options: commit, discard, or approved temporary worktree.

## Output Expectations

Autonomous run output is the Verified Patch Bundle:
- `PR_SUMMARY.md`
- `RECEIPT.json`
- `REPLAY.jsonl`

See:
- `docs/execution/ARTIFACTS.md`
- `docs/execution/REPLAY.md`
