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
- Upstream commit: `1aa6e364d9fc67cd22d493db9bf223bb0080bb0e` (catalog `v19`, 48 components)
- Vendored: 2026-07-09 (bumped from `e32b97e`, catalog `v5`)
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

### Monorepo build deltas vs. verbatim upstream (v19 bump)

The four packages are copied verbatim EXCEPT for the minimal edits the monorepo
strict tsconfig forces (upstream compiles under a laxer config):

- `effect-native-core/src/effect.ts` — monorepo-only "effect version bridge"
  re-export (`@effect-native/core/effect`) so consumers unify Effect versions.
  Not an upstream file.
- `effect-native-core/src/index.ts` — removed 5 unused token imports
  (`colorTokens`/`dimensionTokens`/`radiusTokens`/`spacingTokens`/`typeScaleTokens`,
  still re-exported) and the dead `formatUnknown` helper to satisfy
  `noUnusedLocals`. The EN-1 local `exactStruct` workaround (effect-native#44)
  is gone — upstream's annotate-based fix is now the vendored code.

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
