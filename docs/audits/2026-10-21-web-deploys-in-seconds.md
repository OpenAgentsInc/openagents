# Web deploys in seconds: a design

Written 2026-10-21. It follows
[the deploy-speed assessment](2026-10-21-web-deploy-speed-and-stack.md),
whose option 1 makes today's image pipeline cheaper. That's a necessary
step, but it still ends every change with a new image and a new Cloud Run
revision. This design removes that step for nearly every web change.

**Goal:** most web changes are live on staging in **seconds**, not minutes,
with Rust and the shared crates kept.

## The idea

Today a change is compiled into a binary, packed into an image, and shipped
as a new revision. In this design the Cloud Run service runs a **stable
host** that rarely changes. The host serves whatever version a **signed
pointer** names. A deploy uploads the changed objects and moves the
pointer.

```text
edit → build only the changed unit, warm → upload by content hash
     → publish a new manifest → sign a pointer to it
     → hosts and browsers follow the pointer
```

## Tiers

| What changed | How it ships | Target |
| --- | --- | --- |
| Copy, CSS, HTML templates, docs | Data, not code. The host interprets templates at run time. Upload the changed objects and publish a manifest; hosts swap it in memory; connected browsers hot-swap CSS or patch the page | 1–3 s |
| A browser module (Grid, chat, Pylon layer, and others) | WebAssembly is host-independent, so it builds incrementally on a warm machine. Upload it by content hash and point the manifest at it; browsers load it on their next navigation, or live where that's safe | 10–40 s |
| Server logic (routes, handlers, APIs) | Handlers are WebAssembly components loaded inside the Rust host with wasmtime. They're still written in Rust, built incrementally for `wasm32-wasip2`, and loaded per manifest version; the host doesn't restart. The component interface is the stable boundary native Rust lacks | 15–30 s |
| The host (runtime, auth core, Nostr plumbing) | Today's image path, or blue-green on our own machines, a few times a week | Minutes |

## The pointer is a Nostr event

We already run the relay that hosts and browsers connect to. A deploy is a
signed event, of a new kind, naming a manifest hash and a channel:
`staging` or `production`.

- **Authorization and audit:** only the deploy key can sign. Every deploy
  is a record on the relay.
- **Rollback:** republish the previous manifest's pointer. It takes about
  a second.
- **Fan-out:** hosts and browsers subscribe to the pointer and see a change
  within about a second. No polling, and no new revision.
- **Promotion:** after staging's smoke checks pass, sign the same manifest
  hash for the `production` channel.

## The manifest

A manifest is an immutable, content-addressed document. It lists:

- template, CSS, and document objects by SHA-256;
- browser modules, each with its `.wasm`, JavaScript glue, and `.br` and
  `.gz` copies;
- handler components with the routes they serve;
- the host version it requires. A host refuses a manifest that needs a
  newer host.

Objects live in a Cloud Storage bucket with immutable caching. Each request
pins one manifest from start to finish, so a page never mixes versions.

## Where builds run

Seconds rule out cold machines.

- **Module and handler builds** run incrementally where the edit happened:
  the developer's Mac, an agent's worktree, or one always-on warm Linux
  builder. A one-line `openagents-web` rebuild already takes 17–27 s
  locally.
- **Provenance:** the warm builder rebuilds each published artifact and
  confirms its hash. Staging may take the fast path. Production promotion
  requires a confirmed hash.

## What has to change in the code

1. **Content leaves the binary.** Templates, CSS, and documents move off
   `include_str!` and `include_bytes!` into the manifest. Compiled-in copies
   stay as a fallback.
2. **The host loads manifests,** follows the pointer, pins a manifest per
   request, and serves objects from the bucket or a local cache.
3. **Handlers move to components gradually.** Start with the pages and
   APIs that change most, and keep security-critical routes such as
   sign-in, billing, and keys in the host.
4. **Browsers hot-reload.** A small client subscribes to the pointer,
   swaps CSS, and reloads changed modules at safe points.
5. **`web.sh deploy` replaces `web.sh stage`** for ordinary changes. It
   detects which tier a change belongs to, builds only that, uploads,
   publishes, and signs.

