# 0006: One design system

Status: Accepted (owner, 2026-10-24: "unify that one design system... make it
all look good and concise and kind of sci-fi game-like").

## Decision

- **One design system for every OpenAgents surface:** shadcn/ui primitives
  themed by OpenAgents tokens, plus the HUD layer (glass panels with corner
  ticks, mono labels, signal dots, HUD buttons, toasts). It lives today in
  `apps/verse-ui/src/theme.css`, built into
  `crates/openagents-web/content/verse-ui/ui.css`.
- **Tokens are the contract.** Color, type scale, spacing, radii, blur,
  motion, and the HUD values (`--hud-glass`, `--hud-line`, `--hud-tick`,
  `--hud-glow`) are defined once. The goal is a Rust token crate that emits
  CSS variables for the web and constants for Aethyr's GPU UI and the mobile
  shells, so a card looks the same wherever it is drawn.
- **No page-local styling.** Server-rendered pages use the shared classes
  (`hud-*`, `lp-*`, `verse-*`). No inline `<style>` blocks or private
  stylesheets for product surfaces. The content security policy keeps
  `style-src 'self'`.
- **Mobile first.** Each surface states what shows at phone width. Hover
  effects only apply under `@media (hover: hover)`. Panels become bottom
  sheets, and secondary controls hide.

## Consequences

- Legacy page styles and the studio's separate blue palette are removed, as
  in commit `c4560e65`.
- New components are added to the design system first, then used.
