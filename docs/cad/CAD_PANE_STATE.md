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

## Deterministic Defaults

Defaults are stable and test-covered in `app_state::tests::cad_demo_state_defaults_are_deterministic`.
