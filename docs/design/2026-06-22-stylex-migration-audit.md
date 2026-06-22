# StyleX Migration Audit

Date: 2026-06-22
Status: Design-system audit and migration recommendation
Scope: `openagents` web, desktop, mobile, `@openagentsinc/ui`, `@openagentsinc/autopilot-ui`, Foldkit integration, and local reference repos:

- `/Users/christopherdavid/work/projects/repos/stylex`
- `/Users/christopherdavid/work/projects/repos/react-native-stylex`

## Implementation progress

2026-06-22 P0 web/desktop vertical slice, issue #5952:

- Added `@openagentsinc/ui/stylex-foldkit`, a Foldkit adapter around
  `stylex.attrs(...)`.
- Added a seed `packages/ui/src/tokens.stylex.ts` dark token module.
- Migrated `@openagentsinc/ui` `emptyState` root/body layout to StyleX while
  preserving the existing Foldkit function API.
- Migrated the shared `@openagentsinc/autopilot-ui` `SessionList` /
  `SessionRow` layout to StyleX. That component is consumed by both
  `apps/openagents.com/apps/web` and `apps/autopilot-desktop`.
- Added Vite StyleX extraction to `apps/openagents.com/apps/web`. The
  production build emits generated StyleX classes in the app CSS asset.
- Added desktop StyleX extraction and webview entry precompilation.
  The desktop CSS build writes Tailwind first, appends generated StyleX CSS,
  and emits the browser entry consumed by Electrobun with `stylex.create(...)`
  compiled out.
- Added package-test runtime fallback wiring so uncompiled Bun unit tests can
  keep rendering migrated Foldkit components while production app builds still
  use the StyleX compiler.
- Mobile work is intentionally deferred from #5952 and the first issue sequence
  per the follow-up instruction to ignore mobile for now.

2026-06-22 P1 shared token authority, issue #5953:

- Added `@openagentsinc/design-tokens` as the neutral source of truth for the
  shared Autopilot dark palette. The package intentionally has no Foldkit,
  StyleX, React, app, or protocol runtime dependency.
- Kept `@openagentsinc/autopilot-ui/tokens` as a compatibility facade for
  existing `darkTokens`, `cssVars(tokens)`, and `nativeTheme(tokens)` imports.
- Rewired `@openagentsinc/autopilot-control-protocol` `CANONICAL_DARK` to use
  the same design-token authority while preserving flattened protocol parity
  output and tests.
- Added `@openagentsinc/ui/tokens` re-exports so shared UI code can consume
  canonical token helpers without depending on Autopilot UI.
- Converted `packages/ui/src/tokens.stylex.ts` from duplicated hard-coded
  palette values to StyleX custom-property aliases for the existing Foldkit CSS
  variables (`--bg`, `--text`, tone vars, and related dark tokens).
- Forum, public landing, tenant theme, spacing, typography, and mobile app
  migration remain intentionally out of scope until later phases define those
  contracts.

2026-06-22 P2 shared UI primitive migration, issue #5954:

- Migrated the first `@openagentsinc/ui` low-level render helpers in
  `shared.ts` to StyleX: heading blocks, buttons, link buttons, text links,
  avatars, avatar groups, button groups, and dropdown menus.
- Migrated the first form controls in `forms.ts` to StyleX: compact buttons,
  input groups, validation chrome, textarea groups, and select menus. Legacy
  exported class strings remain available for not-yet-migrated form families.
- Migrated the Prompt Input AI Elements module to StyleX while preserving its
  exported class constants and `data-ui-base` contracts for compatibility and
  auditability.
- Improved the Foldkit StyleX adapter so test/runtime fallback styles accept
  conditional style entries and emit deterministic fallback classes in Bun unit
  tests.
- Added package-level render coverage proving the migrated shared, form, and AI
  Elements surfaces route through the StyleX adapter without changing Foldkit
  component APIs.

## Executive verdict

