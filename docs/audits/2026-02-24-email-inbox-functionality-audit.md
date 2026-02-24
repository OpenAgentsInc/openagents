# Email and Inbox Functionality Audit (Desktop + Cross-Surface)

Date: 2026-02-24

## Scope

This audit covers all currently discoverable email/inbox functionality across:

- Desktop app inbox UX/data flow (`apps/autopilot-desktop`, `crates/autopilot_ui`, `crates/autopilot_app`)
- Shared inbox domain crate (`crates/autopilot-inbox-domain`)
- Rust control service auth + email/webhook integrations (`apps/openagents.com/service`)
- Runtime inbox/comms surfaces (`apps/runtime`, `crates/runtime`)
- iOS auth lane where it shares the same email-code API contract (`apps/autopilot-ios`)
- Legacy Laravel email/comms files still present in repo (`apps/openagents.com/app`, `routes`, `tests`, `config`)

## Preflight Authorities Checked

- `docs/adr/INDEX.md`
- `docs/plans/active/rust-migration-invariant-gates.md`
- `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
- `docs/adr/ADR-0002-proto-first-contract-governance.md`
- `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`
- `docs/adr/ADR-0008-bounded-vercel-sse-compatibility-lane.md`
- `docs/PROJECT_OVERVIEW.md`

Constraints applied during audit:

- Treat Rust control/runtime paths as canonical.
- Treat proto/contracts as authority for cross-surface auth/session semantics.
- Identify but do not treat legacy Laravel lanes as canonical behavior.

## Executive Summary

1. Desktop inbox is implemented as a local sample-data workflow, not a real mailbox integration.
2. Shared inbox domain logic exists (classification/draft heuristics) and is test-covered, but has no provider/network I/O.
3. Email-code authentication is implemented and active in the Rust control service, and consumed by desktop/iOS clients.
4. Email integration support exists in control service (Resend webhook ingest + Google OAuth token persistence), but this is not wired into a user-facing mailbox inbox pipeline.
5. Runtime has an "inbox" service, but it is an agent envelope queue, not end-user email inbox.
6. A contract drift risk exists: control-service forwarding defaults to `/internal/v1/comms/delivery-events`, while runtime router does not currently expose that route.
7. Legacy Laravel email/inbox/auth implementations remain in-tree as historical lanes and should not be treated as active architecture.

## Status Matrix

| Capability | Status | Evidence |
|---|---|---|
| Desktop inbox list/thread/approval/audit UI routing | Implemented (local state only) | `apps/autopilot-desktop/src/main.rs:3329`, `apps/autopilot-desktop/src/main.rs:4008`, `crates/autopilot_ui/src/lib.rs:3526`, `crates/autopilot_ui/src/lib.rs:5850` |
| Desktop inbox data source (real mailbox sync) | Missing | `apps/autopilot-desktop/src/inbox_domain.rs:33` |
| Desktop inbox draft approve/reject persistence/send | Missing (local flag toggles only) | `apps/autopilot-desktop/src/inbox_domain.rs:125`, `apps/autopilot-desktop/src/inbox_domain.rs:137` |
| Shared inbox policy/draft/audit domain logic | Implemented | `crates/autopilot-inbox-domain/src/lib.rs:133`, `crates/autopilot-inbox-domain/src/lib.rs:225`, `crates/autopilot-inbox-domain/src/lib.rs:307` |
| Email-code auth API (`/api/auth/email`, `/api/auth/verify`) | Implemented (Rust control service) | `apps/openagents.com/service/src/lib.rs:1202`, `apps/openagents.com/service/src/lib.rs:1215`, `apps/openagents.com/service/src/lib.rs:5895`, `apps/openagents.com/service/src/lib.rs:6078` |
| Session refresh + logout/revocation | Implemented | `apps/openagents.com/service/src/lib.rs:15363`, `apps/openagents.com/service/src/lib.rs:15421`, `apps/openagents.com/service/src/auth.rs:825`, `apps/openagents.com/service/src/auth.rs:981` |
| Desktop client auth lane | Implemented with compatibility header | `apps/autopilot-desktop/src/runtime_auth.rs:58`, `apps/autopilot-desktop/src/runtime_auth.rs:81`, `apps/autopilot-desktop/src/runtime_auth.rs:20` |
| iOS client auth lane | Implemented | `apps/autopilot-ios/Autopilot/Autopilot/RuntimeCodexClient.swift:45`, `apps/autopilot-ios/Autopilot/Autopilot/RuntimeCodexClient.swift:66` |
| Resend webhook ingest/idempotency/signature verification | Implemented | `apps/openagents.com/service/src/lib.rs:7983`, `apps/openagents.com/service/src/lib.rs:8210`, `apps/openagents.com/service/src/lib.rs:8229` |
| Runtime forwarding of normalized delivery events from control service | Implemented in control service (dependent on runtime endpoint availability) | `apps/openagents.com/service/src/lib.rs:8469`, `apps/openagents.com/service/src/lib.rs:8613`, `apps/openagents.com/service/src/config.rs:54` |
| Runtime comms delivery endpoint implementation | Missing in current runtime router | `apps/runtime/src/server.rs:773` |
| Runtime docs/spec for comms delivery endpoint | Present but likely stale/drifted | `apps/runtime/docs/openapi-internal-v1.yaml:24`, `apps/runtime/docs/RUNTIME_CONTRACT.md:107` |
| Google integration token persistence (`gmail.primary`) | Implemented in control domain store | `apps/openagents.com/service/src/domain_store.rs:1614`, `apps/openagents.com/service/src/domain_store.rs:1671` |
| Legacy Laravel email/auth + webhook code | Present, non-canonical | `apps/openagents.com/routes/auth.php:22`, `apps/openagents.com/app/Http/Controllers/Auth/EmailCodeAuthController.php:51`, `apps/openagents.com/app/Http/Controllers/Api/Webhooks/ResendWebhookController.php:15`, `docs/PROJECT_OVERVIEW.md:58` |

## Detailed Findings

### 1) Desktop inbox is a local simulation

- `DesktopInboxState::new()` seeds three hard-coded threads and computes category/risk/policy locally (`apps/autopilot-desktop/src/inbox_domain.rs:33`).
- Inbox actions only mutate in-memory state and emit `AppEvent::InboxUpdated` (`apps/autopilot-desktop/src/main.rs:4008`).
- No provider auth token use, mailbox fetch, or send API calls exist in the desktop inbox path.

Current maturity: UI/interaction prototype and local domain bridge, not production mailbox functionality.

### 2) Shared inbox domain crate is heuristic and reusable, not integrated with mailbox providers

- Classification is keyword-based (`apps/autopilot-desktop/src/inbox_domain.rs` delegates to `crates/autopilot-inbox-domain/src/lib.rs:133`).
- Draft text generation is template/local-style based (`crates/autopilot-inbox-domain/src/lib.rs:225`).
- Quality scoring/reporting exists (`crates/autopilot-inbox-domain/src/lib.rs:307`).

Current maturity: reusable domain primitives are in place; external data-plane is absent.

### 3) Email-code auth in Rust control service is fully implemented and active

- Routes are wired in Rust service router (`apps/openagents.com/service/src/lib.rs:1202`, `apps/openagents.com/service/src/lib.rs:1215`, `apps/openagents.com/service/src/lib.rs:1216`, `apps/openagents.com/service/src/lib.rs:1226`).
- Challenge + verify + session issuance/rotation/revocation behavior is implemented in `AuthService` (`apps/openagents.com/service/src/auth.rs:442`, `apps/openagents.com/service/src/auth.rs:472`, `apps/openagents.com/service/src/auth.rs:825`, `apps/openagents.com/service/src/auth.rs:981`).
- Proto authority exists for these semantics (`proto/openagents/control/v1/auth.proto:71`).

Current maturity: production-ready auth/session lane.

### 4) Desktop auth lane works, but has client-id/header drift

- Desktop code currently sends `x-client: openagents-expo` (`apps/autopilot-desktop/src/runtime_auth.rs:20`), with comment indicating compatibility fallback.
- Desktop README says it uses `X-Client: autopilot-desktop` (`apps/autopilot-desktop/README.md:22`).

Current maturity: functional but inconsistent code/docs identity labeling.

### 5) "Elsewhere" email integration exists in control service (Resend + Google), but not as inbox sync

- Resend integration secrets and metadata are persisted (`apps/openagents.com/service/src/domain_store.rs:1562`).
- Resend webhook pipeline is implemented: signature verification, idempotency, normalization, storage, retries, runtime forwarding (`apps/openagents.com/service/src/lib.rs:7983`, `apps/openagents.com/service/src/lib.rs:8469`, `apps/openagents.com/service/src/lib.rs:8613`).
- Google OAuth integration secret payload persists under `integration_id: "gmail.primary"` (`apps/openagents.com/service/src/domain_store.rs:1671`).

Current maturity: integration plumbing exists; mailbox read/send workflow into desktop inbox not implemented.

### 6) Runtime comms delivery contract appears drifted

- Control service default forwarding target is `/internal/v1/comms/delivery-events` (`apps/openagents.com/service/src/config.rs:54`).
- Runtime router does not expose that route in current build (`apps/runtime/src/server.rs:773` onward route list).
- Runtime docs still claim this endpoint (`apps/runtime/docs/openapi-internal-v1.yaml:24`, `apps/runtime/docs/RUNTIME_CONTRACT.md:107`).

Impact: webhook forwarding may fail at runtime depending on deployed target/router composition.

### 7) Runtime "inbox" is not user email inbox

- `InboxFs` and `DeadletterFs` are generic envelope queue services mounted in agent environment (`crates/runtime/src/services/inbox.rs:12`, `crates/runtime/src/services/deadletter.rs:9`, `crates/runtime/src/env.rs:67`).

Current maturity: runtime agent message queue exists; unrelated to desktop email inbox feature intent.

### 8) Legacy Laravel email/auth/webhook code still exists but is non-canonical

- Laravel routes/controllers still define `/api/auth/email`, `/api/auth/verify`, and Resend webhook logic (`apps/openagents.com/routes/auth.php:22`, `apps/openagents.com/app/Http/Controllers/Auth/EmailCodeAuthController.php:51`, `apps/openagents.com/app/Http/Controllers/Api/Webhooks/ResendWebhookController.php:15`).
- Repository overview explicitly marks `apps/openagents.com/app`, `routes`, `tests`, `config` as historical/non-canonical (`docs/PROJECT_OVERVIEW.md:58`).

Current maturity: archival/legacy lane remains in-tree and can cause operator confusion if not clearly ignored.

## Verification Performed

### Targeted tests run

- `cargo test -p autopilot-inbox-domain` -> passed (4 tests)
- `cargo test -p autopilot-desktop inbox_domain` -> passed (2 tests)
- `cargo test -p autopilot_ui inbox_` -> passed (4 tests)
- `cargo test -p runtime inbox` -> passed (1 test)
- `cargo test -p runtime test_control_plane_http` -> passed (1 test)
- `cargo test -p openagents-control-service refresh_rotates_refresh_token_and_logout_revokes_session` -> passed
- `cargo test -p openagents-control-service resend_webhook_forwarding_retries_and_projects_delivery` -> passed

### Discovery checks run

- Repo-wide scan for inbox/email surfaces (`rg --files | rg -i "inbox|email|gmail|resend|magic_auth|auth.*verify|comms.*delivery|mail"`).
- Runtime endpoint presence check (`rg -n "comms/delivery-events|delivery-events" apps/runtime/src crates/runtime`), confirming no runtime implementation match outside docs.

## Overall Status

- Auth/session email-code lane: strong and operational (Rust control authority).
- Desktop inbox lane: UX and domain scaffolding present, but not connected to real mailbox providers.
- Email integration lane: webhook/integration infrastructure present, but end-to-end inbox product behavior is incomplete.
- Critical architecture risk: control-runtime comms delivery endpoint drift.

## Recommended Follow-Ups (Priority Order)

1. Resolve contract drift by either implementing runtime `POST /internal/v1/comms/delivery-events` or changing control forwarding config/docs to the actual runtime endpoint.
2. Align desktop auth client identity header with intended desktop client id (and update compatibility strategy/docs consistently).
3. Define canonical mailbox data-plane contract (provider sync + thread model + draft send/approval persistence) and wire desktop inbox to it.
4. Keep legacy Laravel email/auth/webhook files clearly flagged as archival to prevent accidental operational reliance.
