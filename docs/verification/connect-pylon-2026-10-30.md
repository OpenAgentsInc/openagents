# Connect a Pylon quickstart — 2026-10-30

The canonical source is `docs/connect-a-pylon.md`. Run
`bun coax/web/connect-pylon-page.ts` after editing it; `--check` detects drift.
It generates the public Markdown twin, GPU-hidden accessible HTML, Coax
content and a shader artifact derived from the homepage shader. No runtime
Markdown parser, DOM controls or new Nostr implementation are involved.

`connect-pylon.coax` reuses the homepage Grid camera/projection, font atlas,
buttons and the Metareal HUD frame. An empty world view leaves project plots
unlit and avoids allocating the homepage's network state. The C face exposes
scalar calls and read-only test observations. The document content hash is
part of the layout fingerprint so hot reload cannot retain stale line heights.

## Checks

- 74 Coax checks passed during implementation, including six authored guide
  checks and the empty-Grid regression. Imported checks were not repeatedly
  rerun for font styling or remote commits.
- Five source/generator checks: exact Markdown/Coax/HTTP-source parity,
  bounded full-copy budget, generated-artifact drift, no visible DOM,
  executable toy-request JSON and accurate join prerequisites.
- Four native Wasm viewport cases: portrait, desktop, landscape and DPR 2.
  Full Markdown copy and exact 96-byte join command, finite/bounded geometry,
  readable mixed-case atlas text, control bounds, CSS scrolling, drag/cancel
  suppression and Home navigation all passed. The page state is below 64 KiB.
- Muted headless Chrome: desktop 1440×900 and real touch input at
  390×844/DPR 2. Both copied all 5,501 canonical Markdown bytes via the normal
  host clipboard action, copied the exact join command, and scrolled by
  160 CSS pixels without copying/navigating. The HTTP Markdown twin matched
  byte for byte and used `text/markdown`; the accessible main was clipped to
  one pixel. Test observations are read-only; content/state was not injected.
- `coax/web/check-visuals.sh apps/openagents-web` passed.

Commands:

```sh
bun test coax/web/tests/connect_pylon_page.test.ts
bun test coax/web/tests/connect_pylon_wasm.test.ts
bun coax/web/tests/connect_pylon_browser.ts http://127.0.0.1:18845 /tmp/connect-pylon
```

The latter tests an already-built running site. Build with
`coax/web/wasm.sh apps/openagents-web/connect-pylon.coax apps/openagents-web/connect-pylon-face.c apps/openagents-web/build/connect-pylon.wasm`.
The homepage and legacy board now navigate to the canonical human page;
their existing route assertions were updated accordingly.

## Scope and publication

The audience join script remains a disposable-identity `psionic-toy` demo.
Larger hosted models and authenticated production API deployment are a
separate coordinated slice. This guide's core does not claim those models
were deployed, authorize production signing keys, introduce payment,
or turn signed provider advertisements into hardware/model attestation.
Root owns deployment, shared build/smoke/golden registration and the final
executable authenticated-agent walkthrough. There is no new book chapter:
this operator quickstart complements the existing Pylon/mesh chapters.

## Website integration

The build now refuses stale generated guide artifacts, compiles its Wasm and
waits for it. HTTP smoke checks the human route, named host/Wasm/shader,
exact canonical Markdown bytes, llms discovery and sitemap inclusion.
Desktop and phone have registered goldens; the author inspected both.
The normal golden walk started/drew both with no errors (6.1% and 12.3% lit).
The typography uses the shared atlas at a normal reading size over a quiet
Grid. Current API instructions include the native `mesh-client.sh` wrapper,
prerequisites, exact hosted aliases, session/NIP-98 boundaries, replay IDs,
provider admission and refusal meanings; the endpoint author reviewed them.
Shared Coax page-view analytics passed a real Wasm host-effect check.
Actual packaging passed the publication-boundary regression: exact Markdown,
content-named module/shader and only the explicitly allowlisted public proof
JSON; a sibling `private.json` fixture stayed unpublished.
Publication remains coordinated with the real fleet/API readiness.