A full migration toward StyleX is technically viable for the OpenAgents DOM
surfaces, including `apps/openagents.com`, the Autopilot desktop webview, and
the shared Foldkit component packages. The key enabler is that the local StyleX
source exposes `stylex.attrs(...)`, which returns DOM-shaped `class`, `style`,
and `data-style-src` attributes in addition to the React-oriented
`stylex.props(...)` API. Foldkit already accepts typed attributes through
`h.Class(...)`, `h.Style(...)`, `h.Attribute(...)`, and `h.DataAttribute(...)`,
so a thin `@openagentsinc/ui` helper can adapt compiled StyleX output to
Foldkit without changing Foldkit's rendering model.

The migration should not be framed as "delete every CSS file." StyleX is a
good target for component styles, variants, responsive states, tokens, and
themeable component surfaces. Global CSS should remain for font faces, root
element defaults, `html` / `body` / `#root`, app-wide data-attribute theme
scopes, third-party quirks, generated icon edge cases, and any raw HTML that is
not compiled through the StyleX transform yet.

For React Native, the new reference repo changes the answer from "unknown" to
"plausible, but separate." `react-native-stylex` is not the Meta
`@stylexjs/stylex` compiler API. It is a runtime React Native styling helper
around `StyleSheet.create(...)`, `ThemeProvider`, `makeUseStyles(...)`,
`withStyles(...)`, and device dependencies such as dimensions, appearance,
orientation, safe area, and i18n. It can support a mobile migration from the
current hand-authored `StyleSheet.create(...)` files, but it should be treated
as a native adapter track that shares tokens with web/desktop rather than as
one identical StyleX style object shared across DOM and native.

Recommended path: adopt StyleX as the future component styling system for
`@openagentsinc/ui` and `@openagentsinc/autopilot-ui`, prove the Foldkit adapter
in a vertical slice, keep Tailwind and existing CSS during migration, and only
retire Tailwind after the shared packages, web app, desktop webview, and mobile
token story are all green.

## Current OpenAgents styling inventory

### Shared Foldkit packages

`packages/ui` is the shared Foldkit component library published as
`@openagentsinc/ui`. It is source-consumed with no package build step, mirrors
the `@openagentsinc/autopilot-ui` package convention, and currently composes
Tailwind utility strings directly. The package README explicitly says it has no
tokens export yet and that `@openagentsinc/autopilot-ui` keeps Autopilot-specific
tokens.

The package is already large enough to justify stronger styling contracts:

- `packages/ui/src/workroom.ts`: 1,902 lines
- `packages/ui/src/page-examples.ts`: 2,004 lines
- `packages/ui/src/data-display.ts`: 1,070 lines
- `packages/ui/src/icon.ts`: 3,840 generated lines
- `packages/ui/src/ai-elements/*`: implemented as Foldkit files for prompt
  input, message, code block, task, sources, tool, confirmation, reasoning, and
  web preview

`packages/autopilot-ui` is the Autopilot domain UI package. It emits Foldkit
HTML and Tailwind classes via local helpers like `h.Class(value)`. Its
`tokens.ts` remains the compatibility import path for `darkTokens`,
`cssVars(tokens)`, and `nativeTheme(tokens)`, but those values now come from
the neutral `@openagentsinc/design-tokens` package.

The immediate opportunity is to move both packages from string-based utility
composition to typed StyleX modules while keeping the same Foldkit component
entry points.

### `apps/openagents.com`

`apps/openagents.com/apps/web/src/styles.css` is the main web stylesheet. It is
787 lines and uses Tailwind v4 through `@tailwindcss/vite`.

Important current responsibilities:

- `@import 'tailwindcss'`
- `@source "../../../workers/api/src"`
- `@theme` declarations for OpenAgents, Forum, public landing, fonts, and
  app-level CSS variables
- Berkeley Mono `@font-face` declarations
- `:root`, `html`, `body`, and app-root defaults
- scoped Forum and public landing theme modes through data attributes
- app utilities such as `no-scrollbar`, text scale utilities, and custom
  component classes

The Vite config currently uses:

```ts
plugins: [tailwindcss(), foldkit({ devToolsMcpPort: 9988 })]
```

