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
- Upstream commit: `e32b97e0f95b99a6f0547ce74d71056225ead10e`
- Vendored: 2026-07-08
- Files copied verbatim: `packages/render-rn/src/**`

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
