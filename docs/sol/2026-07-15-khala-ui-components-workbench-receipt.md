# Khala UI component workbench receipt

- Class: receipt
- Date: 2026-07-15
- Status: deployed and live-proven; issue closed
- Dispatch: no; use [#8855](https://github.com/OpenAgentsInc/openagents/issues/8855)
- Owner: OpenAgents UI architecture
- Effect Native source: `f7f7fe6ed8e4245126d7149b3f3060d3d8d8c0e9` (`effect-native/v43`)

## Result

`/components/khala` is the public inspection surface for the complete owned
Khala UI visual-effects catalog. It renders all 30 non-audio capabilities from
the pinned parity ledger instead of presenting catalog metadata alone:

- 4 foundation paint/theme capabilities;
- 6 motion capabilities, including all 31 easing curves and a replay driver;
- 1 choreography planner;
- 11 frame capabilities covering all 12 owned motifs and decorative clipping;
- 2 grapheme-aware text effects;
- 2 container-local illumination effects; and
- 4 deterministic Canvas background families.

The route stays inside the existing TanStack Start `/components/$family`
boundary. Effect Native remains the component, theme, effect, and renderer
authority; React remains the host. This is therefore directly reusable by the
React-based Electron renderer without changing Electron main, preload, IPC,
navigation, or security policy.

Audio is explicitly excluded. The page emits complete semantic SSR output,
starts visual drivers only after mount, owns effect cleanup through a scoped
React lifecycle bridge, suspends continuous Canvas work under reduced-motion
and hidden/offscreen conditions, and keeps decorative SVG nodes inert.

## Vendor boundary

The coherent source pin now includes seven private workspace packages:
`core`, `tokens`, `render-dom`, `render-rn`, `khala-ui`, `render-canvas`, and
`gallery`. The anti-staleness guard requires one commit and catalog version
across the set and proves the 30-row parity ledger, 12 motifs, four backgrounds,
React Native polygon lowering, and absence of Arwes/audio runtime dependencies.

## Verification

- Start TypeScript: pass.
- `khala-ui`, `render-canvas`, and `gallery` strict TypeScript: pass.
- Focused SSR and vendor-guard suites: 11/11 pass.
- Production Start build: pass; `/components/khala` emits 73,226 bytes of SSR
  HTML in the local production preview plus the concrete Start JS asset.
- Production Cloud Run revision: `openagents-monolith-00141-b6m`, serving 100%
  of traffic.
- Live route: `https://openagents.com/components/khala` returned HTTP 200.
- Concrete live asset: `/assets/index-C6KMvN47.js` returned HTTP 200.
- SSR markers: `data-khala-workbench="complete"`,
  `data-khala-capability-count="30"`, and `data-khala-audio="excluded"`.
- Architecture scan: this lane adds no raw JSON parse outside the named
  Schema-backed Gallery boundary and no new `Effect.runPromise`; the full
  repository scan remains red on unrelated pre-existing Worker and renderer
  bridge debt recorded by the guard output.
- Interactive browser automation was unavailable because the installed browser
  client could not initialize its process shim in the current Codex runtime.
  The SSR, type, focused test, build, and live HTTP/asset proofs remain the
  release gates for this bounded route.

## Final disposition

This receipt is evidence, not dispatch authority. The sanctioned Cloud Run
deployment, live route/asset smoke, and closure are also recorded on #8855.