StyleX's local `@stylexjs/unplugin` docs recommend putting `stylex.vite()`
before framework transforms. For this app, the likely coexistence order to
spike is `stylex.vite(...)`, `tailwindcss()`, then `foldkit(...)`, with
verification that the emitted CSS still lands at `assets/openagents.css` through
the existing Rollup `assetFileNames` rule.

### Autopilot desktop app

`apps/autopilot-desktop/src/ui/styles.css` is 3,503 lines and is currently built
with Tailwind CLI:

```sh
bunx @tailwindcss/cli -i src/ui/styles.css -o src/ui/styles.out.css
```

The generated `styles.out.css` is copied into the Electrobun view bundle as
`views/autopilot-desktop/styles.css`.

Important current responsibilities:

- scans desktop view source with `@source "./**/*.ts"`
- scans `packages/autopilot-ui/src/**/*.ts` so shared Tailwind classes are
  present
- declares `:root` variables that mirror `cssVars(darkTokens)`
- owns a large amount of bespoke webview CSS for the shell, panes, HUD, Verse,
  overlays, terminal surfaces, and interaction states

StyleX can work here, but desktop needs an explicit build decision. The local
StyleX repo includes a Bun unplugin entrypoint at
`@stylexjs/unplugin/bun`, and the source writes dev CSS to `dist/stylex.dev.css`
by default. The desktop app does not currently run a general-purpose CSS
bundler for the view stylesheet; it runs Tailwind CLI and copies a generated
file. A desktop StyleX slice should therefore either:

- introduce a small Bun/esbuild/Vite CSS build step for the webview, or
- keep Tailwind CLI during coexistence and add a StyleX CSS aggregation step
  that produces the copied `views/autopilot-desktop/styles.css` equivalent.

### Mobile app

`clients/mobile/AutopilotRemoteControl` is Expo SDK 55, React Native 0.83.6,
React 19, and `react-native-web` 0.21.0. The mobile repo guidance forbids EAS
cloud builds; native builds are local, and OTA goes through OpenAgents-owned
infrastructure.

The app currently uses plain React Native `StyleSheet.create(...)` across the
screens. The largest files include:

- `app/nodes.tsx`: 713 lines
- `app/session-detail.tsx`: 311 lines
- `app/artifact-viewer.tsx`: 277 lines
- `app/sessions.tsx`: 194 lines
- `app/spawn.tsx`: 179 lines
- `app/settings.tsx`: 141 lines

Mobile uses `CANONICAL_DARK` from
`@openagentsinc/autopilot-control-protocol`, not `@openagentsinc/autopilot-ui`
tokens directly. That protocol package already has `assertThemeParity(...)`
tests for the palette. Any StyleX migration should consolidate or bridge these
token sources before changing mobile style files.

### Three.js and Foldkit visuals

The existing repo guidance routes Three.js rendering through
`@openagentsinc/three-effect` and Foldkit bindings. StyleX should not replace
that system. It should style the DOM wrappers, controls, overlays, layout slots,
and canvas sizing. Three.js scene lifecycle, renderer resources, asset loading,
and Foldkit scene bindings should remain in `three-effect`.

## What the local StyleX repo confirms

The local StyleX reference is on commit `3d7e23f0` from 2026-06-21,
`release 0.19 (#1729)`. `@stylexjs/stylex` and `@stylexjs/unplugin` are both
at version `0.19.0`.

Confirmed capabilities from source and local docs:

- `stylex.create(...)` defines style maps that are compiled ahead of time.
- `stylex.props(...)` returns React-style `{ className, style,
  data-style-src }`.
- `stylex.attrs(...)` returns DOM-style `{ class, style, data-style-src }`,
  which is the critical Foldkit bridge point.
- `stylex.defineVars(...)` and `stylex.createTheme(...)` support runtime
  themeable CSS variables.
- `stylex.defineConsts(...)` supports shared static constants such as media
  queries and z-index values.
- `stylex.keyframes(...)`, `stylex.firstThatWorks(...)`, `@media`,
  `@supports`, `@container`, pseudo-classes, pseudo-elements, and relational
  selectors exist.
- `@stylexjs/unplugin` supports Vite, Rollup, Webpack, Rspack, esbuild, Bun,
  Rolldown, and Farm entrypoints.