## Hosted sites and games

The same machinery has to work for what other people build on OpenAgents:
creators' sites and, above all, **games** that update while people play
them. Treat our own website as the first tenant of a general hosting layer,
not as a special case.

### Every site and game is a tenant

- **Its own manifest and channels.** A tenant (a site, a game, or a Grid
  zone) has its own manifest history and its own `staging` and `live`
  channels. Its pointer events name the tenant, and the tenant's own key
  signs them, not ours.
- **Shared objects.** Objects live in one content-addressed store, so a
  library, texture, or model that two games use is stored once.
- **Quotas and moderation.** Per-tenant quotas cover storage, egress, and
  publish rate. Moderation can pin a tenant to a known-good manifest, or
  take its pointer down.
- **The Grid as the directory.** A hosted game is a place in the Grid: a
  portal or zone whose beacon names the tenant and its live channel.
  Entering the portal follows that pointer.

### Rapid game code updates

- **Game logic ships as components.** It's built incrementally (Rust to
  WebAssembly components, or anything that targets the component model),
  uploaded by hash, and pointed to. A creator's rebuild is live in seconds.
- **State outlives code.** The engine keeps world and player state outside
  the swapped component, so a code swap doesn't reset the world. A
  component declares the version of the state it reads. A changed state
  shape needs an explicit migration, or a "reload this zone" step that
  players can see.
- **Swaps happen at safe points:** between simulation ticks or frames,
  never in the middle of one. Players who are mid-fight finish the tick on
  the old code and continue on the new.
- **Servers and clients move together.** Where a game has server-side
  logic (on Pylons or our hosts), the pointer moves both sides. Each side
  refuses a peer whose manifest is incompatible, the same way stage groups
  refuse mismatched builds today.

### Asset streaming

- **Assets are content-addressed chunks:** meshes, textures, audio, and
  levels, each with levels of detail. Clients stream what's near the player
  first, as MMO-style area streaming, and prefetch along likely paths.
- **A new asset needs only a new pointer.** Re-publishing an asset is a new
  hash plus a manifest change. Clients already in the area fetch the new
  chunk and swap it at the next safe frame. Unchanged chunks stay cached
  everywhere, because their hashes don't change.
- **Delivery.** The CDN serves popular chunks. Pylons and peers can serve
  chunks too, since content hashes make any source verifiable. This fits
  NIP-PYLON and the Grid's existing Nostr transport.
- **Budgets.** Each zone declares a first-play budget, as the Grid's
  first-frame budget does today. A publish that breaks the budget gets a
  warning on staging and is refused on `live` unless the creator overrides
  it.

### One pointer, every platform

The pointer and the manifest are platform-neutral. Each client follows the
same channel:

| Client | What it loads |
| --- | --- |
| Web | WebAssembly modules and components, and assets over HTTP, hot-swapped in the page |
| Desktop (GPUI app, Verse) | The same components in wasmtime, natively compiled where the manifest offers it, plus the same asset chunks |
| Mobile (iOS and Android over the shared Rust core) | The same components in an embedded WebAssembly runtime, with interpreted or AOT modes where store rules require them, plus the same chunks, streamed under data-saver budgets |

A manifest can list builds per target, such as a generic WebAssembly build
or a native build for each platform. Each client picks the best build it
can run, and falls back to the generic one.

### Demo targets

Not built yet. These are the demos to prepare for:

1. **Live edit in a running zone.** Change a creature's behavior or a
   texture on a laptop, and watch it change in the Grid for every connected
   player, on web, desktop, and phone, within seconds, without anyone
   reloading.
2. **A creator publishes a game.** A creator, such as Blue Rush, publishes a
   small game as a tenant. It appears as a portal in the Grid, and players
   walk in while assets stream.
3. **Rollback in front of an audience.** Push a broken build, roll back in
   a second, and show that nobody lost world state.

## What we keep

- Rust everywhere, with no TypeScript, plus the crates shared with the
  desktop app, the phone app, and Pylons.
- Staging-first promotion and smoke checks, run against each manifest.
- Cloud Run, which now runs a host that changes rarely.

## Phases

