# Inbox Domain Migration Map

Issue: OA-RUST-049  
Date: 2026-02-21

## Goal

Fold reusable inbox-autopilot mailbox policy/draft/audit domain logic into desktop-owned shared Rust crates so `autopilot-desktop` can consume that logic without depending on the legacy standalone inbox-autopilot app root (removed in `OA-RUST-052`).

## Source to Target Mapping

1. Thread policy classification
   - Source: legacy inbox-autopilot daemon pipeline (`classify_thread`) from git history
   - Target: `crates/autopilot-inbox-domain/src/lib.rs` (`classify_thread`)

2. Draft composition templates
   - Source: legacy inbox-autopilot daemon pipeline (`compose_local_draft`, style inference) from git history
   - Target: `crates/autopilot-inbox-domain/src/lib.rs` (`compose_local_draft`, `infer_style_signature_from_bodies`)

3. Draft quality scoring
   - Source: legacy inbox-autopilot daemon quality module from git history
   - Target: `crates/autopilot-inbox-domain/src/lib.rs` (`build_draft_quality_report`, quality structs)

4. Domain enums and parse helpers
   - Source: legacy inbox-autopilot daemon types/db modules from git history
   - Target: `crates/autopilot-inbox-domain/src/lib.rs`
   - Included:
     - `ThreadCategory`, `RiskTier`, `PolicyDecision`, `DraftStatus`
     - `parse_thread_category`, `parse_risk_tier`, `parse_policy`, `parse_draft_status`
     - `risk_to_str`

## Current Consumers

1. Desktop app
   - `apps/autopilot-desktop/src/inbox_domain.rs` consumes shared domain crate directly for category classification and draft preview generation.

2. Historical producer (removed root)
   - Legacy inbox-autopilot daemon implementation remains recoverable via git history for provenance/reference only.

## Ownership

1. Shared domain logic authority: `crates/autopilot-inbox-domain`.
2. Desktop integration authority: `apps/autopilot-desktop`.
3. Legacy daemon is removed; no new source-of-truth drift back into removed app roots.

## OA-RUST-051 Route Integration Follow-up

1. Codex and Inbox now share one desktop command/event lane through `UserAction` + `AppEvent` in `crates/autopilot_app`.
2. Desktop shell maintains a single cross-domain route state (`DesktopRouteState`) that tracks:
   - active surface (`Codex`, `Inbox`, or `Mixed`)
   - active Codex thread context
   - selected Inbox thread context
   - active Inbox pane route
3. Route state is recomputed on pane/navigation/input/event transitions in `crates/autopilot_ui`, ensuring Codex and Inbox state stay coordinated inside one WGPUI shell.
