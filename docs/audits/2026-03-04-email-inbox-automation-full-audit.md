# 2026-03-04 Email Inbox Automation Full Audit

Date: 2026-03-04  
Author: Codex  
Status: Complete

## Request Context

This audit inventories the **entire email inbox automation feature** currently in this repository so it can be removed in a follow-up change by moving feature code to `~/code/backroom` and deleting it from this repo.

This document is audit-only. No feature removal is performed here.

## Scope And Audit Method

Authorities reviewed before auditing:
- `docs/MVP.md`
- `docs/OWNERSHIP.md`

Repository audit method:
1. Enumerated all email/Gmail/inbox automation references across `apps/`, `crates/`, `docs/`, and `scripts/`.
2. Traced compile-time dependency edges from workspace root to desktop app and lockfile.
3. Traced runtime wiring end-to-end: pane registry -> pane system hit actions -> input dispatch -> reducers -> Gmail/network adapters.
4. Traced credential lifecycle and OAuth entrypoints.
5. Ran targeted validation gates/tests to confirm current operational state.

## Executive Summary

The email inbox automation feature is **implemented, active, and deeply integrated**.

Current footprint:
- 1 dedicated crate: `crates/email-agent` (16 Rust source files, 5,476 LOC).
- 3 primary desktop lane files: 3,389 LOC (`state/email_lane.rs`, `input/reducers/email.rs`, `panes/email.rs`).
- Multiple integration touchpoints in pane registry/system/renderer, input dispatch/tool bridge, credentials, and app state.
- Dedicated docs surface: `docs/email-agent/` (6 docs, 768 LOC), plus references in global docs.
- Dedicated release gates: `scripts/lint/email-agent-quality-gate.sh`, `scripts/lint/email-agent-release-gate.sh`.

This feature is **outside explicit MVP pane scope** in `docs/MVP.md` (MVP table includes `Job Inbox` but no email panes).

## MVP / Ownership Alignment Findings

### 1) Not in current MVP required pane set

`docs/MVP.md` canonical MVP pane table includes `Job Inbox` but no email lane panes (`docs/MVP.md:291-314`).

### 2) Ownership document does not include email-agent crate as active MVP surface

`docs/OWNERSHIP.md` active surfaces list does not include `crates/email-agent` (`docs/OWNERSHIP.md:11-96`).

### 3) Existing audit in repo is now stale/incorrect

`docs/audits/2026-03-04-email-agent-plan-implementation-audit.md` claims `gmail_refs=0` and “not implemented” (`lines 36-58`), which is contradicted by current code.

## Full Inventory

## A) Workspace + Dependency Edges

### Workspace membership
- `Cargo.toml:7` includes `"crates/email-agent"` as a workspace member.

### App dependency
- `apps/autopilot-desktop/Cargo.toml:29` depends on `openagents-email-agent` via path dependency.

### Lockfile dependency graph evidence
- `Cargo.lock:348` lists `openagents-email-agent` as a dependency of `autopilot-desktop`.
- `Cargo.lock:4046` defines package `openagents-email-agent`.

## B) Dedicated Email Feature Crate

### Crate root
- `crates/email-agent/Cargo.toml`
- `crates/email-agent/src/lib.rs`

### Module set (all in `crates/email-agent/src/`)
- `approval_workflow.rs`
- `draft_pipeline.rs`
- `e2e_harness.rs`
- `follow_up_scheduler.rs`
- `gmail_connector.rs`
- `gmail_sync.rs`
- `knowledge_base.rs`
- `normalization.rs`
- `observability.rs`
- `quality_scoring.rs`
- `retrieval.rs`
- `security_privacy.rs`
- `send_execution.rs`
- `style_profile.rs`
- `tenant_isolation.rs`
- `lib.rs`

### Public API surface exported from crate
`crates/email-agent/src/lib.rs` exports Gmail backfill/sync, normalization, retrieval, style profile, draft generation, approval workflow, send execution/idempotency, follow-up scheduling, observability, quality scoring, tenant isolation, security/privacy, and e2e harness APIs.

## C) Desktop Product Wiring

