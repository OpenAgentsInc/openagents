# 2026-02-25 App Surface Removal Audit

Status: planning audit (no removals executed in this change)  
Date: 2026-02-25  
Scope owner: repo-wide (apps, workspace manifests, scripts, docs, deploy runbooks)

## Requested End State

1. Web app functionality is removed.
2. Web presence is reduced to a simple page with a desktop download link.
3. Mobile app is removed entirely.
4. Onyx is moved to `~/code/backroom` and removed from this repo.
5. `apps/` remains for retained app surfaces (`apps/openagents.com`, `apps/autopilot-desktop`, `apps/runtime`, `apps/lightning-ops`, `apps/lightning-wallet-executor`).
6. Reusable libraries remain in `crates/`; only targeted app surfaces are removed.

## Preflight Authority Check

Reviewed before drafting this audit:

1. `docs/adr/INDEX.md`
2. `docs/plans/rust-migration-invariant-gates.md`

Constraints applied to this cleanup plan:

1. Preserve proto-first contracts and Rust authority boundaries (`INV-01`, `INV-02`).
2. Do not introduce alternate authority transports (`INV-03`, `INV-06`).
3. Keep deploy/runtime lane isolation explicit during path moves (`INV-08` to `INV-10`).
4. Avoid introducing `.github` workflow automation (`INV-12`).

## Current State Snapshot

### `apps/` inventory

Tracked files by app directory:

1. `apps/openagents.com`: 720
2. `apps/runtime`: 141
3. `apps/lightning-ops`: 61
4. `apps/autopilot-ios`: 53
5. `apps/lightning-wallet-executor`: 37
6. `apps/onyx`: 19
7. `apps/autopilot-desktop`: 14

Local disk usage (includes untracked local artifacts):

1. `apps/autopilot-ios`: 9.5G
2. `apps/openagents.com`: 709M
3. `apps/lightning-wallet-executor`: 297M
4. `apps/lightning-ops`: 104M
5. `apps/runtime`: 9.6M
6. `apps/autopilot-desktop`: 604K
7. `apps/onyx`: 408K

### Web surface (current)

`apps/openagents.com` is not just a static site. It currently contains:

1. Rust control service (`apps/openagents.com/service`, 80 tracked files).
2. Rust/WASM web shell (`apps/openagents.com/web-shell`, 38 tracked files).
3. Legacy Laravel/Inertia/React and deployment assets (602 tracked files outside `service/` and `web-shell/`).

Active Rust control service currently hosts:

1. auth/session endpoints (`/api/auth/*`),
2. sync token endpoints (`/api/sync/token`, `/api/v1/sync/token`),
3. runtime thread endpoints (`/api/runtime/threads*`),
4. route split and compatibility lanes (`route_split`, Vercel SSE adapter),
5. static asset + manifest + service-worker hosting.

Implication: "web reduced to simple download page" is a product/UI cut plus backend surface reduction decision, not only a frontend file delete.

### Mobile surface (current)

`apps/autopilot-ios` includes:

1. Swift/SwiftUI host app (`Autopilot.xcodeproj`, app/test targets),
2. Rust-core bridge files and reproducibility scripts,
3. iOS-specific guardrail and parity docs.

Mobile is coupled into CI and docs:

1. `scripts/local-ci.sh` lanes `ios-rust-core` and `ios-codex-wgpui`.
2. `scripts/docs-check.mjs` includes mandatory iOS docs.
3. cross-surface harness scripts/docs reference the iOS project path.

### Onyx surface (current)

`apps/onyx` contains a standalone Rust desktop app crate and docs.

Onyx is also coupled to:

1. workspace membership (`Cargo.toml`),
2. architecture/docs references (`AGENTS.md`, `docs/core/*`, `docs/sync/*`),
3. protocol + ADR contracts:
   - `docs/adr/ADR-0007-onyx-integration-surface-and-non-goals.md`
   - `docs/protocol/onyx-integration-contract-v1.md`
4. runtime sync policy/tests (`apps/runtime/src/sync_auth.rs`, `apps/runtime/src/server/tests.rs`).

## Critical Coupling That Must Be Addressed

### Workspace and cargo graph

`Cargo.toml` workspace members currently include app crates under `apps/`:

1. `apps/autopilot-desktop`
2. `apps/openagents.com/service`
3. `apps/openagents.com/web-shell`
4. `apps/runtime`
5. `apps/lightning-ops`
6. `apps/lightning-wallet-executor`
7. `apps/onyx`

These entries show where targeted removals will touch workspace members and path references.
Retained `apps/` surfaces should stay in place. Required workspace/path changes are limited to removing targeted members and references:

1. `apps/autopilot-ios` (remove from workspace and references).
2. `apps/onyx` (archive/remove and remove from workspace/references).
3. `apps/openagents.com/web-shell` (remove only if no longer needed for the simple landing-page target).

### CI and automation

Primary blocker files:

1. `scripts/local-ci.sh` (many trigger patterns and lanes hardcoded to `apps/*`).
2. `scripts/docs-check.mjs` (hardcoded app doc files, including web + iOS paths).
3. `scripts/release/validate-rust-cutover.sh` (control/runtime deploy checks under `apps/*`).
4. `scripts/run-cross-surface-contract-harness.sh` and related docs.

