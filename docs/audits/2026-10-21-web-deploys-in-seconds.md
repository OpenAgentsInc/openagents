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
