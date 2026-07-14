# Vendored `@effect-native/render-rn` (EN-3, issue #8568)

`@effect-native/render-rn` is the React Native renderer for the Effect Native
component set — renderer **adapter #1** for `clients/khala-mobile`
(MASTER_ROADMAP §EN unlock #3). It is a vendored snapshot of the public
[`OpenAgentsInc/effect-native`](https://github.com/OpenAgentsInc/effect-native)
framework, sitting next to the EN-1 web packages
(`effect-native-core`, `effect-native-tokens`, `effect-native-render-dom`).

## Why vendored (not a git/npm dependency)

effect-native is **not published to npm** and ships **unbuilt TypeScript** whose
internal packages cross-reference each other with `workspace:*`. A `github:`
install pulls the repo root (no scoped sub-packages); a `file:` reference to the
sibling `~/work/effect-native` checkout cannot resolve the `workspace:*`
cross-refs from openagents and does not exist in CI/Metro. The only clean,
CI-safe, Metro-safe path today is to land the source as openagents workspace
members. The package **names match upstream** (`@effect-native/render-rn`, …),
so swapping to the real published dependency later is a package.json-only change.

## Provenance

- Upstream repo: `OpenAgentsInc/effect-native`
- Upstream commit: `ec04d1a066d6f3ed0c67735ba451cfc90a343aa8` (catalog `v39`)
- Vendored: 2026-07-14 (bumped from `6db0a67b`, catalog `v30`; earlier
  `f8251374` `v30`, `e0c57cb` `v29`, `eb9685b` `v25`, `1aa6e364` `v19`,
  `e32b97e` `v5`)
- Files copied verbatim: `packages/render-rn/src/**`

### Single source of truth + anti-staleness guard (2026-07-09)

The pinned commit + catalog version now live in ONE manifest,
[`../effect-native-vendor.json`](../effect-native-vendor.json), covering all
four vendored packages (`core`, `tokens`, `render-dom`, `render-rn`). Each
vendored `package.json` also records the same commit under `effectNativeVendor`.

- `bun run --cwd apps/openagents.com test:effect-native-vendor-guard` — HARD
  test (in `check:deploy`): a partial bump (one package at a stale commit) or a
  core `CatalogVersion` that disagrees with the manifest is RED.
- `bun run --cwd apps/openagents.com check:effect-native-vendor` — freshness
  WARNING (never a failure): compares the manifest commit against the sibling
  `~/work/effect-native` `origin/main` tip and prints how many commits behind.

To re-vendor: fetch upstream, re-copy each package's `src/**`, then bump the
commit + catalogVersion in `effect-native-vendor.json` AND every vendored
`package.json` `effectNativeVendor.commit`.

### Monorepo build deltas vs. verbatim upstream (v25 bump)

The four packages are copied verbatim EXCEPT for the minimal edits the monorepo
strict tsconfig forces (upstream compiles under a laxer config):

Consumers (e.g. `apps/start`) import the vendored `./src/index.ts` directly and
typecheck it under the strict base tsconfig (`noUnusedLocals: true`), so any
unused import/local in the vendored source is a consumer RED. The deltas:

- `effect-native-core/src/effect.ts` — monorepo-only "effect version bridge"
  re-export (`@effect-native/core/effect`) so consumers unify Effect versions.
  Not an upstream file.
- `effect-native-core/src/index.ts` — removed the dead `formatUnknown` helper and
  the 5 unused value imports
  `colorTokens`/`dimensionTokens`/`radiusTokens`/`spacingTokens`/`typeScaleTokens`
  (upstream re-exports these directly via `export { … } from "@effect-native/tokens"`,
  so the local import bindings are unused).
- `effect-native-render-dom/src/index.ts` — removed 13 unused marketing-component
  value imports (`Section`/`Hero`/`AnnouncementBadge`/`CtaSection`/`Footer`/`NavBar`/
  `Accordion`/`PricingColumn`/`PricingTable`/`LogoRow`/`StatsBand`/`Glow`/`MockupFrame`).
  The DOM renderer consumes only the `*View` types, not the factory functions.

The upstream `@effect-native/render-canvas` package (Three.js scene-graph
renderer) is NOT vendored: no monorepo consumer imports it yet. See the manifest
`upstreamPackagesNotVendored`.

## Coherence bump of the shared core (this change)

The EN-1 web packages were vendored from an earlier upstream snapshot
(`0.0.0-openagents.8567`, catalog `v0`). `render-rn` at `e32b97e` requires the
`v5` catalog (Modal/Sheet/SectionList/typed navigation/responsive variants), so
this change **bumps `@effect-native/core` (`effect-native-core/src/index.ts`) to
the same `e32b97e` snapshot** to keep one coherent core behind both renderers
(the roadmap's "one source, two renderers" rule). `@effect-native/tokens` is
byte-identical to upstream and unchanged. The DOM renderer package
(`effect-native-render-dom`) is left at its landed snapshot and re-validated
against the new core (its stage1 surface + test must stay green).

Note: EN-1's local `exactStruct` workaround (a crude `Schema.Struct(fields)`
that dropped excess-key rejection, tracking upstream **effect-native#44**) is
**superseded** by the proper upstream fix now present in `e32b97e` — the
annotate-based `exactStruct` (`onExcessProperty: "error"`) that keeps exact
rejection while accepting all declared keys. The stage1 mount test verifies it
works in the app runtime.

## Sync rule

Do not hand-edit `src/**` except for a minimal, documented effect-version delta.
Real component/renderer work belongs upstream in `OpenAgentsInc/effect-native`
first, then re-vendored by bumping the commit above and re-copying `src/**`.

## v30 hunk-level re-vendor (2026-07-10, GL-1 #8647)

The `e0c57cb -> f825137` upstream delta (Scope-bound RN host-driver registry
`ReactNativeHostDriver`/`makeReactNativeHostRuntime`, render-rn-internal
`@expo/ui` SwiftUI Liquid Glass lowering for the glass set, and the v30
glass-chrome icons `Menu`/`Compose`/`Mic`/`Sparkles`) was applied as a
HUNK-LEVEL patch onto the vendored files instead of a wholesale re-copy,
because the vendored copies now carry deliberate monorepo divergences that a
re-copy would destroy:

- `effect-native-core/src/index.ts` — 12 extra app `IconName`s
  (`Agent`/`ChatCompose`/`Chats`/`Code`/`Compare`/`Folder`/`Home`/
  `NotificationBell`/`Plane`/`Settings`/`Terminal`/`Tools`) added by the
  desktop/mobile lanes (`8ed6d166fd`, `d1abe0e81e`). NOT upstream; upstream
  demand should be filed through GAPS before these are re-vendored verbatim.
- `effect-native-render-rn/src/index.ts` — Scope-owned intent effect runtime
  (`ReactNativeRenderRuntimeOptions.runEffect`, FiberSet-backed reporter
  wiring) replacing upstream's `Effect.runPromise` in `runReportedIntent`,
  plus RN glyphs for the 12 app icons and their SF-Symbol entries in the new
  `sfSymbolForIcon` map. CUT-01 #8681 additionally owns the semantic glass
  `Composer` lowering: iOS 26+ uses renderer-internal `@expo/ui` observable
  `TextField` state plus an explicit typed send button, while Android/older iOS
  keep the accessible RN `TextInput`/material fallback with identical intents.
- `effect-native-render-dom/src/index.ts` — OpenAI icon catalog import, glass
  CSS tuning, and the atomic-style registry refactor from the desktop lane.
- `@expo/ui` is an optional peer of `@effect-native/render-rn` and a real
  dependency of `apps/openagents-mobile` (the installation vehicle for the
  native module). App code never imports it — enforced by
  `apps/openagents-mobile/tests/component-sharing.test.ts`.

These divergences are now the "known divergence" base for the next re-vendor:
either upstream them first (preferred, per the sync rule) or re-apply the next
upstream delta hunk-wise as done here.

## Post-v30 upstream lowering fix

- `effect-native` `796f4b9e935a5425934230dad3914b7c6fd90587`
  (effect-native#73) was applied hunk-wise to `render-rn`: transcript messages
  now sit inside a full-width role-aligned row and carry `minWidth: 0` plus
  `flexShrink: 1` under their 82% bound. The catalog and the shared v30 vendor
  manifest remain unchanged because this is a lowering-only fix.

## v39 pin bump — harmonization catalog convergence (2026-07-14, openagents#8811 step 1)

Mechanical pin-bump-only step (no recipe sweep, no new-component adoption).
Upstream `6db0a67b -> 9d81139` (9 commits) landed the Apps SDK UI
harmonization catalog through `v31`-`v39`: tier-1 primitive token ramps and a
tone x variant x state color matrix (`#74`/`#75`), control-lattice
sub-tokens (`#76`), render-dom's component-token-tier + `data-*` variant
lowering mechanism (`#77`), Button's full tone/variant/size matrix (`#78`),
matrix axes + a new Alert component on Badge/Chip/TextField/Select (`#79`),
Avatar/AvatarGroup (`#80`), a 101-name icon expansion (`#85`, absorbing the
monorepo's desktop-shell icon set upstream "for parity"), EmptyMessage
(`#82`), CopyButton (`#84`), SegmentedControl, and Spinner/LoadingDots/
ShimmerText (`#83`). All of it is documented upstream as backward-compatible:
legacy trees resolve through an `isLegacy` flag (Badge/Chip/TextField/Select)
or a pre-`#78` variant normalizer (Button), so old serialized/JS-authored
trees keep rendering identically.

Applied as a `git merge-file` three-way merge (base = old pin, ours =
vendored copy with its known divergences, theirs = new upstream tip) per
package, then resolved conflicts by hand:

- **core / tokens**: took upstream's side wherever it touched the catalog
  version ladder or the icon-name closed set (a strict superset — v33's
  icon expansion explicitly absorbed the monorepo's desktop-shell names and
  three new roles/a11y fields the monorepo did not have). `tokens` merged
  with zero conflicts.
- **render-dom**: kept the monorepo's OWN pixel-sized icon path
  (`iconSizePixels` + `iconSvg(name, sizePx)` reading the sibling `./icons.ts`
  asset file added earlier the same day by openagents#8813 Lane A) instead of
  adopting upstream's new inline `iconRegistry` + CSS-var (`--en-icon-size-*`)
  sizing mechanism for the ORIGINAL 31 icon names — upstream's glyphs for
  those names are a different, simpler stroke style, and swapping them would
  have been a real visual regression for icons already live in
  `openagents-desktop`, not a mechanical pin bump. `icons.ts` was extended
  with the ~70 brand-new v33 icon names (verbatim upstream SVG bodies,
  re-wrapped in the file's existing template) so the type still compiles
  against the expanded 101-name `IconName` union; none of those 70 names had
  any pre-bump consumer, so there is no regression surface there. All ~19
  icon-drawing call sites across render-dom (Button loading spinner, Avatar,
  EmptyMessage, Combobox/Select/Tabs/Sidebar items, Graph markers, the new
  Select dropdown-indicator glyph, CopyButton, IconButton, Icon) were
  reconciled onto that one signature. The new Button/Badge/Chip/TextField/
  Select/Alert matrix CSS, SegmentedControl, Avatar, EmptyMessage, CopyButton,
  and Spinner/LoadingDots/ShimmerText renderers were taken from upstream
  in full — they are new, additive, `isLegacy`-gated code paths with zero
  current desktop call sites, so vendoring them carries no rendering risk.
- **render-rn**: took upstream's side for the icon glyph tables (its own
  honest-text-glyph/SF-Symbol fallback tables were already a strict superset,
  byte-identical on every overlapping name) and for the Button tone/variant/
  size matrix rewrite (mirrors render-dom's `#78` adoption; `resolveButtonAppearance`
  normalizes the pre-`#78` `"primary"`/`"secondary"`/`"ghost"` tokens onto their
  exact matrix equivalents, so old and new trees render identically).

No `shell.ts`/`settings.ts`/etc. recipe file changed, no `app.css` change, no
`design-conformance.test.ts` rule change, and no new component was adopted
into any desktop or mobile view — those are separately scoped follow-up work
(openagents#8811 steps 2+).