### Documentation and architecture authority

High-reference files that must be updated in same migration:

1. `AGENTS.md`
2. `README.md`
3. `docs/core/ARCHITECTURE.md`
4. `docs/core/PROJECT_OVERVIEW.md`
5. `docs/core/AGENT_MAP.md`
6. `docs/core/DEPLOYMENT_RUST_SERVICES.md`
7. `docs/core/LOCAL_CI.md`
8. `docs/sync/ROADMAP.md`
9. `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`

### Policy/contract artifacts tied to removed surfaces

If iOS and Onyx are removed, these become stale and require supersede/archive decisions:

1. `docs/adr/ADR-0007-onyx-integration-surface-and-non-goals.md`
2. `docs/protocol/onyx-integration-contract-v1.md`
3. iOS-specific docs under `apps/autopilot-ios/docs/`
4. iOS/Onyx references in protocol fixtures and compatibility docs.

## Required Cleanup Plan (Full Execution)

## Phase 0: Lock final target topology (required before deletions)

Decide and document:

1. Whether control-plane APIs currently used by desktop remain active.
2. Retained services stay under existing `apps/` locations unless explicitly changed:
   - runtime
   - control service (if retained)
   - lightning services
   - desktop app
3. Canonical destination for the static download page.
4. Desktop download URL source of truth (release page/object storage path).

Without this, removals can break desktop/runtime dependencies and release flows.

## Phase 1: Web product reduction to landing-only

1. Remove web-shell runtime/UI lane.
2. Remove route-split/SSE compatibility code and config if no web app functionality remains.
3. Serve only a minimal static landing page with desktop download link.
4. Keep only backend APIs explicitly needed by non-web clients (if any).
5. Archive legacy `apps/openagents.com/app`, `routes`, `resources`, `tests`, and associated deploy artifacts to backroom if not needed.

## Phase 2: Mobile complete removal

1. Remove `apps/autopilot-ios` entirely.
2. Remove iOS lanes/scripts from local CI and docs-check.
3. Remove iOS references from architecture, sync, protocol, and runbooks.
4. Remove iOS harness docs or archive them.

## Phase 3: Onyx archive + deletion

1. Copy `apps/onyx/` to `~/code/backroom` archive.
2. Remove `apps/onyx` from repo.
3. Remove Onyx workspace entry.
4. Remove Onyx-specific runtime policy/tests and contract docs (`ADR-0007`, `onyx-integration-contract-v1`) or replace with superseded records.

## Phase 4: Retained Apps Stabilization (No `apps/` Eradication)

Keep retained app surfaces in `apps/` and stabilize references after removals:

1. Keep:
   - `apps/openagents.com` (reduced to landing-page web presence + any retained control API path).
   - `apps/autopilot-desktop`
   - `apps/runtime`
   - `apps/lightning-ops`
   - `apps/lightning-wallet-executor`
2. Remove from workspace/path references only:
   - `apps/autopilot-ios`
   - `apps/onyx`
   - `apps/openagents.com/web-shell` if landing-only mode removes it.
3. Update scripts/docs that referenced removed surfaces.

## Phase 5: Global reference cleanup

1. Replace or remove all `apps/...` references in docs/scripts/readmes.
2. Update release runbooks and canonical command examples.
3. Update protocol fixtures and compatibility docs where removed surfaces are still encoded.

## Phase 6: Verification and acceptance gates

Required checks after migration:

1. `test -d apps`
2. `test ! -d apps/autopilot-ios`
3. `test ! -d apps/onyx`
4. `cargo check --workspace` passes with updated workspace members.
5. `./scripts/local-ci.sh docs` passes with updated docs-check expectations.
6. Any retained deploy smoke scripts pass with existing retained paths.
7. Web endpoint behavior verified:
   - root page renders download-only landing
   - no interactive web app routes remain.

## Backroom Archival Requirements

For requested archive/delete behavior:

1. archive target should include timestamped folder, e.g.:
   - `~/code/backroom/openagents-app-archive/<YYYY-MM-DD>-app-surface-removal/`
2. at minimum archive:
   - `apps/onyx/`
   - removed web/mobile code and docs selected for retention outside this repo.
3. keep a manifest in repo docs listing archive path + contents moved.

## Risks and Failure Modes

1. Over-removing control APIs can break desktop auth/runtime flows.
2. Deleting iOS/Onyx without cleaning CI/docs causes persistent gate failures.
3. Removing web-shell/control pieces without preserving required desktop/runtime API contracts can break active clients.
4. ADR/protocol drift if removed surfaces stay described as active.
5. Accidental deletion of retained `apps/` surfaces can break deploy and local CI lanes.

## Recommended Execution Order

1. Final topology decision (Phase 0) and write it into canonical docs.
2. Reduce web surface to landing-only while preserving required retained APIs.
3. Remove `apps/autopilot-ios`.
4. Archive/remove `apps/onyx`.
5. Finish with docs/CI/runbook cleanup and validation.

This order minimizes broken intermediate states and keeps verification lanes meaningful throughout migration.
