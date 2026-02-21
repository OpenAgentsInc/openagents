# openagents-ui-core (OA-RUST-025)

Shared UI-core crate for cross-surface WGPUI consumers.

## Scope

1. Shared design tokens (`tokens` module).
2. Shared layout/scene primitives (`primitives` module).

This crate is intentionally small and surface-agnostic so web, desktop, and iOS
shells can reuse the same foundational UI semantics.

## Current exports

- `tokens::APP_DISPLAY_NAME`
- `tokens::DESKTOP_WINDOW_TITLE`
- `tokens::palette::{canvas_bg, surface_card, border_subtle}`
- `tokens::spacing::{EDGE_MARGIN, CARD_HEIGHT, CARD_MAX_WIDTH}`
- `draw_shell_backdrop(scene, size)`
- `draw_shell_card(scene, size, ShellCardSpec)`

## Consumers

- `apps/openagents.com/web-shell`
- `apps/autopilot-desktop`
