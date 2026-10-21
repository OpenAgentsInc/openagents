# Web deploy speed: fix the pipeline or change the stack?

Written 2026-10-21, after three staging builds of the website timed out at
Cloud Build's 60-minute limit in one day. The owner asked whether the
website should leave Rust for HTML, CSS, and JavaScript with a fast backend
such as PHP, keeping Rust only for desktop nodes.

**Short answer:** most of the hour isn't Rust compile time, and the backend
language isn't the bottleneck. The website deploys as one monolithic image
that rebuilds and re-optimizes about 20 browser modules on a fresh machine
every time. Fix that, and a server change can reach staging in about 5
minutes and a copy or CSS change in under a minute, without a rewrite. If
the pipeline fix misses those targets within a week, move the non-3D page
shell to plain HTML, CSS, and JavaScript, and keep Rust for the server and
the 3D Grid.

## What a deploy does today

`scripts/deploy/web.sh stage COMMIT` submits two Cloud Build jobs and then
updates Cloud Run. The web image (`crates/openagents-web/Dockerfile`)
does all of the following on every deploy:

- **The server:** `openagents-web` is one crate of about **201,000 lines**,
  part of a 288-crate workspace.
- **About 20 browser modules**, each its own release WebAssembly build
  (`-p` list in the Dockerfile): the 3D Grid (`everglade-web`), the pylon
  layer, lander chat, the GPUI chat and its WebGL variant, files, agent
  graph, CAD, wallet, and others. The GPUI modules come from
  `apps/openagents/web` (45 directories) on a **second toolchain**
  (Rust 1.98.1, against 1.97.1 for the rest).
- **Post-processing for each module:** `wasm-bindgen`, `wasm-opt`
  (Binaryen), `brotli -q 11`, and `gzip -9`.
- **One Docker image** carrying the server and every module, so any change
  rebuilds and re-ships everything.

## Where the hour went

From the log of staging build `02c298a0` (commit `f16e902818`), which timed
out at 3,600 s:

| Cause | Evidence | Share of the hour |
| --- | --- | --- |
| Serial post-processing of unchanged modules | Gaps of 600–730 s with no output between a cargo "Finished" line and each module's size report (files, chat, chat WebGL, keys WebGL, agent graph, and others) | Most of it |
| One build stage that misses the shared cache | "Hit rate 14.0% … miss: 690" for one stage, likely the GPUI stage on its own toolchain | 10–20 minutes |
| Rust compilation that hit the cache | Other stages hit kache's remote store at 80–92% | A few minutes |

The shared compile cache (`gs://openagentsgemini-rust-build-cache`,
`oa-kache`, #12176) does work. For comparison, AGENTS.md records a one-line
edit to `openagents-web` rebuilding in **27 s** locally. The time goes to
architecture: rebuilding everything, serially, on a fresh machine, as one
artifact.

## Options

### 1. Keep Rust and fix the pipeline (recommended first)

| Change | Effect |
| --- | --- |
| Ship browser modules separately from the server: upload each built module to Cloud Storage under its content hash, and have the server read a small manifest | A server change no longer rebuilds or ships modules; a module change ships only that module |
| Hash each module's inputs and skip building and optimizing it when the hash already exists | Unchanged modules cost seconds instead of about 10 minutes each |
| Run module builds and post-processing in parallel | The remaining module work runs concurrently instead of serially |
| Fix the stage that misses the cache, so its keys match | Removes about 690 cold crate compiles |
| Staging uses lighter settings: lower `wasm-opt` level, `brotli -q 9`, thin LTO | Faster staging builds, with production unchanged |
| Load page copy, HTML templates, and CSS at run time from a content directory instead of compiling them in | Text and style edits deploy as a file upload, in seconds |
| Optionally, a persistent Linux builder with a warm target directory | Incremental server builds of about 1–3 minutes, as on the Mac |
| Later, split `openagents-web` into smaller crates | Faster incremental rebuilds once a cache is warm. Doesn't help a cold build on its own |

**Targets:** a copy or CSS change on staging in under 1 minute, a server
change in about 5 minutes, and a single changed browser module in about 10
minutes. **Cost:** days for the first three rows, about two weeks for all of
it. Nothing user-facing is rewritten.

### 2. Hybrid: a plain web shell, Rust where it pays

Write the page shell (navigation, marketing pages, forms, settings, the chat
layout) in server-rendered HTML, CSS, and a little JavaScript. Keep Rust for
the server, for the 3D Grid and Verse (WebGPU/WebGL, physics, Nostr
movement), and for code shared with the desktop and phone apps: Nostr, the
wallet, Pylon routing, chat cards.

- **Gains:** front-end edits deploy in seconds with no compile step.
  Designers and agents iterate faster.
- **Costs:**
  - It changes two binding rules in AGENTS.md. Product code is Rust (with
    TypeScript banned), and Rust Native is the card and sheet layer on every
    surface.
  - The web chat, now GPUI in WebAssembly, would be rebuilt as HTML.
  - Cards drawn by one shared Rust layer for desktop, phone, and web would
    diverge on the web.
- **Effort:** weeks for the shell and chat UI.

### 3. Rewrite the website (HTML/CSS/JS front end, PHP, Node, or Go backend)

- **Gains:** the fastest possible deploys and a familiar stack.
- **Costs:**
  - About 201,000 lines of server features to rebuild: accounts and
    sessions, billing, the chat store, the Pylon router, the gateway, admin
    pages, and Nostr integration.
  - Protocol logic now shared with the desktop app and Pylons (the `nostr`,
    `pylon`, and `verse-net` crates) would be duplicated in a second
    language, so the two can drift.
  - The 3D Grid would need a rewrite in a JavaScript 3D library.
- **Effort:** months, during the SPEEDRUN window.
- **On PHP specifically:** a PHP or Go server deploys quickly because it
  ships little, not because the language is faster. Our Rust server isn't
  the slow part. The browser-module pipeline is, and a rewrite only removes
  it by dropping what those modules do.

## Recommendation

1. **Now:** do option 1's first four rows: separate module shipping,
   content-hash skipping, parallel post-processing, and the cache-miss fix.
   The build-cache agent already owns the last two. Add the runtime content
   directory for copy and CSS.
2. **Set a deadline:** by 2026-10-28, any website change reaches staging in
   at most 5 minutes, and copy or CSS in at most 1 minute, measured on real
   commits.
3. **If the deadline is missed:** adopt option 2 for the page shell, after
   an explicit change to the AGENTS.md rules above. Keep Rust for the server
   and the 3D Grid.
4. **Don't nuke and restart (option 3)** unless options 1 and 2 both fail.
   The deploy problem doesn't justify losing the shared code across desktop,
   phone, web, and Pylons, or months of rework.

## Measure it

Record for every staging deploy:

- the build and deploy step times, which `web.sh` already prints with `took`;
- the kache hit rate per stage;
- how many modules were rebuilt versus reused.

Keep them in the deploy log so the 5-minute target can be checked.
