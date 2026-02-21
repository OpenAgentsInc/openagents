# OpenAgents: Agent Contract (READ THIS FIRST)

This file is a **map**, not a manual. Start here, then follow links for deeper specs/runbooks.

## Map

### Core orientation

- Progressive disclosure map: `docs/AGENT_MAP.md`
- Documentation index: `docs/README.md`
- Repository map / ownership: `docs/PROJECT_OVERVIEW.md`
- Priorities and sequencing: `docs/ROADMAP.md`
- Vocabulary authority: `docs/GLOSSARY.md`
- Canonical architecture: `docs/ARCHITECTURE-RUST.md`
- Canonical migration sequencing: `docs/ARCHITECTURE-RUST-ROADMAP.md`
- Historical architecture snapshot (non-canonical): `docs/ARCHITECTURE.md`
- Architecture decisions (invariants/contracts): `docs/adr/INDEX.md`
- ADR authoring/review process: `docs/adr/README.md`
- Rust migration invariant gates: `docs/plans/active/rust-migration-invariant-gates.md`

### Contracts (canonical specs)

- Execution artifacts + replay: `docs/execution/README.md`
- Protocol contracts + reasons + comms: `docs/protocol/README.md`
- DSE/compiler contracts: `docs/dse/README.md`

### Planning and delivery

- Plans hub (active/completed/template): `docs/plans/README.md`
- Active plans: `docs/plans/active/`
- Completed plans: `docs/plans/completed/`

### Product/runtime surfaces

- Core web app: `apps/openagents.com/`
- Runtime: `apps/runtime/`
- iOS app: `apps/autopilot-ios/`
- Desktop app: `apps/autopilot-desktop/`
- Onyx app: `apps/onyx/`
- Shared Rust crates: `crates/`

### Operations, testing, and audits

- Production-safe E2E flow: `docs/autopilot/testing/PROD_E2E_TESTING.md`
- Trace retrieval and debugging: `docs/autopilot/testing/TRACE_RETRIEVAL.md`
- Architecture audits: `docs/audits/README.md`

### Local and research context

- Local notes/runbooks (operator-facing): `docs/local/`
- Research and references: `docs/research/`, `docs/rlm/`
- Transcript archives: `docs/transcripts/`

## Authority (Non-Negotiable)

1. If documentation conflicts with behavior: **CODE WINS**
2. If terminology conflicts across docs: **GLOSSARY WINS**
3. If architecture intent conflicts (invariants/interfaces/contracts): **ADRs WIN**
4. If implementation status conflicts across docs: prefer the active codebase + `docs/PROJECT_OVERVIEW.md`
5. If sequencing conflicts: **ROADMAP WINS**

## Engineering Invariants (Ship-Quality Rules)

- Verification first: do not claim success without running the relevant harness (lint/test/build/e2e as appropriate).
- No stubs: do not land TODO-only placeholders or mock implementations in production paths.
- Typed contracts: decision points become **Signatures**; tools have JSON schemas validated by the runtime.
- Everything is logged + replayable: deterministic hashes, receipts, and replay events are required.

Canonical specs:
- Execution artifacts: `docs/execution/ARTIFACTS.md`
- Replay format: `docs/execution/REPLAY.md`
- Protocol field semantics: `docs/protocol/PROTOCOL_SURFACE.md`
- Rust migration invariant gate checklist: `docs/plans/active/rust-migration-invariant-gates.md`

## Effect Best Practices (Non-Negotiable)

Always consult `effect-solutions` before writing Effect code:

1. `effect-solutions list`
2. `effect-solutions show <topic>...`
3. Reference implementations: `.reference/effect/` (run `effect-solutions setup` first)

Legacy TypeScript Effect package lanes under `packages/` are archived; do not add new production dependencies on `packages/*`.

## Git Hygiene (Non-Negotiable)

- Do not create `git worktree`s without explicit user approval.
- Do not use `git stash` without explicit user approval.
- If the worktree is dirty and you need a clean state, present options: commit, discard, or (with approval) temporary worktree.

## Where To Change Things

- **Core web control service:** `apps/openagents.com/service/` (Rust)
- **Core web UI shell:** `apps/openagents.com/web-shell/` (Rust/WGPUI WASM)
- iOS surface: `apps/autopilot-ios/`
- Desktop surface: `apps/autopilot-desktop/`
- Onyx surface: `apps/onyx/`
- Shared Rust crates: `crates/*`
- Canonical docs/contracts: `docs/` (start with `docs/README.md`)

Historical lanes (legacy-only, non-canonical for new work): Laravel/PHP app files under `apps/openagents.com/app`, `apps/openagents.com/resources`, and related legacy web runtime assets.

## Verification Entry Points

- **Default local CI changed-files gate (pre-commit hook):**
  ```bash
  ./scripts/local-ci.sh changed
  ```
- **Default local CI pre-push Rust gate (pre-push hook):**
  ```bash
  ./scripts/local-ci.sh all-rust
  ```
- **Workspace Rust compile baseline (mandatory migration gate):**
  ```bash
  cargo check --workspace --all-targets
  ```
- **Workspace compile lane via local CI wrapper:**
  ```bash
  ./scripts/local-ci.sh workspace-compile
  ```
- **Rust web control service:** `apps/openagents.com/service/README.md`
- **Rust web shell:** `apps/openagents.com/web-shell/README.md`
- **Runtime Cloud Run deploy/runbook:** `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
  - Canonical deploy command (deploy + mandatory migration validation):
    ```bash
    GCP_PROJECT=openagentsgemini \
    GCP_REGION=us-central1 \
    RUNTIME_SERVICE=runtime \
    MIGRATE_JOB=runtime-migrate \
    IMAGE=us-central1-docker.pkg.dev/openagentsgemini/runtime/runtime:<TAG> \
    apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh
    ```
- iOS: `apps/autopilot-ios/docs/README.md`
- Desktop: `apps/autopilot-desktop/README.md`

Production debugging and request correlation:
- `docs/autopilot/testing/PROD_E2E_TESTING.md`

## Output Expectations (Agent Runs)

The canonical output of an autonomous run is the **Verified Patch Bundle**:
- `PR_SUMMARY.md`
- `RECEIPT.json`
- `REPLAY.jsonl`

See:
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0002-verified-patch-bundle.md`
- `docs/execution/ARTIFACTS.md`
- `docs/execution/REPLAY.md`
