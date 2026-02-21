# OpenAgents: Agent Contract (READ THIS FIRST)

This file is a **map**, not a manual. Start here, then follow links for deeper specs/runbooks.

## Map

### Core orientation

- Progressive disclosure map: `docs/AGENT_MAP.md`
- Documentation index: `docs/README.md`
- Repository map / ownership: `docs/PROJECT_OVERVIEW.md`
- Priorities and sequencing: `docs/ROADMAP.md`
- Vocabulary authority: `docs/GLOSSARY.md`
- Architecture decisions (invariants/contracts): `docs/adr/INDEX.md`
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
- Mobile app: `apps/mobile/`
- Desktop app: `apps/desktop/`
- Onyx app: `apps/onyx/`
- Shared packages: `packages/`

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

For Effect packages (`packages/dse`, `packages/effuse`, `packages/effuse-test`): run `npm run effect:patch` after install for diagnostics.

## Git Hygiene (Non-Negotiable)

- Do not create `git worktree`s without explicit user approval.
- Do not use `git stash` without explicit user approval.
- If the worktree is dirty and you need a clean state, present options: commit, discard, or (with approval) temporary worktree.

## Where To Change Things

- **Core web app:** `apps/openagents.com/` — Laravel 12 + Inertia + React (TypeScript). See `docs/plans/active/laravel-rebuild.md`.
- Mobile surface: `apps/mobile/`
- Desktop surface: `apps/desktop/`
- Onyx surface: `apps/onyx/`
- Shared packages: `packages/*`
- Canonical docs/contracts: `docs/` (start with `docs/README.md`)

## Verification Entry Points

- **Laravel web (core):** `apps/openagents.com/README.md` (or app root; see `docs/plans/active/laravel-rebuild.md`).
- **Laravel web production deploy/runbooks:** `apps/openagents.com/docs/GCP_DEPLOY_PLAN.md` and `apps/openagents.com/docs/PRODUCTION_ENV_AND_SECRETS.md` (env/secrets + Cloud Run deploy flow).
- **Runtime Cloud Run deploy/runbook:** `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
  - Mandatory after each runtime deploy:
    ```bash
    GCP_PROJECT=openagentsgemini \
    GCP_REGION=us-central1 \
    RUNTIME_SERVICE=runtime \
    MIGRATE_JOB=runtime-migrate \
    apps/runtime/deploy/cloudrun/run-migrate-job.sh
    ```
- Mobile: `apps/mobile/README.md`
- Desktop: `apps/desktop/README.md`

Production debugging and request correlation:
- `docs/autopilot/testing/PROD_E2E_TESTING.md`

## Output Expectations (Agent Runs)

The canonical output of an autonomous run is the **Verified Patch Bundle**:
- `PR_SUMMARY.md`
- `RECEIPT.json`
- `REPLAY.jsonl`

See:
- `docs/adr/ADR-0002-verified-patch-bundle.md`
- `docs/execution/ARTIFACTS.md`
- `docs/execution/REPLAY.md`
