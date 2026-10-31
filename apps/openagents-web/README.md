# openagents.com on Coaxial

The website, drawn by Coax on WebGPU. The framework is specified in
[coax/web.md](../../coax/web.md).

The homepage at `/` is `newland.coax`: six featured projects, their signed task
claims and real Pylons on an interactive Grid. `/new-land` is an alias. Shared
Coax verifies public Nostr events and beacon digests; the page watches new
announcements and refreshes complete project histories without reloading.
Disconnected or incomplete reads preserve the previous verified view. Expired
capacity becomes unknown. Git trees are a real deployment snapshot, and worker
labels are project-authored claims. No simulated people are drawn.

`newland-network.coax` owns the live coordinator. `landing-browser.coax` and
`landing-fold.coax` compile separately into the same Wasm module. The C faces
bridge buffers and effects; the browser host supplies generic sockets, clock,
input and WebGPU. The [homepage notes](new-land/README.md) name the focused checks.
`font.coax` is Filament ([coax/font.md](../../coax/font.md)); HTML/Markdown twins
carry accessible text. Visible UI is drawn in Coax.

`/verse` and `/verse/galaxy` are the Verse playtest's way in, one app
(`verse.coax`, `verse-face.c`, `verse.wgsl`, `data-page` 0 and 1): the
landing (CREATE ANYTHING, ENTER THE VERSE, the email box with GO, and ENTER
to play as a guest; an email goes to `POST /api/playtest/join`) and the
galaxy, where only the Haunted House opens. Their panels are the Haunted
House's wood-and-bronze plates, not the HUD bar.

```sh
coax/bin/coax test apps/openagents-web/newland.coax --steps 400000000
coax/web/tool.sh suites                              # every suite here and in verse/, each at its header's budget
coax/web/serve.sh apps/openagents-web                # http://127.0.0.1:8800/, hot reload
apps/openagents-web/build.sh                         # release build: all page modules
```

A suite's header names how it runs: `coax test FILE --steps N` (and
`--library`) in its first lines. A suite whose checks need more than the
default 20,000,000 steps each says so there; nothing raises the budget for
every suite. `coax/web/tool.sh suites` runs each that way, and a release runs
the ones it changed after it is live (releases/<time>/SUITES, a report).

`coax serve` rebuilds on save: a `.coax` or `.c` change swaps the engine
into the running page at the next frame and keeps its state (the animation
carries on); a `.wgsl` change swaps the shaders; an `.html`, `.js` or `.md`
change reloads the page. Build errors show in the page's corner with the
`.coax` location; the page keeps running the last good build.

`http://127.0.0.1:8800/?probe` reads one frame back from the GPU and prints
its middle to the console, for checking a render in a headless browser.
`&at=8` runs the clock 8 seconds first, and `&do=` runs input before the
frame (`m16;kab;t0.5` arms the terminal, types two keys, waits half a
second; see `host.js`).

A new `.coax` file is watched from the next `coax serve` start; until then,
save `newland.coax` to rebuild.

Needs upstream LLVM: `brew install llvm lld wasi-libc wasi-runtimes`.

## NIP implementation guides

`/nip` indexes 22 NIPs with Coax library support, partial foundations or a
production-relay declaration. Each `/nip/NN` page names its exact scope and
remaining work. `/nip/01` keeps the five-case event-verification pipeline;
the other pages share `nip-guide.coax` and its separately compiled
`nip-guide-observe.coax`, which calls the existing libraries on public fixtures.
Relay-only examples inspect frozen NIP-11 metadata; they do not test relay
conformance. No guide takes keys or publishes events.

Edit `nip/catalog.json`, then run `python3 coax/web/nip-pages.py` to refresh
the Coax inventory and HTML/Markdown twins. The build checks their freshness.
Focused checks and scope are in the
[verification record](../../docs/verification/2026-10-30-nip-pages.md).