- The Vite plugin auto-discovers installed packages that depend on
  `@stylexjs/stylex` and excludes them from dependency optimization so package
  source can be transformed. `externalPackages` can force inclusion.
- The unplugin aggregates CSS from transformed modules and appends it to an
  existing CSS asset when possible, otherwise it emits a fallback `stylex.css`.
- The ESLint plugin includes rules for valid styles, unused styles, shorthand
  discipline, extension enforcement for theme files, and conflicting props.

Important constraints:

- `stylex.create(...)`, `defineVars(...)`, `createTheme(...)`, and related APIs
  throw at runtime if the code is not compiled by `@stylexjs/babel-plugin`.
- Style objects must be statically analyzable.
- Imported values are not generally allowed in `stylex.create(...)` unless they
  are StyleX variables or constants from `.stylex.ts` / `.stylex.js` files.
- Theme variable files using `defineVars(...)` or `defineConsts(...)` must use
  `.stylex.ts` or `.stylex.js`, use named exports, and should not mix unrelated
  exports.
- Multi-value shorthands are discouraged or rejected. Migrations need to expand
  many Tailwind-like declarations into longhand CSS properties.
- `stylex.props(...)` / `stylex.attrs(...)` should own `class` / `style` for an
  element. Mixing extra classes manually on the same element should be avoided
  after migration.

## What `react-native-stylex` confirms

The local React Native reference is on commit `73a6c2d` from 2026-03-31. Its
package is `react-native-stylex` version `5.0.0`, with peer dependencies
`react >=19.0.0` and `react-native >=0.59.0`.

It is not the same package or API as Meta StyleX:

- Meta StyleX: compile-time atomic CSS for DOM.
- `react-native-stylex`: runtime React Native style hooks and HOCs around
  native `StyleSheet.create(...)`.

Confirmed mobile capabilities:

- `ThemeProvider` / `ThemeConsumer`
- `useTheme()`
- `makeUseStyles((theme) => ({ ... }))`
- `withStyles(useStyles)(Component)` for class components and ref forwarding
- TypeScript declaration merging for `DefaultTheme`
- `useColorTransition(...)` for animated theme color transitions
- `react-native-safe-area-context` integration through
  `StylexSaveAreaConsumer`, custom `SafeAreaProvider`, and `getSafeArea()`
- `Appearance` integration with `appearance(...)`, `darkAppearance(...)`,
  `lightAppearance(...)`, and `noPreferenceAppearance(...)`
- dimensions helpers `getWindowDimensions()` and `getScreenDimensions()`
- responsive helpers including `minWidth`, `maxWidth`, `minHeight`,
  `maxHeight`, aspect-ratio helpers, `createBreakpoints(...)`, and
  `createBreakpointsMatcher(...)`
- orientation helpers `orientation(...)`, `portraitOrientation(...)`, and
  `landscapeOrientation(...)`
- i18n helpers for RTL/LTR cases while still recommending React Native logical
  properties such as `start` / `end`

Implementation details that matter for OpenAgents:

- `makeUseStyles(...)` chooses a themed or non-themed hook based on function
  arity.
- The themed path caches styles in a `WeakMap` keyed by theme.
- Both themed and non-themed paths call `StyleSheet.create(...)`.
- The dependency registry tracks whether a style read dimensions, appearance,
  safe area, or similar runtime state, then recreates the style and forces
  subscribers to re-render when those dependencies change.
- The example app uses ordinary Metro and React Native Babel config, plus
  `ThemeProvider` and safe-area consumer wiring. No compiler plugin analogous
  to Meta StyleX is required.

This is a reasonable mobile migration target if OpenAgents wants StyleX-like
native ergonomics. It does not, by itself, give us shared web/native
`stylex.create(...)` files.

## Foldkit compatibility

StyleX should work with Foldkit through an adapter in `@openagentsinc/ui`.

The adapter shape is small. The P0 implementation lives at
`packages/ui/src/stylex-foldkit.ts` and adapts compiled StyleX output into
Foldkit's typed attribute list:

