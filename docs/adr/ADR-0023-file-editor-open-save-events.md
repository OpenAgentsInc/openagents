# ADR-0023: File Editor Open/Save Event Contract

## Status

Accepted

## Date

2026-01-30

## Context

Phase 1 of the Zed parity roadmap introduces a workspace file editor with tabs and splits. The UI needs a stable, typed contract to request file saves and to receive deterministic success/failure feedback, aligning with the existing `OpenFile` request and `FileOpened`/`FileOpenFailed` events.

## Decision

We will extend the `UserAction` and `AppEvent` contracts to include file-save operations for the desktop UI.

- `UserAction::SaveFile { path, contents }` is the canonical request for writing editor contents to disk.
- `AppEvent::FileSaved { path }` and `AppEvent::FileSaveFailed { path, error }` are the canonical responses.
- The `path` value is treated as absolute by the UI and is resolved in the desktop host using the same path-expansion rules as `OpenFile`.

These events are now the stable interface for file saving between `autopilot_ui` and the desktop host.

## Scope

What this ADR covers:
- The UI ↔ host contract for file-save requests and responses.
- The additional `UserAction` and `AppEvent` variants and their payloads.

What this ADR does NOT cover:
- File change watching or external file refresh.
- Version control integration or conflict resolution.

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| `UserAction::SaveFile` fields | Stable: `path`, `contents` |
| `AppEvent::FileSaved` fields | Stable: `path` |
| `AppEvent::FileSaveFailed` fields | Stable: `path`, `error` |

Backward compatibility expectations:
- Existing `OpenFile` and `FileOpened`/`FileOpenFailed` semantics remain unchanged.

## Consequences

**Positive:**
- Enables editor dirty tracking + save feedback without ad-hoc UI state.
- Provides a stable contract for additional editor workflows (autosave, save-as).

**Negative:**
- Adds new UI ↔ host surface area that must be kept in sync.

**Neutral:**
- Does not alter workspace or project model semantics.

## Alternatives Considered

1. **Write files directly from UI** — Rejected; bypasses runtime logging and host enforcement.
2. **Reuse OpenFile for save** — Rejected; conflates read vs write semantics.
3. **Embed save in `Command`** — Rejected; lacks typed payload and explicit event response.

## References

- `crates/autopilot_app/src/lib.rs`
- `apps/autopilot-desktop/src/main.rs`
- `crates/autopilot_ui/src/lib.rs`
- `docs/zed/zed-parity-roadmap.md`
