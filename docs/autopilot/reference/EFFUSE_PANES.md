# Effuse Panes (Web Port Of Desktop HUD Pane System)

This repo's Autopilot Desktop UI (Rust/WGPUI) includes a Commander-style HUD pane system:

- blank canvas background
- floating panes (z-ordered, draggable, resizable)
- hotbar slots + keyboard shortcuts

The web UI in `apps/web` uses Effuse for rendering, and we sometimes want the same pane/hotbar UX for:

- operator/debug surfaces (trace viewers, receipts, compile reports)
- long-running "agent cockpit" layouts
- parity experiments with the desktop UX patterns

To support that, we added a browser/DOM port as a standalone TypeScript package:

- `packages/effuse-panes/`

## Start Here (Package Docs)

- `packages/effuse-panes/README.md`
- `packages/effuse-panes/docs/INDEX.md`

Key references:

- API: `packages/effuse-panes/docs/API_REFERENCE.md`
- Effuse integration: `packages/effuse-panes/docs/INTEGRATION_EFFUSE.md`
- Parity checklist: `packages/effuse-panes/docs/PARITY_CHECKLIST.md`
- Mobile brainstorming: `packages/effuse-panes/docs/MOBILE.md`

## Source-Of-Truth (Rust)

The behavior is derived from:

- `crates/autopilot_ui/src/lib.rs` (pane store + orchestration in `MinimalRoot`)
- `crates/wgpui/src/components/hud/pane.rs`
- `crates/wgpui/src/components/hud/resizable_pane.rs`
- `crates/wgpui/src/components/hud/hotbar.rs`

Additional historical context:

- `docs/autopilot-old/HUD.md` (HUD + pane system overview)
- `packages/effuse-panes/docs/PANE_DRAGGING.md` (drag/resize design + work log)

## What The Web Port Does (And Does Not Do)

The package is intentionally split into:

- a small, unit-testable **core** (`PaneStore`, `ResizablePane`, `HotbarModel`)
- a thin **DOM adapter** (`mountPaneSystemDom`) that wires pointer + keyboard events

In `apps/web`, pane lifecycle is wrapped in an Effect service:

- `apps/web/src/effect/paneSystem.ts`
- `PaneSystemService.mount(...)` returns `{ paneSystem, release }`
- `release` is used on overlay close instead of ad-hoc destroy calls

It does **not** render app-specific pane content. The host app must:

- decide what pane ids/kinds exist
- map hotbar slots to pane open/toggle actions
- render content into each pane's `[data-oa-pane-content]` node (Effuse, React, etc)

## How To Use It In apps/web

The recommended pattern is the same pattern we already use for stable backgrounds:

- mount once, then update only slots/content on subsequent renders

See:

- `apps/web/src/effuse-pages/marketingShell.ts`
- `apps/web/src/effuse-pages/authedShell.ts`

For the pane system specifically, follow:

- `packages/effuse-panes/docs/INTEGRATION_EFFUSE.md`

## Why This Matters For Autopilot

Once mounted, the pane system gives us a stable "desktop-like" container where we can add:

- DSE debug cards and trace viewers
- receipts/blob inspectors
- long-context evidence viewers (RLM traces / chunk previews)

without forcing everything into a single linear chat scroll.

This makes debugging and operator workflows substantially easier, especially for long-running agents.