```ts
import * as stylex from "@stylexjs/stylex"
import type { StyleXStyles } from "@stylexjs/stylex"
import type { Attribute } from "foldkit/html"
import { html } from "foldkit/html"

export function stylexAttrs<Message>(
  ...styles: ReadonlyArray<StyleXStyles>
): ReadonlyArray<Attribute<Message>> {
  const h = html<Message>()
  const attrs = stylex.attrs(...styles)
  const result: Array<Attribute<Message>> = []

  if (attrs.class) result.push(h.Class(attrs.class))
  if (attrs.style) result.push(h.Attribute("style", attrs.style))
  if (attrs["data-style-src"]) {
    result.push(h.DataAttribute("style-src", attrs["data-style-src"]))
  }

  return result
}
```

The underlying integration point is confirmed by source and by #5952:
`stylex.attrs(...)` emits DOM attributes, and Foldkit components already accept
typed attributes.

Completed package changes from #5952 and #5953:

- `@stylexjs/stylex` is installed in `packages/ui` and
  `packages/autopilot-ui`.
- `@openagentsinc/ui/stylex-foldkit` exports the Foldkit adapter.
- `packages/ui/src/tokens.stylex.ts` exports StyleX variable aliases for the
  existing Foldkit CSS custom properties.
- Existing component function signatures remain stable; app code continues to
  import Foldkit components, not raw StyleX internals.

The main build requirement is that every consumer of the source-exported
packages must run the StyleX transform over those package files. Since
`@openagentsinc/ui` and `@openagentsinc/autopilot-ui` are workspace packages
with source exports, the web app and desktop app cannot treat them as opaque
prebuilt dependencies once they contain `stylex.create(...)` calls.

## `@openagentsinc/ui` and `@openagentsinc/autopilot-ui` compatibility

The shared UI library can migrate incrementally because the external API is
Foldkit HTML components, not Tailwind classes. A migrated component can keep the
same exported function and props while replacing:

```ts
h.div([h.Class("grid gap-2 border border-[#222] bg-[#010102] p-4")], children)
```

with:

```ts
h.div(stylexAttrs(styles.surface), children)
```

For component variants, StyleX maps well to existing named local constants:

- `tone` variants become conditional `stylex.attrs(base, tone === "danger" &&
  styles.danger)`.
- responsive Tailwind prefixes become nested StyleX media query values.
- CSS variable references become StyleX variables from `.stylex.ts` files.
- open escape hatches such as `attrs?: ReadonlyArray<Attribute<Message>>` can
  stay, but should be reserved for semantics, data attributes, ARIA, events,
  and rare layout overrides.

The token situation was fixed in #5953 for the Autopilot dark palette. The
previously parallel contracts were:

- `packages/autopilot-ui/src/tokens.ts` with DOM CSS vars and native-shaped
  colors
- `packages/autopilot-control-protocol/src/theme-parity.ts` with
  `CANONICAL_DARK`
- `apps/autopilot-desktop/src/ui/styles.css` root vars mirroring the UI tokens

The migration now has `@openagentsinc/design-tokens` as the neutral authority
for those values, with re-exported or derived:

- StyleX vars and themes for web/desktop
- CSS custom property aliases for coexistence and raw CSS
- React Native theme values for mobile
- control-protocol parity constants for protocol-level tests

## Benefits

### Typed style contracts

StyleX moves component styling from free-form strings into typed style objects.
That is valuable for `@openagentsinc/ui`, which is intended to become the
trusted component surface for generated and program-selected UI. Style keys,
variant boundaries, and accepted style prop surfaces become auditable.

### Less Tailwind string drift

The current package code repeats many long utility strings across Foldkit
components. StyleX would make repeated declarations share atomic CSS and make
variant composition explicit.

### Better cross-package ownership

Source-exported packages currently rely on app-level Tailwind scanning to see
all class strings. StyleX moves that requirement to a compiler transform that
can be configured for workspace package source and linted. That is a better fit
for `@openagentsinc/ui` as a package boundary.

### Token consolidation

P1 established a neutral token authority for `darkTokens`, `CANONICAL_DARK`,
desktop `:root` parity, and native-shaped theme output. Future StyleX migration
work should consume that authority instead of reintroducing local palette
copies.

