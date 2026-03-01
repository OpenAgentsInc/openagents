# CAD Pane State Container

This document defines the MVP `CadDemoPaneState` container in desktop app state.

## Location

- `apps/autopilot-desktop/src/app_state_domains.rs`
- `apps/autopilot-desktop/src/app_state.rs`

## Purpose

- Provide deterministic CAD pane bootstrap/session metadata before full reducer wiring.
- Keep app-layer orchestration state in `apps/autopilot-desktop` per ownership boundaries.

## Fields (MVP)

- `load_state`
- `last_error`
- `last_action`
- `session_id`
- `document_id`
- `document_revision`
- `active_variant_id`
- `variant_ids`
- `last_rebuild_receipt`
- `rebuild_receipts`
- `eval_cache` (capacity-bounded per-session cache store)
- `rebuild_worker` (lazy background worker handle)
- `next_rebuild_request_id`
- `pending_rebuild_request_id`
- `last_good_mesh_id`
- `warnings`
- `warning_filter_severity`
- `warning_filter_code`
- `warning_hover_index`
- `focused_warning_index`
- `focused_geometry_ref`
- `history_stack`
- `timeline_rows`
- `timeline_selected_index`
- `timeline_scroll_offset`
- `selected_feature_params`

## Rebuild Receipt Stream

`CadRebuildReceiptState` tracks deterministic eval-cycle telemetry:

- `event_id`
- `document_revision`
- `variant_id`
- `rebuild_hash`
- `duration_ms` (deterministic synthetic timing)
- `cache_hits`
- `cache_misses`
- `cache_evictions`
- `feature_count`

Receipts are:

- retained in-pane (`rebuild_receipts`, bounded history)
- surfaced as lane events through activity feed upserts with source tag `cad.eval`
- committed from background worker responses while preserving `last_good_mesh_id` during pending rebuilds

Warning panel state is session-persistent and filterable by severity/code.

Timeline state is selection-aware and auto-scrolls to keep the active feature row visible.

## Deterministic Defaults

Defaults are stable and test-covered in `app_state::tests::cad_demo_state_defaults_are_deterministic`.
