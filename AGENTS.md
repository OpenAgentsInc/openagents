# OpenAgents: Agent Contract (READ THIS FIRST)

This file is a map. Start here, then follow the linked canonical docs.

## Core Orientation

- Progressive disclosure map: `docs/core/AGENT_MAP.md`
- Documentation index: `docs/core/README.md`
- Repository map / ownership: `docs/core/PROJECT_OVERVIEW.md`
- Priorities and sequencing: `docs/core/ROADMAP.md`
- Vocabulary authority: `docs/core/GLOSSARY.md`
- Canonical architecture: `docs/core/ARCHITECTURE.md`
- Canonical migration sequencing: `docs/core/ARCHITECTURE.md` (`Implementation Sequencing`)
- Canonical deploy/process matrix: `docs/core/DEPLOYMENT_RUST_SERVICES.md`
- ADR index: `docs/adr/INDEX.md`

## README Doc Index (Moved)

Canonical architecture and migration sequencing:

- `docs/core/ARCHITECTURE.md`
- `docs/core/DEPLOYMENT_RUST_SERVICES.md`

Documentation entry points:

- `AGENTS.md`
- `docs/core/README.md`
- `docs/core/PROJECT_OVERVIEW.md`
- `docs/core/ROADMAP.md`

## Contracts (Canonical Specs)

- Execution artifacts + replay: `docs/execution/README.md`
- Protocol contracts: `docs/protocol/README.md`
- DSE/compiler contracts: `docs/dse/README.md`
- Proto authority: `proto/README.md`

## Plans and Strategy

- Plans index: `docs/plans/README.md`
- Active plans: `docs/plans/`
- Synthesis strategy: `docs/core/SYNTHESIS.md`
- Synthesis execution posture: `docs/core/SYNTHESIS_EXECUTION.md`

## Product and Service Surfaces

- Web control service: `apps/openagents.com/service/`
- Web WASM shell: `apps/openagents.com/web-shell/`
- Runtime service: `apps/runtime/`
- Desktop app: `apps/autopilot-desktop/`
- iOS app: `apps/autopilot-ios/`
- Onyx app: `apps/onyx/`
- Shared Rust crates: `crates/`

## Operations and Verification

- Local CI policy: `docs/core/LOCAL_CI.md`
- Staging/prod validation matrix: `docs/core/RUST_STAGING_PROD_VALIDATION.md`
- Legacy infra decommission sequencing: `docs/core/RUST_LEGACY_INFRA_DECOMMISSION.md`
- Deploy/process matrix (canonical commands): `docs/core/DEPLOYMENT_RUST_SERVICES.md`
- Control service staging deploy runbook: `apps/openagents.com/service/docs/STAGING_DEPLOY_RUNBOOK.md`
- Runtime deploy + migrate runbook: `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- Runtime WS incident runbook: `apps/runtime/docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`

Canonical Rust runtime deploy + migrate command:

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-runtime/runtime-rust:<TAG> \
apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh
```

Note:
- Cloud Run service `openagents-runtime` (and `openagents-runtime-migrate`) is a legacy runtime lane. Do not deploy Rust runtime images there.
- Artifact Registry `openagents-runtime/runtime:latest` is legacy; Rust runtime images publish to `openagents-runtime/runtime-rust:*`.

## Authority (Non-Negotiable)

1. If docs conflict with behavior: code wins.
2. If terminology conflicts: `docs/core/GLOSSARY.md` wins.
3. If architecture intent conflicts: ADRs win.
4. If status conflicts: active codebase + `docs/core/PROJECT_OVERVIEW.md` win.
5. If sequencing conflicts: `docs/core/ROADMAP.md` wins.

## Mandatory Pre-Coding Gate (Non-Negotiable)

You must complete this gate before writing or modifying code.

0. Read these baseline authorities first for every coding task:
   - `docs/adr/INDEX.md`
   - `docs/plans/active/rust-migration-invariant-gates.md`
1. Identify touched surfaces and read the governing ADR(s), invariant gates, and ownership-boundary docs.
2. Record a preflight proof in your first implementation update:
   - List exactly which ADR(s)/invariant docs were checked.
   - State the concrete constraints they impose on the planned change.
3. Do not edit files until that preflight proof is written.
4. If proposed code violates those constraints, stop and redesign before editing files.
5. If constraints still conflict with requested work, stop and ask the user how to proceed.

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
- `.github/` workflow automation is forbidden in this repo; do not add GitHub Actions workflow files.

Canonical references:
- `docs/execution/ARTIFACTS.md`
- `docs/execution/REPLAY.md`
- `docs/protocol/PROTOCOL_SURFACE.md`
- `docs/plans/active/rust-migration-invariant-gates.md`

## Git Hygiene

- Do not create `git worktree` without explicit user approval.
- Do not use `git stash` without explicit user approval.
- If clean-state work is needed, present options: commit, discard, or approved temporary worktree.
- In multi-agent repos, unrelated modified/staged files are expected.
- Do not pause work to ask for direction solely because unrelated files changed.
- Ignore unrelated files and commit only the files you edited for the requested task.
- Never revert or rewrite unrelated files unless the user explicitly requests it.

## Output Expectations

Autonomous run output is the Verified Patch Bundle:
- `PR_SUMMARY.md`
- `RECEIPT.json`
- `REPLAY.jsonl`

See:
- `docs/execution/ARTIFACTS.md`
- `docs/execution/REPLAY.md`
