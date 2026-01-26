# Migration Harmonization (2026-01-25 14:30)

## Summary

- Split the Autopilot desktop Rust backend into a workspace crate and left
  `apps/autopilot-desktop/src-tauri` as a thin Tauri wrapper.
- Reconciled DSPy LM provider detection in `crates/adjutant` to support the
  AI Gateway configuration used by the desktop app.
- Updated desktop docs to reflect the new crate layout and Effuse frontend,
  and moved the Rust↔TS IPC ADR into the root ADR index.

## Changes

### New backend crate

- Created `crates/autopilot-desktop-backend` and moved the following modules
  from the Tauri app crate:
  - `acp.rs`, `agent/`, `ai_server/`, `backend/`, `codex.rs`, `codex_home.rs`
  - `contracts/`, `event_sink.rs`, `file_logger.rs`, `state.rs`, `types.rs`
- `apps/autopilot-desktop/src-tauri/src/lib.rs` now calls the backend builder
  and runs with `tauri::generate_context!()`.
- `apps/autopilot-desktop/src-tauri/src/main.rs` is now a minimal entrypoint.
- `apps/autopilot-desktop/src-tauri/src/bin/gen_types.rs` now calls
  `autopilot_desktop_lib::contracts::export_ts`.
- Environment loading and tmp path resolution were updated to locate
  `apps/autopilot-desktop/.env` and `apps/autopilot-desktop/tmp` reliably.

### LM provider alignment

- Added **AI Gateway** to `crates/adjutant/src/dspy/lm_config.rs`:
  - `AI_GATEWAY_API_KEY` enables the provider.
  - `AI_GATEWAY_BASE_URL` or `AI_SERVER_HOST`/`AI_SERVER_PORT` set base URL.
  - Added env overrides for model/max tokens/temperature.
- Updated `crates/adjutant/docs/README.md` to reflect the new priority order
  and environment variables.

### Documentation updates

- Updated app docs to reference `crates/autopilot-desktop-backend` paths and
  Effuse UI structure:
  - `apps/autopilot-desktop/docs/ai-gateway-setup.md`
  - `apps/autopilot-desktop/docs/adjutant-agent.md`
  - `apps/autopilot-desktop/docs/autopilot/ARCHITECTURE.md`
  - `apps/autopilot-desktop/docs/autopilot/IMPLEMENTATION.md`
  - `apps/autopilot-desktop/docs/README.md`
- Moved ADR for Rust↔TS IPC contract generation:
  - `apps/autopilot-desktop/docs/adr/ADR-0002-rust-ts-type-generation.md`
    → `docs/adr/ADR-0021-rust-ts-type-generation.md`
  - Updated `docs/adr/INDEX.md` with ADR-0021 entry.

## Follow-ups

1. Run a workspace build to refresh `Cargo.lock` if needed:
   - `cargo check -p autopilot-desktop`
2. Verify `cargo autopilot` launches the Tauri dev flow and AI server
   initialization works with `apps/autopilot-desktop/.env`.
3. Consider removing the duplicate ADR-0001 copy in
   `apps/autopilot-desktop/docs/adr/` if not needed.

## Update

- The desktop backend has since moved back into
  `apps/autopilot-desktop/src-tauri/src/`, and AI server management is now
  provided by `crates/ai-server/`.