1. **A benchmarkable prototype, in days.** Keep it multi-tenant from the
   start: a manifest and channels for each tenant, so our website is just
   tenant one. A standalone stable host with a
   manifest, runtime templates, signed-pointer following over a local relay,
   one browser module by hash, and one handler as a component. It measures
   edit-to-live latency for each tier.
2. **The real site, in about a week.** Content and modules move to
   manifests, the host follows pointers, and the deploy command learns the
   tiers.
3. **Handlers, in about two weeks.** The most-changed routes become
   components, and the warm builder starts confirming hashes.
4. **Hosted tenants and games, after that.** Tenant keys and quotas,
   component game logic with state that outlives code swaps, chunked asset
   streaming, and clients for the desktop and phone apps that follow the
   same pointers. Plan this to support the demos above.

## Risks

- **Component boundaries:** handlers need a clean interface to the host's
  state, such as sessions, the chat store, and Nostr. Keep it narrow, and
  start with handlers that only render.
- **Consistency:** manifests and per-request pinning prevent version mixes.
  Live browser patches must fall back to a reload when they can't apply.
- **Security:** a signed pointer must never bypass checks. The production
  channel needs a separate key and a confirmed build hash.
- **Cost:** a warm builder and one bucket cost about the same as today's
  Cloud Build runs.

## Prototype measurements

Phase 1 is built: [`crates/livehost`](../../crates/livehost/README.md)
(#12430), landed in `8790fc1893` with the benchmark fix in `d5d61b6f37`.
The numbers below come from `d5d61b6f37` on the owner's Mac (Apple
silicon, macOS 26.4), measured on 2026-10-21 while other agents were
building (load average 25–55).

### What the prototype is

- **Host:** `livehost serve`, an axum server with wasmtime 48. It follows
  one signed pointer per tenant and channel over a local `nostr-relay`
  (Postgres 16), loads the manifest it names, and swaps it in memory. Each
  request keeps the manifest it started with.
- **Tenants:** two, signed by different keys: `site` (runtime templates,
  copy, a stylesheet, a browser module, and a handler component) and
  `game` (a plain directory with an SVG sprite). Both share one object
  store.
- **Pointer:** kind `30440`, `d` = `TENANT/CHANNEL`, `m` = the manifest's
  SHA-256. The host refused a pointer that another key signed for `game`,
  and its state didn't change.
- **Deploy:** `livehost deploy` digests every file and each guest
  package's sources, rebuilds only a changed guest with incremental
  `cargo build --release`, uploads new objects with `br` and `gzip`
  copies, writes the manifest, and publishes the pointer.
- **Browser:** headless Chrome from `openagents browser run`, with three
  tabs open (`/`, `/hello`, and the game). The page's live client reports
  each update it shows, after the stylesheet loads or after the reloaded
  page and its module finish. The bench times each edit from the moment
  it writes the file.

### Edit to live, local store

Five measured runs per case, after one unmeasured warm-up run per case.
Times are median / worst.

| Change | Deploy command | Edit to host serving | Edit to browser updated | How the browser updated |
| --- | --- | --- | --- | --- |
| Stylesheet | 6 / 19 ms | 7 / 21 ms | 8 / 25 ms | Swapped in place, no reload |
| Copy (page data) | 6 / 6 ms | 7 / 8 ms | 23 / 42 ms | Reload |
| Template | 6 / 18 ms | 7 / 19 ms | 24 / 38 ms | Reload |
| Browser module (`wasm32-unknown-unknown`, 3 KB) | 129 / 172 ms | 131 / 173 ms | 150 / 190 ms | Reload |
| Handler component (`wasm32-wasip2`, 57 KB) | 190 / 287 ms | 199 / 298 ms | 212 / 318 ms | Reload |
| Game sprite, second tenant | 5 / 8 ms | 5 / 10 ms | 19 / 25 ms | Reload; `site` kept its manifest in all 5 runs |
| Rollback to the previous manifest | 3 / 4 ms | 4 / 4 ms | 18 / 21 ms | Reload |

Build time is almost all of the module and handler cases: 121–158 ms for
the module and 163–274 ms for the handler. The host compiled the new
handler component in 10–19 ms. Every other phase took 1–3 ms.

