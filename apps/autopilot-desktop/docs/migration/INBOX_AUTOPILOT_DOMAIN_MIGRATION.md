# Inbox Domain Migration Map

Issue: OA-RUST-049  
Date: 2026-02-21

## Goal

Fold reusable inbox-autopilot mailbox policy/draft/audit domain logic into desktop-owned shared Rust crates so `autopilot-desktop` can consume that logic without depending on `apps/inbox-autopilot`.

## Source to Target Mapping

1. Thread policy classification
   - Source: `apps/inbox-autopilot/daemon/src/pipeline.rs` (`classify_thread`)
   - Target: `crates/autopilot-inbox-domain/src/lib.rs` (`classify_thread`)

2. Draft composition templates
   - Source: `apps/inbox-autopilot/daemon/src/pipeline.rs` (`compose_local_draft`, style inference)
   - Target: `crates/autopilot-inbox-domain/src/lib.rs` (`compose_local_draft`, `infer_style_signature_from_bodies`)

3. Draft quality scoring
   - Source: `apps/inbox-autopilot/daemon/src/quality.rs`
   - Target: `crates/autopilot-inbox-domain/src/lib.rs` (`build_draft_quality_report`, quality structs)

4. Domain enums and parse helpers
   - Source: `apps/inbox-autopilot/daemon/src/types.rs` + `apps/inbox-autopilot/daemon/src/db.rs`
   - Target: `crates/autopilot-inbox-domain/src/lib.rs`
   - Included:
     - `ThreadCategory`, `RiskTier`, `PolicyDecision`, `DraftStatus`
     - `parse_thread_category`, `parse_risk_tier`, `parse_policy`, `parse_draft_status`
     - `risk_to_str`

## Current Consumers

1. Inbox daemon
   - `apps/inbox-autopilot/daemon/src/pipeline.rs` now imports classification/draft logic from `autopilot-inbox-domain`.
   - `apps/inbox-autopilot/daemon/src/db.rs` now imports parse/string conversion helpers and shared domain structs from `autopilot-inbox-domain`.
   - `apps/inbox-autopilot/daemon/src/quality.rs` now re-exports shared quality report logic from `autopilot-inbox-domain`.

2. Desktop app
   - `apps/autopilot-desktop/src/inbox_domain.rs` now consumes shared domain crate directly for category classification and draft preview generation.

## Ownership

1. Shared domain logic authority: `crates/autopilot-inbox-domain`.
2. Desktop integration authority: `apps/autopilot-desktop`.
3. Legacy daemon remains a consumer only and should not become the source-of-truth for duplicated domain helpers again.