### Core app state and lane residency
- `apps/autopilot-desktop/src/app_state.rs:56-84` adds `PaneKind::{EmailInbox, EmailDraftQueue, EmailApprovalQueue, EmailSendLog, EmailFollowUpQueue}`.
- `apps/autopilot-desktop/src/app_state.rs:3113` stores `pub email_lane: EmailLaneState` in global `RenderState`.
- `apps/autopilot-desktop/src/render.rs:259` initializes `EmailLaneState::default()`.
- `apps/autopilot-desktop/src/render.rs:690` passes `email_lane` into pane renderer.

### Dedicated lane state and row models
- `apps/autopilot-desktop/src/state/email_lane.rs`
- Includes deterministic lane state, diagnostics, selection state, rows for inbox/drafts/approvals/sends/follow-ups, backfill/sync ingestion, and row rebuild logic.

### Dedicated pane rendering
- `apps/autopilot-desktop/src/panes/email.rs`
- Renders all five email lane panes and per-pane action/result/error headers.
- `apps/autopilot-desktop/src/panes/mod.rs:8` includes `pub mod email;`.

### Pane registry and command palette exposure
- `apps/autopilot-desktop/src/pane_registry.rs:458-526` defines five email pane specs.
- Command IDs exposed:
  - `pane.email_inbox`
  - `pane.email_draft_queue`
  - `pane.email_approval_queue`
  - `pane.email_send_log`
  - `pane.email_follow_up_queue`
- Registry tests enforce these mappings (`apps/autopilot-desktop/src/pane_registry.rs:936-952`).

### Pane system hit-testing and layout geometry
- Email pane action enums in `apps/autopilot-desktop/src/pane_system.rs:138-168`.
- Hit-action variants in `apps/autopilot-desktop/src/pane_system.rs:745-749`.
- Email button/row bounds helpers in `apps/autopilot-desktop/src/pane_system.rs:2064-2280`.
- Click routing by pane kind in `apps/autopilot-desktop/src/pane_system.rs:3763-3863`.

### Pane renderer dispatch
- `apps/autopilot-desktop/src/pane_renderer.rs:244-257` dispatches each email pane to renderer fns.

### Input dispatch and keyboard shortcuts
- Main hit-action dispatch routes email pane actions in `apps/autopilot-desktop/src/input.rs:2432-2441`.
- Enter-key email shortcuts in `apps/autopilot-desktop/src/input.rs:2705-2741`.

### Reducer integration
- `apps/autopilot-desktop/src/input/reducers/mod.rs:6` includes `mod email;`
- Exposes wrappers for email actions + Gmail helper functions (`lines 47-148`).
- Feature logic lives in `apps/autopilot-desktop/src/input/reducers/email.rs`:
  - Gmail backfill, incremental sync, send adapter (`lines 53-86`)
  - Gmail OAuth login flow (`line 88+`)
  - Inbox refresh/sync/draft/approval/send/follow-up actions (`lines 176-905`)
  - Live Gmail API adapters and payload decode path (`lines 1379-2078`)

### Tool bridge automation path
- Tool action mapping supports email pane actions in `apps/autopilot-desktop/src/input/tool_bridge.rs:1249-1301`.
- Email mapping tests in `apps/autopilot-desktop/src/input/tool_bridge.rs:4861-4904`.

## D) Credentials + Gmail OAuth Surface

### Credential templates and lifecycle
- Gmail credential constants in `apps/autopilot-desktop/src/credentials.rs:17-19`.
- Gmail credential templates included in `CREDENTIAL_TEMPLATES` (`apps/autopilot-desktop/src/credentials.rs:31-90`).
- OAuth lifecycle struct and parser:
  - `GoogleGmailOAuthLifecycle` (`apps/autopilot-desktop/src/credentials.rs:130-151`)
  - `parse_google_gmail_oauth_lifecycle` (`apps/autopilot-desktop/src/credentials.rs:368-393`)

### Credentials pane runtime hooks
- `apps/autopilot-desktop/src/app_state/credentials_state.rs:271-276` loads Gmail OAuth lifecycle.
- `apps/autopilot-desktop/src/input/actions.rs:4376-4379` routes `StartGmailOAuthLogin` to reducer.
- `apps/autopilot-desktop/src/pane_renderer.rs:1567,1673` draws Gmail Login button.
- `apps/autopilot-desktop/src/pane_system.rs:2009,3697-3700` defines bounds and click action for Gmail Login.