### Smaller and more deterministic component CSS

StyleX emits atomic CSS and reuses declarations. For the shared component
library, that should reduce the cost of having many component families and
avoid the "scan every possible utility string" behavior of the current Tailwind
setup.

### Coexistence with current global CSS

The unplugin supports CSS layers and appending to existing CSS assets. That
means OpenAgents can run StyleX and Tailwind side by side while migrating, then
turn off Tailwind only after the last utility-class component is gone.

### Mobile modernization without NativeWind

`react-native-stylex` lets the mobile app keep native `StyleSheet.create(...)`
semantics while gaining theme hooks, responsive helpers, appearance, safe-area,
orientation, and i18n utilities. That aligns with the app's current native
style shape and avoids introducing Tailwind-style class strings into React
Native.

## Costs and risks

### Migration size

The styled surface is large. The visible line-count sample audited here is over
23,000 lines across the main CSS files, UI packages, Autopilot UI package, and
mobile screens. Not all of those lines are styles, and `icon.ts` is generated,
but it shows the scale. This should be a phased migration, not a branch that
rewrites every screen at once.

### Build integration

The web app needs Vite StyleX integration while preserving Foldkit and the
Cloudflare asset path `assets/openagents.css`.

The desktop app needs a new or adjusted CSS generation path. Its current
Tailwind CLI build is simple and explicit. StyleX compilation must produce a
file Electrobun copies into the packaged webview.

### Static analysis constraints

StyleX will reject patterns that are common in Tailwind migration code:

- arbitrary imported constants inside `stylex.create(...)`
- object spreads from local helpers
- multi-value shorthand declarations
- runtime function calls while defining style objects
- mixing manual classes and StyleX styles on one element

This is good long-term pressure, but it makes the first migrations slower.

### CSS that should stay CSS

Some CSS is not worth migrating into component StyleX:

- `@font-face`
- `html`, `body`, and app-root defaults
- base resets
- app-level theme attribute plumbing
- third-party browser quirks
- generated CSS output from tools
- raw template strings or server-rendered HTML that the StyleX compiler cannot
  see yet

### React Native API divergence

`react-native-stylex` is useful, but it is not Meta StyleX. Shared tokens are
realistic. Shared web/native style declarations are not a safe assumption
without building our own facade or accepting a lowest-common-denominator style
authoring layer.

### Developer workflow

The repo would need lint rules and examples early. Otherwise developers will
keep writing Tailwind classes, raw CSS, and StyleX side by side without a clear
boundary.

## Proposed migration plan

### Phase 0: vertical-slice spike

Goal: prove StyleX, Foldkit, Vite, and desktop CSS/view generation before broad
rewrites. Mobile token bridging was part of the original audit recommendation
but is deferred from the first implementation sequence.

Tasks:

- Add `@stylexjs/stylex`, `@stylexjs/unplugin`, and
  `@stylexjs/eslint-plugin` in a branch.
- Add `packages/ui/src/stylex-foldkit.ts` with the Foldkit adapter.
- Add `packages/ui/src/tokens.stylex.ts` with a small set of canonical dark
  vars.
- Migrate one low-risk `@openagentsinc/ui` primitive and one
  `@openagentsinc/autopilot-ui` card.
- Configure `apps/openagents.com/apps/web/vite.config.ts` with
  `stylex.vite(...)` and confirm output still lands in `assets/openagents.css`.
- Add one desktop view/component using the same migrated package component and
  prove packaged CSS generation.
- Defer the mobile token bridge to a separate native migration track.

Exit criteria:

- web app builds and includes generated StyleX CSS
- desktop webview prebuild compiles `stylex.create(...)` out of the browser
  bundle and includes generated StyleX CSS in the copied stylesheet
- existing Tailwind components still work during coexistence
- mobile remains untouched until the separate native track starts

### Phase 1: token consolidation

Goal: one dark token authority with derived outputs.

Status: completed for the Autopilot dark palette in #5953.

Tasks:

- `@openagentsinc/design-tokens` owns the canonical Autopilot dark palette.
- `packages/ui/src/tokens.stylex.ts` derives StyleX aliases for the existing
  CSS custom properties.
