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

## Final source proof, before coordinated publication

The canonical document is now 10,266 UTF-8 bytes. Its SHA-256 is
`1b54506a329052b1c1c2b82fdf15e623527212ecfdb825824d8255f1267e7f09`.
Final muted desktop and DPR-2 touch browser cases copied every byte and the
96-byte join command, and scrolled correctly. Six source checks plus five
native Wasm cases pass. The updated author-reviewed desktop/phone golden
walk still starts and draws with no errors. The qualified compiler was
installed with the existing provenance verifier/cache; no provenance was
forged and no unchanged compiler/bootstrap suite was repeated.

The public `confidential-round.json` is an exact 4,480-byte copy of root's
`docs/verification/data/oct30-confidential/round.json` from `da0bbfa057`,
SHA-256 `9bd363515e84f9776bef13fa5381ed512030e51efd6e051f01d4d93b37b55fa3`.
The guide labels it as a dated Intel TDX CPU capture and links the original
signed records and reproduction report at that immutable source commit.
Presentation checks cover byte parity, the CPU hardware label, absent GPU
claim and presence of the checked receipt. They do not substitute for the
operator's certificate, measurement, key-binding and receipt verification.

Hosted availability is gated on fresh fleet/API rollout. No production web
release was started while the GPU hosts were being recovered from capacity
stockouts; the guide makes no permanent-availability or GPU-attestation claim.

## Final production closeout

Production release `a6b0f91c74`, stamp `20261030T185415Z`, contains the frozen
guide candidate `83985bbc7f1684dc1e05f8e9c4c38767cbbcca26`. Its normal release
passed production HTTP smoke and actual live homepage starts. No extra release
or unchanged compiler gate was run for this closeout.

The canonical source and production `/docs/connect-a-pylon/index.md` are byte
identical: **10,374 bytes**, including the 16 KiB API request bound. SHA-256:
`0aa18131dc848f1234e64d31d0aea407b62b680012340f38f76c0479746bdc4e`. Production names
`connect-pylon.a78c411cdbdf.wasm` and the shared Grid shader.

A final muted, headless browser run exercised ordinary input against the real
production GPU page at 1440×900 desktop and 390×844 at DPR 2 phone. Both copied
all 10,374 Markdown bytes, copied the exact 96-byte join command, scrolled
160 CSS pixels, preserved the command clipboard through scrolling, and fetched
the byte-identical Markdown with its `text/markdown` MIME. Neither recorded a
runtime exception. The read-only observation receipt is
[data/connect-pylon-2026-10-30/production-browser.json](data/connect-pylon-2026-10-30/production-browser.json).

The browser harness now waits for nonzero layout and actual submitted GPU work
completion. Module `ready` precedes first layout; the host's `sceneReady` flag
is specifically for 3D draw lists and does not describe this 2D GPU guide.
This verification correction changes no product state or rendering.

The documented production helper also returned real authenticated Elephant and
Mammoth results independently during this task. Managed, keyless recovery for
the existing shared GPU host is recorded separately in
`psionic/docs/serving/spot-recovery.md` and
`docs/verification/data/pylon-spot-recovery-2026-10-30/managed-job.json`.
