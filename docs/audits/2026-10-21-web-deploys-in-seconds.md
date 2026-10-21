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

## What we keep

- Rust everywhere, with no TypeScript, plus the crates shared with the
  desktop app, the phone app, and Pylons.
- Staging-first promotion and smoke checks, run against each manifest.
- Cloud Run, which now runs a host that changes rarely.

## Phases

1. **A benchmarkable prototype, in days.** A standalone stable host with a
   manifest, runtime templates, signed-pointer following over a local relay,
   one browser module by hash, and one handler as a component. It measures
   edit-to-live latency for each tier.
2. **The real site, in about a week.** Content and modules move to
   manifests, the host follows pointers, and the deploy command learns the
   tiers.
3. **Handlers, in about two weeks.** The most-changed routes become
   components, and the warm builder starts confirming hashes.

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