- `@openagentsinc/autopilot-ui/tokens` keeps the compatibility exports.
- `@openagentsinc/autopilot-control-protocol/CANONICAL_DARK` derives from the
  shared source.
- `assertThemeParity(...)`, package token tests, desktop theme tests, and
  StyleX web/desktop build checks guard the migration.

### Phase 2: migrate shared UI primitives

Goal: make `@openagentsinc/ui` the StyleX-first component library.

Status: started in #5954 with the first shared, form, and AI Elements families.
Tailwind compatibility exports remain for not-yet-migrated families.

Recommended order:

1. Continue small form primitives, checkboxes, radio groups, toggles, and
   comboboxes.
2. Finish `feedback.ts`, badges, panels, and empty states.
3. Continue `ai-elements/*` beyond Prompt Input.
4. Migrate `workroom.ts` and layout shells.
5. Migrate larger public, business, and page-example composites.

Keep old CSS/Tailwind available until each family is fully moved.

### Phase 3: migrate Autopilot domain UI

Goal: convert `@openagentsinc/autopilot-ui` to consume shared tokens and
StyleX helpers.

Targets:

- node status
- cloud quota
- earnings
- decisions and approvals
- assignments
- artifacts and receipts
- session views
- public activity components

This package should stop owning independent styling primitives and should lean
on `@openagentsinc/ui` for base surfaces where possible.

### Phase 4: migrate web app-local CSS

Goal: reduce `apps/openagents.com/apps/web/src/styles.css` to global concerns.

Move component classes and app-local view classes into StyleX modules near the
Foldkit views that use them. Leave root styles, font faces, base variables, and
theme selectors in CSS.

### Phase 5: migrate desktop CSS

Goal: reduce `apps/autopilot-desktop/src/ui/styles.css` to global webview,
asset, and platform concerns.

Recommended order:

1. shared package components already rendered in desktop
2. pane and shell primitives
3. HUD controls that map cleanly to components
4. session/detail/filter controls
5. Verse overlays and high-touch bespoke states

The raw Three.js scene system should stay in `three-effect`; only the DOM
surfaces around it should move.

### Phase 6: mobile migration track

Goal: move mobile from repeated `StyleSheet.create(...)` objects to a themed
native style system.

Recommended path:

- Add `react-native-stylex` only after an Expo-compatible install check using
  `npx expo install` or an explicit version decision.
- Add a mobile `ThemeProvider` wired to the shared native token output.
- Migrate one small screen to `makeUseStyles(...)`.
- Add safe-area and dimensions helpers only where they replace existing local
  code.
- Preserve local build and OTA workflows. Do not introduce EAS cloud.
- Keep React Native style arrays for animated values where needed.

## Verification checklist

For a StyleX vertical slice:

- `bun run typecheck:ui`
- `bun run test:ui`
- `bun run typecheck:autopilot-ui`
- `bun run test:autopilot-ui`
- `bun run --cwd apps/openagents.com/apps/web build`
- `bun run check:deploy`
- desktop CSS/build command updated and run
- desktop packaged asset check confirms the StyleX CSS is copied into the
  Electrobun view
- mobile TypeScript check or Expo export command after any later mobile package
  change

For a docs-only audit change, `git diff --check` is sufficient.

## Recommendation

Proceed with StyleX, but do it as an incremental design-system migration:

- yes to StyleX for `@openagentsinc/ui`
- yes to StyleX for `@openagentsinc/autopilot-ui`
- yes to a Foldkit adapter based on `stylex.attrs(...)`
- yes to StyleX token consolidation
- yes to a React Native migration track using `react-native-stylex` if the
  Expo install check passes
- no to a single big-bang rewrite
- no to deleting global CSS until it only contains true global concerns
- no to claiming web and native can share identical StyleX style files without
  a separate facade design

The strongest first milestone has now landed for DOM: one shared UI primitive,
one Autopilot domain component, one web render, one desktop render, and the
shared token authority. The remaining work is mainly disciplined migration,
with mobile intentionally deferred to its separate native track.