### Serving during swaps

64 connections for 20 s, spread over `/` (template), `/hello` (handler
component, one instance per request), and the game's page:

| Run | Requests/s | p50 | p99 | Max | Errors | Mixed-version responses |
| --- | --- | --- | --- | --- | --- | --- |
| No swaps | 56,276 | 0.92 ms | 3.60 ms | 47 ms | 0 | 0 |
| 31 swaps (copy, stylesheet, template, and handler changes) | 55,003 | 0.95 ms | 3.74 ms | 76 ms | 0 | 0 |

"Mixed-version" counts responses whose body named a different manifest
than their header; there were none across 33 manifests.

### With Cloud Storage

A second host read from `gs://openagents-bench-artifacts/livehost-bench-12430`
(since deleted) on its own channel, with no browser. Three runs per case,
median / worst:

| Change | Deploy command | Edit to host serving |
| --- | --- | --- |
| Stylesheet | 693 / 851 ms | 918 / 1,067 ms |
| Copy | 396 / 435 ms | 649 / 683 ms |
| Browser module | 457 / 518 ms | 663 / 742 ms |
| Handler component | 732 / 908 ms | 1,027 / 1,149 ms |
| Game sprite | 346 / 349 ms | 541 / 557 ms |
| Rollback | 189 / 225 ms | 272 / 311 ms |

Each Cloud Storage round trip from the Mac cost 75–300 ms. A deploy makes
three in sequence (read the current manifest, upload, write the manifest),
and the host two (manifest, then objects). The first full upload of the
site, 10 objects and 107 KB, took 974 ms.

### Commands

```sh
export CARGO_TARGET_DIR=~/work/openagents-target-1 CARGO_BUILD_JOBS=6
. scripts/dev/cargo-build-env.sh
cargo build --release -p livehost -p nostr-relay
PATH=/opt/homebrew/opt/postgresql@16/bin:$PATH crates/livehost/scripts/relay.sh &
export LIVEHOST_GUEST_TARGET=~/work/openagents-target-1-livehost
crates/livehost/scripts/demo.sh WORK &      # host on :8090; prints the deploy commands
# Deploy each tenant once with the printed commands, then:
openagents browser run --timeout 60 -- "$CARGO_TARGET_DIR/release/livehost" bench \
  --host 127.0.0.1:8090 --store WORK/store --relay ws://127.0.0.1:7448 \
  --site WORK/src/demo/site --site-key WORK/site.key \
  --game WORK/src/demo/game --game-key WORK/game.key \
  --runs 5 --load 64,20 --out WORK/bench.json
# Cloud Storage: a host with --store gs://BUCKET/PREFIX and a host config on
# channel "gcs", then the same bench with --host 127.0.0.1:8091
# --store gs://BUCKET/PREFIX --channel gcs --runs 3, outside the browser.
```

### What the numbers mean

- **Content changes are bounded by the network, not the deploy.** Locally,
  everything from the edit to a visible browser change takes tens of
  milliseconds; the deploy itself takes under 10 ms. With Cloud Storage,
  it takes about a second, all of it in round trips we can remove.
- **Code tiers are bounded by build time.** The demo's guests are tiny, so
  they build in 0.1–0.3 s. A real browser module or handler from
  `openagents-web` would take as long as its incremental rebuild, which
  this prototype doesn't measure.
- **Swaps don't hurt serving.** Throughput and p99 barely moved during 31
  swaps, with no errors and no mixed versions. A handler instance per
  request is cheap enough at this size.

### What phase 2 needs

- **Measure real code.** Time an incremental rebuild of one real browser
  module (such as the pylon layer) and one real route compiled as a
  component. That decides whether the module and handler tiers meet their
  10–40 s and 15–30 s targets.
- **Fewer Cloud Storage round trips.** Cache the current manifest beside
  the deploy key so a deploy needn't read it back, upload the manifest in
  parallel with the objects, and send the manifest's object list with the
  pointer so a host fetches everything at once. Hosts in Cloud Run sit
  next to the bucket, so their reads should be faster than from the Mac.
