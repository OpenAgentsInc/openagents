# Effect Native Storybook workbench receipt

- Class: receipt
- Date: 2026-07-15
- Status: deployed and live-proven; issue closed
- Dispatch: no; use [#8856](https://github.com/OpenAgentsInc/openagents/issues/8856)
- Owner: OpenAgents UI architecture

## Result

`/components/core` is now a rendered component workbench rather than an
inventory card. It mounts the complete Effect Native Gallery through the owned
DOM renderer: 79 component groups and 108 concrete stories covering layout,
controls, inputs, collections, overlays, data display, operational views,
feedback, loading states, marketing blocks, host surfaces, and Khala frames.

The same workbench is available from the DOM and React Native renderer family
routes so renderer consumers can inspect one typed component contract. The
training route presents the operational subset, while `/components/tokens`
renders real color, type, spacing, and radius specimens. Story controls are
shown beside each preview and the typed view is retained only as a collapsed
diagnostic inspector.

Each story owns a scoped renderer lifecycle. Supplying the story viewport also
prevents each preview from registering a redundant window resize listener. A
contained rendering canvas keeps overlay and fixed-position examples inside
their specimen boundary.

## Verification

- Start TypeScript: pass.
- Vite Plus formatting and changed-file policy check: pass with only existing
  renderer warnings.
- Start suite: 46 files and 197 tests pass, including exact 79-component and
  108-story SSR inventory assertions and representative variant assertions.
- Production Start build: pass; `/components/core` emits 735,577 bytes of SSR
  HTML in the local production preview plus the concrete Start JS asset.
- Local production preview: route and concrete asset returned HTTP 200, with
  exact `data-storybook-component` and `data-storybook-story` counts of 79 and
  108.
- Production Cloud Run revision: `openagents-monolith-00143-jwc`, serving 100%
  of traffic.
- Live route: `https://openagents.com/components/core` returned HTTP 200 and
  735,577 bytes of SSR HTML.
- Live inventory markers: exactly 79 `data-storybook-component` sections and
  108 `data-storybook-story` previews, including the primary Button variant.
- Concrete live asset: `/assets/index-BoHjCJZS.js` returned HTTP 200.
- Interactive browser automation is unavailable because the installed browser
  client cannot initialize its process shim in the current Codex runtime. The
  type, test, build, SSR, production asset, and live HTTP checks are the release
  gates for this bounded workbench.

## Final disposition

This receipt is evidence, not dispatch authority. The sanctioned Cloud Run
deployment, live route/asset proof, and closure are also recorded on #8856.
