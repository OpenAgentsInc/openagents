# Zed Parity Roadmap (Historical / Desktop Surface)

This roadmap captures the intended "parity" feature set for a Zed-like desktop experience (tabs, splits, file editing) within the OpenAgents desktop surface.

Status:
- Rust-era desktop architecture referenced by some ADRs was archived during the 2026-02-11 deprecation.
- The ADR contracts that reference this roadmap remain authoritative for intent, but implementation pointers may be historical.

## Phase 1 (Minimum Viable Editor)

- Open file into an editor tab.
- Track dirty state.
- Save file deterministically via a typed UI ↔ host contract.

Related contract:
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0023-file-editor-open-save-events.md`

## Phase 2 (Workspace UX)

- Splits and tab management.
- Recent files / quick open.
- File tree navigation.

## Phase 3 (Tooling + Integration)

- Search/replace, go-to definition.
- Integration with agent execution/runtime (tool receipts, replay).

