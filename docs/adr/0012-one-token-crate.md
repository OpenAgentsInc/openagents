# 0012: One token crate

Status: Accepted (owner, 2026-10-24: "Merge. You decide how.").

## Context

Two crates defined design tokens:

- `crates/oa-tokens`: the OpenAgents token table. It holds the Apps SDK UI
  layers, Coder Light, Coder Noir, our product roles, typography, and
  resolved native palettes. The desktop app, `coder-ui`, `openagents-ui`
  (which generates the web component stylesheets), the chat app, and
  `verse-film` link it.
- `crates/aethyr-tokens`: the Kiki and Verse tokens (shadcn theme, HUD
  layer, blur, motion, spacing), added for
  [0006](0006-one-design-system.md) and the
  [UI protocol](../aethyr/ui-protocol.md). It writes
  `apps/verse-ui/src/aethyr/tokens.css`.

[0006](0006-one-design-system.md) asks for one design system with tokens
defined once.

## Decision

- **`oa-tokens` is the only token crate.** It absorbs `aethyr-tokens`, and
  `aethyr-tokens` is deleted. The tokens serve every OpenAgents surface, not
  only Aethyr, and `oa-tokens` already has the most consumers, so it keeps
  its name.
- **Aethyr's roles join the table**, in the same naming scheme:
  - a `hud` module for the HUD layer (glass, line, tick, glow, header height
    and its phone value);
  - blur, motion, and spacing scales beside `typography`.
- **One palette.** Where a Kiki shadcn color and an existing Coder Noir role
  mean the same thing, the Noir value wins and Kiki references the role. The
  dark Kiki surfaces then match the desktop and the mobile apps. Values with
  no Noir equivalent become product roles.
- **One generator per output, one source.** `oa-tokens` writes the web
  stylesheet that `apps/verse-ui` imports, as `openagents-ui` already does
  for its own components. The view tree keeps naming roles, never values.
- **Drift checks stay.** The generated stylesheet test, the
  `.openagents/generated.json` entry, and the TypeScript protocol digest
  point at `oa-tokens`.

## Consequences

- Kiki pages may shift slightly where a shadcn value is replaced by its Noir
  role. The change is reviewed in side-by-side screenshots at 390 px and
  1,440 px before it ships.
- Rust Native, GPUI, the mobile shells, and Aethyr's GPU UI all read the same
  values, so a card looks the same on every surface.