- **Move the real site's content.** Load `openagents-web`'s templates,
  CSS, and documents from a manifest, with compiled-in copies as the
  fallback, and route through the host's per-request pin.
- **Pick the template language.** The prototype's own interpreter has
  values, loops, conditions, partials, and asset URLs. The real pages
  need whatever their Rust renderers do today.
- **A host interface for handlers.** The WIT boundary has no imports yet.
  Real handlers need sessions, the chat store, and Nostr, kept narrow.
- **Hosting:** per-tenant quotas, moderation pins, promotion from
  `staging` to `live` with a separate key and a confirmed build hash, and
  object eviction in the host.
- **Pointer timestamps.** A relay orders pointers by `created_at` in whole
  seconds, so a deploy stamps one second past the previous pointer.
  Several deploys a second run the stamp ahead of the clock. A sequence
  tag would fix it.

## Phase 2: openagents.com on livehost

Phase 2 is #12435. Its first step measured real code against the tier
targets; the content tier then moved onto livehost for staging.

### Rebuild times for real code

One-line edits on the owner's Mac with a warm shared target, under other
agents' load, timed through everything a deploy needs: cargo,
`wasm-bindgen`, `wasm-opt`, and compression. Median / worst of three to five
runs.

| Module | Production settings (LTO, codegen-units=1, `wasm-opt`, brotli 11) | Staging settings (no LTO, codegen-units=16, incremental, no `wasm-opt`, brotli 5) |
| --- | --- | --- |
| `pylons-web`, the /pylons Grid layer (212 KB) | 2.9 / 3.0 s | 0.90 / 0.96 s |
| `coder-chat-web` (73 KB) | 1.7 / 1.7 s | 0.93 / 0.98 s |
| `everglade-web`, the 3D Grid (25 MB), edit in its own crate | 146 / 157 s | 5.5 / 5.6 s |
| `everglade-web`, edit in the shared `verse-core` | not measured | 7.8 / 8.2 s |

For `everglade-web` under production settings, cargo took 42–64 s,
`wasm-opt -O2` 13–17 s, and brotli 11 with gzip 9 59–85 s. The first build
under staging settings is cold (183 s for `everglade-web`), and the staging
module is larger (29 MB against 25 MB).

A real route as a component: /pylons' list rendering over the shared
`pylons-view` crate, built for `wasm32-wasip2`
(`crates/livehost/guests/pylons-route`, 162 KB), rebuilt in 0.25 / 0.39 s
after an edit to the route and 0.39 / 0.43 s after an edit to
`pylons-view`. The host compiled it in 23 ms. For comparison, the same edit
to `pages/pylons_field.rs` rebuilt the native `openagents-web` server in
10.4 s median (102 s worst), before an image or a revision.

So the module and handler targets (10–40 s and 15–30 s) hold with room to
spare once staging builds with its own settings. Production keeps LTO and
brotli 11 on the promotion path. The open questions are the GPUI modules
(`apps/openagents/web`, on their own toolchain), which aren't measured,
and a host interface for live data, which a real route needs and which
`openagents-web`'s routes can't use until they are extracted from the
server crate.

### The content tier

- **What moved.** `openagents-web` reads the docs, the legal text, the API
  docs, `fonts.css`, `legacy-demo.css`, and the `openagents-ui` stylesheet
  and script from the `openagents` tenant's manifest
  (`crates/openagents-web/src/live_content.rs`). The compiled-in copies stay:
  a path the manifest lacks, or a server without the follower, serves them.
  The blog isn't moved: its posts are parsed into borrowed `'static` data.
- **How a server follows.** `OPENAGENTS_LIVE_RELAY`, `_STORE`, and `_PUBKEY`
  (and `_CHANNEL`, default `staging`) start `livehost::follow`, the
  follower half of the crate, without wasmtime. It verifies each pointer,
  takes the manifest from the pointer itself when it fits (under 48 KiB),
  and fetches every new object at once.
- **Deploy.** `scripts/deploy/livehost.sh deploy` assembles the tier
  (`scripts/deploy/web-content.sh`, which runs `openagents-ui-bundle` for
  the same stylesheet bytes the build compiles in), uploads new objects and
  the manifest in parallel to `gs://openagentsgemini-livehost`, and signs
  the staging pointer on `wss://relay.openagents.com`. `rollback` and
  `status` work the same way. The tools build in a target of their own: in
  the shared target a content deploy waited minutes on other builds'
  artifact lock.
