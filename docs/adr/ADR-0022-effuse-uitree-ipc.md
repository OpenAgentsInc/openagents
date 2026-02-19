# ADR-0022: Effuse UITree + UI Patch IPC Contract

## Status

Deprecated

## Date

2026-01-25

## Deprecated Date

2026-02-19

## Replacement

- `crates/autopilot_app/src/lib.rs` and `crates/autopilot_ui/src/lib.rs` for current desktop action/event contracts
- `apps/autopilot-desktop/src/main.rs` for current host-side event bridge behavior
- `docs/codex/unified-runtime-desktop-plan.md` for current desktop/runtime direction

## Context

This ADR captured a former Tauri IPC contract (`ui-event` channel) for a UI tree
patch model. The active desktop architecture no longer uses this Tauri boundary.

## Decision

We will use a **flat UITree model** with **JSON patch updates** as the canonical
IPC contract for signature-driven UI in Autopilot Desktop.

> We will represent UI as a `UITree` (root + elements map) and stream updates as
> `UiPatch` operations over the `ui-event` Tauri channel, validated against the
> Effuse component catalog.

This decision establishes:
- `UITree` as the canonical UI representation for Autopilot Desktop.
- `UiPatch` as the canonical incremental update format.
- `UiEvent` as the IPC payload for UI updates (`UiTreeReset`, `UiPatch`,
  `UiDataUpdate`).
- Effuse catalog as the component whitelist and schema authority.

## Scope

What this ADR covers:
- Autopilot Desktop UI tree representation.
- IPC payloads and event channel for UI updates.
- Validation requirements for AI-generated UI.

What this ADR does NOT cover:
- WGPUI HUD components (see ADR-0019).
- Nostr protocol schemas (see [PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md)).
- External marketplace or replay formats.

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Event channel | Stable: `ui-event` |
| UITree shape | Stable: `{ root: string, elements: Record<string, UIElement> }` |
| UIElement fields | Stable: `key`, `type`, `props`, `children?`, `visible?` |
| Patch ops | Stable: `add`, `remove`, `replace`, `set` |
| Patch path | JSON Pointer syntax (`/root`, `/elements/<key>/props/...`) |
| Catalog validation | Required before render |

Backward compatibility expectations:
- New UI components must be added to the catalog before use.
- Unknown component types are rejected in strict mode.

Versioning rules:
- If UITree semantics change, increment the UI contract version and update
  IPC types accordingly.

## Consequences

**Positive:**
- Deterministic, validated UI updates from signatures.
- Safe, catalog-constrained AI layouts.
- Clear IPC boundary with replayable UI updates.

**Negative:**
- UI trees are verbose compared to raw HTML.
- Patch correctness must be maintained by producers.

**Neutral:**
- Rendering remains Effuse-based (no VDOM).

## Alternatives Considered

1. **Stream raw HTML strings** — Rejected due to safety and validation risks.
2. **Embed UI events in UnifiedEvent** — Rejected to keep UI IPC isolated.
3. **Use React runtime** — Rejected to preserve Effuse/EZ runtime.

## References

- `apps/autopilot-desktop/src/effuse/ui`
- `apps/autopilot-desktop/src/components/catalog.ts`
- `apps/autopilot-desktop/src-tauri/src/contracts/ipc.rs`
- [GLOSSARY.md](../GLOSSARY.md) — Canonical terminology
