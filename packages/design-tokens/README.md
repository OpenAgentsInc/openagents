# @openagentsinc/design-tokens

The OpenAgents design-token vocabulary as plain typed data: one closed lattice
of spacing, color, radius, type-scale, breakpoint, dimension, motion, elevation
and control tokens, the tier-1 primitive color palette those tokens derive
from, the tier-2 tone x variant x state color matrix, and the theme values
built from them.

This package holds token data and its schemas. It holds no renderer, no
component, no service wiring, and no framework: it depends on `effect` only,
for `Schema`.

## Exports

- `khalaTheme` — the single Khala Protoss-blue dark theme. Product surfaces
  mount exactly this one; there is no light variant and no runtime switch.
- `defaultTheme` — the neutral fixture theme.
- `Theme` and its parts (`ColorTheme`, `SpacingTheme`, `ControlTheme`, …) —
  the theme type consumed by anything that lowers tokens into a host, e.g.
  `@openagentsinc/ui`'s `desktopThemeCssVariables`.
- The token vocabularies (`colorTokens`, `spacingTokens`, `controlTokens`, …)
  and their schemas, for surfaces that validate token names.
- `khalaPalette` and `withAlpha` — the tier-1 ramp steps and the only
  sanctioned way to mint a translucent color from them.

## Provenance

The contents moved verbatim out of the vendored Effect Native tokens
package during the Effect Native removal (openagents#9325, packet 2). The
token vocabulary was not redesigned by that move.

What stayed behind with the framework, and is therefore absent here: the
`ThemeService` Context tag and its Layers, the `defineTheme` / `decodeTheme` /
`encodeTheme` renderer plumbing, the Autopilot reference theme, and the Khala
motif-geometry resolvers.