- **Promotion.** `livehost.sh promote HASH` points `live` at staging's
  manifest with a separate live key, and refuses unless staging serves
  exactly HASH. No production server follows `live`.

### On staging

Staging follows the staging pointer since revision
`openagents-web-1-staging-837c2346c4-223046` (setup in
[the deployment log](../deployment/openagents-web.md#2026-10-21-content-deploys-from-livehost-on-staging-12435)).
A docs edit was visible on staging.openagents.com 3.8 s median and 8.5 s
worst after the file changed (5 runs, three matching responses in a row),
and a rollback 3.1 s median, against roughly 7 to 60 minutes through an image
and a revision. The deploy command itself took about 1 s: 0.3–1.0 s to read
the current pointer from the relay, 0.15–0.7 s to upload, and 0.1–0.2 s to
publish. A server swaps 0–58 ms after the pointer arrives and waits up to
3 s for the first manifest when it starts.

### Modules and handlers on staging

Staging revision `openagents-web-1-staging-541b2ee804-234004` serves the
/pylons list from a handler component and browser module files from the
content manifest (commits `9b12a573d7`, `c058c5184c`, `462605f774`), with the
staging fast-build settings from `scripts/deploy/staging-build-env.sh`
(`8bdd5e4d6f`). Measured from this Mac on 2026-10-22, five runs after one
warm-up, a change counted as live after three matching responses in a row:

| Change | Edit to live on staging, median / worst | Of which: build | Of which: `livehost.sh deploy` |
| --- | --- | --- | --- |
| A docs page | 4.4 / 4.9 s | none | 3.4–4.1 s |
| The /pylons list component | 5.1 / 5.9 s | 0.15–0.19 s | 3.0–3.7 s |
| The pylons-web browser module (staging settings, 615 KB with copies) | 5.5 / 6.2 s | 0.8–1.0 s | 3.3–4.1 s |
| Rollback of the docs change | 3.1 / 3.3 s | none | 2.1–2.5 s |

The staging smoke passed afterward (106 passed, 3 skipped). When a deploy
left the modules out again, staging served the image's copy of the module.
In the script's 3–4 s, the upload and publish take about 1 s; the rest is
Cargo's freshness checks for the deploy tools and the stylesheet bundler,
which calling the built binaries directly would remove.

How it fits together:

- **Handlers.** `crates/openagents-web/components/pylons-list` is the /pylons
  list as a `site-route` component. The server shares one thing with it,
  `data.read("pylons-snapshot")`, runs it with wasmtime per request, and
  renders natively when the manifest has no such handler or it fails.
- **Modules.** A route that serves module files through `serve_build` reads
  `modules/<path under /srv>/<file>` from the manifest first, with its
  `.br` or `.gz` copy and the object hash as `ETag`, and falls back to the
  image's file. Module objects load on first request within a 256 MB
  budget. #12427 builds modules by content hash and hands them to
  `livehost.sh deploy` as `LIVEHOST_MODULES_DIR`; after an image deploy it
  publishes the image's modules the same way, so a stale manifest never
  overrides a newer image.
- **Staging build settings.** `. scripts/deploy/staging-build-env.sh` before
  any `scripts/build-*-web.sh`: no LTO, 16 codegen units, incremental, no
  `wasm-opt`, brotli 5, gzip 6. Production promotion keeps the production
  settings.

### What's left

- **Production** follows nothing until the owner approves the live key and
  production's `OPENAGENTS_LIVE_*` variables (an after-close check of
  #12435).
- **More routes** become components as they are pulled out of
  `openagents-web` behind narrow `data.read` names, and pages that inline a
  module's glue (lander5's runner) read it from the manifest too.
- **`web.sh`** calls `livehost.sh` once #12417 and #12427 land their changes.
- **The blog**, whose posts are parsed into `'static` data.
