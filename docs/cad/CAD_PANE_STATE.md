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

## Deterministic Defaults

Defaults are stable and test-covered in `app_state::tests::cad_demo_state_defaults_are_deterministic`.