## E) Documentation and Runbooks

### Dedicated email-agent docs
- `docs/email-agent/ARCHITECTURE.md`
- `docs/email-agent/SYSTEM_GUIDE.md`
- `docs/email-agent/ONBOARDING.md`
- `docs/email-agent/OPERATIONS_PLAYBOOK.md`
- `docs/email-agent/SECURITY_PRIVACY_RUNBOOK.md`
- `docs/email-agent/TENANT_ISOLATION.md`

### Cross-doc references
- `docs/PANES.md` contains five email pane definitions and command-palette references.
- `docs/CREDENTIALS.md` documents `Gmail Login` behavior and workflow.

### Existing audit conflict
- `docs/audits/2026-03-04-email-agent-plan-implementation-audit.md` is now inconsistent with repository state and should be treated as superseded by this audit.

## F) Scripts and Quality Gates

### Dedicated scripts
- `scripts/lint/email-agent-quality-gate.sh`
- `scripts/lint/email-agent-release-gate.sh`

### Script behavior
- Quality gate runs: `cargo test -p openagents-email-agent quality_gate_thresholds_hold_for_golden_set`.
- Release gate runs quality gate plus `cargo test -p openagents-email-agent e2e_harness`.

## Validation Results (Captured During This Audit)

Executed on 2026-03-04:
1. `scripts/lint/email-agent-release-gate.sh` -> pass
- quality gate test: pass
- e2e harness tests: pass (5/5)

2. `cargo test -p autopilot-desktop email_lane_commands_map_to_expected_singleton_panes` -> pass
3. `cargo test -p autopilot-desktop pane_action_mapping_supports_email_panes` -> pass
4. `cargo test -p autopilot-desktop backfill_ingest_populates_inbox_rows_in_descending_time_order` -> pass

Note: `autopilot-desktop` test runs emitted existing dead-code warnings unrelated to this audit.

## Removal Impact Map (For Follow-Up Deletion Change)

If the feature is removed entirely, these domains must be addressed together:

1. Workspace/package graph
- Remove `crates/email-agent` from workspace and dependencies.
- Regenerate `Cargo.lock`.

2. Desktop pane surface
- Remove five email pane kinds, specs, renderer dispatch, pane geometry/hit actions, keyboard actions, and tool-bridge action mappings.

3. Desktop lane state and reducers
- Remove `EmailLaneState` and all email reducer wiring.

4. Gmail credential/OAuth path
- Remove Gmail credential templates/constants, Gmail Login UI action/button, OAuth callback flow.

5. Docs and operator scripts
- Remove `docs/email-agent/*`, strip email sections from shared docs, remove email-agent lint scripts.

6. Tests
- Remove/replace tests that assert email pane/action behavior.

## Archive Payload Candidate (For `~/code/backroom` Move)

Minimum payload to preserve feature history before deletion:
- `crates/email-agent/`
- `docs/email-agent/`
- `scripts/lint/email-agent-quality-gate.sh`
- `scripts/lint/email-agent-release-gate.sh`
- Email-lane app files:
  - `apps/autopilot-desktop/src/panes/email.rs`
  - `apps/autopilot-desktop/src/state/email_lane.rs`
  - `apps/autopilot-desktop/src/input/reducers/email.rs`

Plus integration files that will need either archival copies or deletion edits in-place:
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input/reducers/mod.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/credentials.rs`
- `apps/autopilot-desktop/src/app_state/credentials_state.rs`
- `apps/autopilot-desktop/src/panes/mod.rs`
- `apps/autopilot-desktop/src/state/mod.rs`
- `apps/autopilot-desktop/Cargo.toml`
- `Cargo.toml`
- `Cargo.lock`
- `docs/PANES.md`
- `docs/CREDENTIALS.md`

## Conclusion

The email inbox automation feature is not dormant; it is a live, integrated subsystem across crate, desktop runtime, docs, and release gates. This audit establishes the complete removal surface needed for the next step (archive to backroom + repo deletion).
