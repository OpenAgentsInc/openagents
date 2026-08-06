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

- `khalaTheme` — the canonical Khala Protoss-blue dark theme mounted by
  supported OpenAgents product surfaces.
- `defaultTheme` — the complete light semantic projection. It keeps role and
  contrast parity for consumers that explicitly opt into a light surface.
- `@openagentsinc/design-tokens/native` — React Native data derived from those
  themes, including standard and reduced-motion variants.
- `@openagentsinc/design-tokens/web.css` — generated CSS variables and Tailwind
  v4 `@theme` aliases. `:root` is dark; `[data-oa-theme="light"]` opts in to the
  light projection; `prefers-reduced-motion` zeros every shared duration.
- `@openagentsinc/design-tokens/contrast` — the WCAG contrast oracle used by
  the package check.
- `Theme` and its parts (`ColorTheme`, `SpacingTheme`, `ControlTheme`, …) —
  the theme type consumed by anything that lowers tokens into a host, e.g.
  `@openagentsinc/ui`'s `desktopThemeCssVariables`.
- The token vocabularies (`colorTokens`, `spacingTokens`, `controlTokens`, …)
  and their schemas, for surfaces that validate token names.
- `khalaPalette` and `withAlpha` — the tier-1 ramp steps and the only
  sanctioned way to mint a translucent color from them.

The attention precedence is a closed semantic set shared by every renderer:
`attentionApproval` (amber), `attentionInput` (indigo), `attentionWorking`
(sky), `attentionFailed` (red), and `attentionDone` (emerald). These are
text-bearing signal colors and pass WCAG AA on every semantic surface.

## Change workflow

Edit the typed themes in `src/index.ts`, then run:

```sh
pnpm --filter @openagentsinc/design-tokens run generate
pnpm --filter @openagentsinc/design-tokens run check
```

The first command rewrites `src/web.generated.css`. The second byte-compares
that generated projection, checks both themes for WCAG AA, and runs the token
and cross-renderer parity tests. A semantic rename therefore changes the web
and native projections in one source commit. Consumer applications may add
ergonomic aliases, but must not add new literal token values.

## Provenance

The contents moved verbatim out of the vendored Effect Native tokens
package during the Effect Native removal (openagents#9325, packet 2). The
token vocabulary was not redesigned by that move.

What stayed behind with the framework, and is therefore absent here: the
`ThemeService` Context tag and its Layers, the `defineTheme` / `decodeTheme` /
`encodeTheme` renderer plumbing, the Autopilot reference theme, and the Khala
motif-geometry resolvers.
